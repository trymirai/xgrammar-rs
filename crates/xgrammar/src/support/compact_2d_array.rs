//! Compressed-sparse-row (CSR) 2D array.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A 2D array stored in Compressed Sparse Row (CSR) format: every row may have a
/// different length, and all rows live contiguously in one backing buffer.
///
/// Two parallel vectors back it:
/// - `data` — all row elements, concatenated;
/// - `indptr` — the start offset of each row in `data`; its last element equals
///   `data.len()`, so row `i` occupies `data[indptr[i]..indptr[i + 1]]`.
///
/// Rows are immutable once inserted (you can still mutate their elements in place via
/// [`Compact2dArray::row_mut`]). Provides
/// `Row` views, this returns borrowed slices — the borrow checker enforces the
/// "inserting a row invalidates outstanding rows" invariant for free.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Compact2dArray<T> {
    data: Vec<T>,
    indptr: Vec<i32>,
}

impl<T> Default for Compact2dArray<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            indptr: vec![0],
        }
    }
}

/// Error returned when a CSR representation fails its invariants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Compact2dArrayError {
    /// `indptr` was empty (it must contain at least the leading `0`).
    #[error("compact 2d array indptr cannot be empty")]
    EmptyIndptr,
    /// `indptr` did not start with `0`.
    #[error("compact 2d array indptr must start with 0")]
    NonZeroStart,
    /// `indptr` was not non-decreasing.
    #[error("compact 2d array indptr must be non-decreasing")]
    NotMonotonic,
    /// The final `indptr` entry did not equal `data.len()`.
    #[error("compact 2d array indptr must end with data.len()")]
    BadEnd,
}

impl<T> Compact2dArray<T> {
    /// Creates an empty array (zero rows).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds an array directly from a CSR representation, validating the invariants.
    ///
    /// # Errors
    /// Returns [`Compact2dArrayError`] if `indptr` is empty, does not start with `0`, is
    /// not non-decreasing, or does not end at `data.len()`.
    pub fn from_data_and_indptr(
        data: Vec<T>,
        indptr: Vec<i32>,
    ) -> Result<Self, Compact2dArrayError> {
        let Some(&first) = indptr.first() else {
            return Err(Compact2dArrayError::EmptyIndptr);
        };
        if first != 0 {
            return Err(Compact2dArrayError::NonZeroStart);
        }
        if indptr.windows(2).any(|w| w[0] > w[1]) {
            return Err(Compact2dArrayError::NotMonotonic);
        }
        if *indptr.last().expect("indptr is non-empty here") as usize != data.len() {
            return Err(Compact2dArrayError::BadEnd);
        }
        Ok(Self {
            data,
            indptr,
        })
    }

    /// Number of rows.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.indptr.len() - 1
    }

    /// Whether there are zero rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Total number of elements across all rows.
    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Returns row `i` as a slice, or `None` if out of bounds.
    #[must_use]
    #[inline]
    pub fn get(
        &self,
        i: usize,
    ) -> Option<&[T]> {
        (i < self.len()).then(|| &self.data[self.row_range(i)])
    }

    /// The positions of row `i` within [`Self::data`]. They stay valid while rows are appended,
    /// so a row can be walked by index while the array grows.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds.
    #[inline]
    #[must_use]
    pub fn row_range(
        &self,
        i: usize,
    ) -> std::ops::Range<usize> {
        self.indptr[i] as usize..self.indptr[i + 1] as usize
    }

    /// Returns row `i` as a slice.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds (an internal invariant violation).
    #[must_use]
    #[inline]
    pub fn row(
        &self,
        i: usize,
    ) -> &[T] {
        self.get(i).expect("row index out of bounds")
    }

    /// Returns row `i` as a mutable slice.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds.
    pub fn row_mut(
        &mut self,
        i: usize,
    ) -> &mut [T] {
        assert!(i < self.len(), "row index out of bounds");
        let start = self.indptr[i] as usize;
        let end = self.indptr[i + 1] as usize;
        &mut self.data[start..end]
    }

    /// Returns the last row.
    ///
    /// # Panics
    /// Panics if there are no rows.
    #[must_use]
    #[inline]
    pub fn back(&self) -> &[T] {
        assert!(!self.is_empty(), "Compact2dArray is empty");
        self.row(self.len() - 1)
    }

    /// Iterates over the rows as slices.
    pub fn iter(&self) -> impl Iterator<Item = &[T]> {
        (0..self.len()).map(move |i| self.row(i))
    }

    /// The flat backing data buffer.
    #[must_use]
    #[inline]
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// The row-offset (index pointer) buffer.
    #[must_use]
    pub fn indptr(&self) -> &[i32] {
        &self.indptr
    }

    /// Removes the last `cnt` rows.
    ///
    /// # Panics
    /// Panics if `cnt` exceeds the number of rows.
    pub fn pop_back(
        &mut self,
        cnt: usize,
    ) {
        assert!(cnt <= self.len(), "cannot pop more rows than exist");
        let new_len = self.indptr.len() - cnt;
        self.indptr.truncate(new_len);
        let new_data_len = *self.indptr.last().expect("indptr keeps its leading 0") as usize;
        self.data.truncate(new_data_len);
    }
}

