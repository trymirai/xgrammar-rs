//! Compressed-sparse-row (CSR) 2D array — a port of `cpp/support/compact_2d_array.h`.

use serde::{Deserialize, Serialize};

/// A 2D array stored in Compressed Sparse Row (CSR) format: every row may have a
/// different length, and all rows live contiguously in one backing buffer.
///
/// Two parallel vectors back it:
/// - `data` — all row elements, concatenated;
/// - `indptr` — the start offset of each row in `data`; its last element equals
///   `data.len()`, so row `i` occupies `data[indptr[i]..indptr[i + 1]]`.
///
/// Rows are immutable once inserted (you can still mutate their elements in place via
/// [`Compact2dArray::row_mut`]). Unlike the C++ original, which hands out raw-pointer
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
        Ok(Self { data, indptr })
    }

    /// Number of rows.
    #[must_use]
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
    pub fn get(&self, i: usize) -> Option<&[T]> {
        if i >= self.len() {
            return None;
        }
        let start = self.indptr[i] as usize;
        let end = self.indptr[i + 1] as usize;
        Some(&self.data[start..end])
    }

    /// Returns row `i` as a slice.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds (an internal invariant violation).
    #[must_use]
    pub fn row(&self, i: usize) -> &[T] {
        self.get(i).expect("row index out of bounds")
    }

    /// Returns row `i` as a mutable slice.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds.
    pub fn row_mut(&mut self, i: usize) -> &mut [T] {
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
    pub fn pop_back(&mut self, cnt: usize) {
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
    /// Panics if any row size is negative.
    pub fn reset_with_row_sizes(&mut self, row_sizes: &[i32])
    where
        T: Default,
    {
        self.indptr.clear();
        self.indptr.reserve(row_sizes.len() + 1);
        self.indptr.push(0);
        let mut acc = 0i32;
        for &size in row_sizes {
            assert!(size >= 0, "Compact2dArray row size cannot be negative");
            acc += size;
            self.indptr.push(acc);
        }
        self.data.clear();
        self.data.resize_with(acc as usize, T::default);
    }

    /// Appends a new row, returning its index.
    pub fn push_row(&mut self, new_data: &[T]) -> usize {
        self.data.extend_from_slice(new_data);
        self.indptr.push(self.data.len() as i32);
        self.indptr.len() - 2
    }

    /// Appends a new row consisting of one leading element followed by `rest`.
    ///
    /// Mirrors the C++ `PushBackNonContiguous`, used by the grammar-expression encoding.
    pub fn push_row_noncontiguous(&mut self, first: T, rest: &[T]) -> usize {
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
    /// Panics if the array has no rows yet.
    pub fn push_in_latest_row(&mut self, new_data: T) {
        assert!(!self.is_empty(), "cannot push into an empty Compact2dArray");
        self.data.push(new_data);
        *self.indptr.last_mut().expect("non-empty") += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_array_has_no_rows() {
        let arr = Compact2dArray::<i32>::new();
        assert_eq!(arr.len(), 0);
        assert!(arr.is_empty());
        assert_eq!(arr.indptr(), &[0]);
        assert!(arr.get(0).is_none());
    }

    #[test]
    fn push_rows_and_index() {
        let mut arr = Compact2dArray::new();
        assert_eq!(arr.push_row(&[1, 2, 3]), 0);
        assert_eq!(arr.push_row(&[]), 1);
        assert_eq!(arr.push_row(&[4]), 2);

        assert_eq!(arr.len(), 3);
        assert_eq!(arr.row(0), &[1, 2, 3]);
        assert_eq!(arr.row(1), &[] as &[i32]);
        assert_eq!(arr.row(2), &[4]);
        assert_eq!(arr.indptr(), &[0, 3, 3, 4]);
        assert_eq!(arr.back(), &[4]);
    }

    #[test]
    fn push_noncontiguous_prepends_leading_element() {
        let mut arr = Compact2dArray::new();
        let idx = arr.push_row_noncontiguous(7, &[8, 9]);
        assert_eq!(idx, 0);
        assert_eq!(arr.row(0), &[7, 8, 9]);
    }

    #[test]
    fn push_in_latest_row_extends_back() {
        let mut arr = Compact2dArray::new();
        arr.push_row(&[1]);
        arr.push_in_latest_row(2);
        arr.push_in_latest_row(3);
        assert_eq!(arr.row(0), &[1, 2, 3]);
        assert_eq!(arr.indptr(), &[0, 3]);
    }

    #[test]
    fn row_mut_edits_in_place() {
        let mut arr = Compact2dArray::new();
        arr.push_row(&[1, 2, 3]);
        arr.row_mut(0)[1] = 20;
        assert_eq!(arr.row(0), &[1, 20, 3]);
    }

    #[test]
    fn pop_back_removes_rows_and_data() {
        let mut arr = Compact2dArray::new();
        arr.push_row(&[1, 2]);
        arr.push_row(&[3, 4, 5]);
        arr.push_row(&[6]);
        arr.pop_back(2);
        assert_eq!(arr.len(), 1);
        assert_eq!(arr.row(0), &[1, 2]);
        assert_eq!(arr.data(), &[1, 2]);
        assert_eq!(arr.indptr(), &[0, 2]);
    }

    #[test]
    fn from_row_sizes_default_constructs() {
        let arr = Compact2dArray::<i32>::from_row_sizes(&[2, 0, 3]);
        assert_eq!(arr.len(), 3);
        assert_eq!(arr.row(0), &[0, 0]);
        assert_eq!(arr.row(1), &[] as &[i32]);
        assert_eq!(arr.row(2), &[0, 0, 0]);
    }

    #[test]
    fn from_data_and_indptr_validates() {
        let ok = Compact2dArray::from_data_and_indptr(vec![1, 2, 3], vec![0, 2, 3]).unwrap();
        assert_eq!(ok.row(0), &[1, 2]);
        assert_eq!(ok.row(1), &[3]);

        assert_eq!(
            Compact2dArray::<i32>::from_data_and_indptr(vec![], vec![]),
            Err(Compact2dArrayError::EmptyIndptr)
        );
        assert_eq!(
            Compact2dArray::from_data_and_indptr(vec![1], vec![1, 1]),
            Err(Compact2dArrayError::NonZeroStart)
        );
        assert_eq!(
            Compact2dArray::from_data_and_indptr(vec![1, 2], vec![0, 2, 1]),
            Err(Compact2dArrayError::NotMonotonic)
        );
        assert_eq!(
            Compact2dArray::from_data_and_indptr(vec![1, 2, 3], vec![0, 2]),
            Err(Compact2dArrayError::BadEnd)
        );
    }

    #[test]
    fn iter_yields_rows() {
        let mut arr = Compact2dArray::new();
        arr.push_row(&[1, 2]);
        arr.push_row(&[3]);
        let rows: Vec<&[i32]> = arr.iter().collect();
        assert_eq!(rows, vec![&[1, 2][..], &[3][..]]);
    }

    #[test]
    fn serde_roundtrip() {
        let mut arr = Compact2dArray::new();
        arr.push_row(&[1, 2, 3]);
        arr.push_row(&[4]);
        let json = serde_json::to_string(&arr).unwrap();
        let back: Compact2dArray<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr, back);
    }
}