impl<T: Clone> Compact2dArray<T> {
    /// Builds an array from per-row sizes, with every element default-constructed.
    #[must_use]
    pub fn from_row_sizes(row_sizes: &[i32]) -> Self
    where
        T: Default,
    {
        let mut out = Self::new();
        out.reset_with_row_sizes(row_sizes);
        out
    }

    /// Resets the array to `row_sizes.len()` rows of default-constructed elements.
    ///
    /// # Panics
    /// Panics if any row size is negative, or if the total size exceeds the `i32` limit.
    pub fn reset_with_row_sizes(
        &mut self,
        row_sizes: &[i32],
    ) where
        T: Default,
    {
        self.indptr.clear();
        self.indptr.reserve(row_sizes.len() + 1);
        self.indptr.push(0);
        let mut acc = 0i32;
        for &size in row_sizes {
            assert!(size >= 0, "Compact2dArray row size cannot be negative");
            assert!(size <= i32::MAX - acc, "Compact2DArray data size exceeds the int32_t limit");
            acc += size;
            self.indptr.push(acc);
        }
        self.data.clear();
        self.data.resize_with(acc as usize, T::default);
    }

    /// Appends a new row, returning its index.
    ///
    /// # Panics
    /// Panics if the total size would exceed the `i32` limit.
    pub fn push_row(
        &mut self,
        new_data: &[T],
    ) -> usize {
        self.check_can_append_data(new_data.len());
        self.data.extend_from_slice(new_data);
        self.indptr.push(self.data.len() as i32);
        self.indptr.len() - 2
    }

    /// Appends a new row consisting of one leading element followed by `rest`.
    ///
    /// Mirrors the C++ `PushBackNonContiguous`, used by the grammar-expression encoding.
    ///
    /// # Panics
    /// Panics if the total size would exceed the `i32` limit.
    pub fn push_row_noncontiguous(
        &mut self,
        first: T,
        rest: &[T],
    ) -> usize {
        self.check_can_append_data(rest.len() + 1);
        self.data.push(first);
        self.data.extend_from_slice(rest);
        self.indptr.push(self.data.len() as i32);
        self.indptr.len() - 2
    }
}

impl<T> Compact2dArray<T> {
    /// Appends one element to the most recently inserted row.
    ///
    /// # Panics
    /// Panics if the array has no rows yet, or if the total size would exceed the `i32` limit.
    pub fn push_in_latest_row(
        &mut self,
        new_data: T,
    ) {
        assert!(!self.is_empty(), "cannot push into an empty Compact2dArray");
        self.check_can_append_data(1);
        self.data.push(new_data);
        *self.indptr.last_mut().expect("non-empty") += 1;
    }

    /// Guards the `i32` row offsets before appending `additional` elements, so a pathological
    /// grammar fails loudly instead of wrapping the offsets.
    fn check_can_append_data(
        &self,
        additional: usize,
    ) {
        assert!(
            additional <= i32::MAX as usize - self.data.len(),
            "Compact2DArray data size would exceed the int32_t limit, likely due to an unbounded grammar pattern \
             causing state explosion"
        );
    }
}

impl Compact2dArray<i32> {
    /// Serializes in the C++ reflection form `{"data_":[...],"indptr_":[...]}`.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!({
            "data_": self.data,
            "indptr_": self.indptr,
        })
    }

    /// Deserializes the C++ reflection form.
    ///
    /// # Errors
    /// Returns an error string when the JSON shape is invalid.
    pub fn deserialize_json_value(value: &Value) -> Result<Self, String> {
        let obj = value.as_object().ok_or_else(|| "compact 2d array expects a JSON object".to_owned())?;
        let data = obj
            .get("data_")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing compact 2d array data_".to_owned())?
            .iter()
            .map(|v| v.as_i64().map(|n| n as i32).ok_or_else(|| "data_ elements must be integers".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        let indptr = obj
            .get("indptr_")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing compact 2d array indptr_".to_owned())?
            .iter()
            .map(|v| v.as_i64().map(|n| n as i32).ok_or_else(|| "indptr_ elements must be integers".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_data_and_indptr(data, indptr).map_err(|e| e.to_string())
    }
}
