//! Batched matcher operations for processing multiple [`GrammarMatcher`]s.

use std::ops::Deref;

use super::{error::MatcherError, grammar_matcher::GrammarMatcher};

/// A batched version of [`GrammarMatcher`] for better efficiency.
///
/// It provides batched versions of the core methods of [`GrammarMatcher`], including
/// [`Self::batch_fill_next_token_bitmask`], [`Self::batch_accept_string`], and
/// [`Self::batch_accept_token`].
///
/// `max_threads` is retained for API parity; the core batch helpers run sequentially. The
/// Python binding parallelizes `batch_fill_next_token_bitmask` with rayon.
#[derive(Debug, Clone, Copy)]
pub struct BatchGrammarMatcher {
    max_threads: i32,
}

impl Default for BatchGrammarMatcher {
    fn default() -> Self {
        Self::new(-1)
    }
}

fn check_batch_size(
    matchers: usize,
    other: usize,
    other_name: &str,
) -> Result<(), MatcherError> {
    if matchers == other {
        return Ok(());
    }
    Err(MatcherError::InvalidArgument(format!(
        "The size of matchers ({matchers}) and {other_name} ({other}) should be the same."
    )))
}

impl BatchGrammarMatcher {
    /// Creates a batch matcher. `max_threads <= 0` means "use hardware concurrency" at the
    /// binding layer; the core sequential helpers ignore it.
    #[must_use]
    pub fn new(max_threads: i32) -> Self {
        Self {
            max_threads,
        }
    }

    /// The configured thread limit (`<= 0` = auto).
    #[must_use]
    pub fn max_threads(&self) -> i32 {
        self.max_threads
    }

    /// A batched version of [`GrammarMatcher::accept_token`].
    ///
    /// # Errors
    /// Returns [`MatcherError::InvalidArgument`] if the slice lengths differ.
    pub fn batch_accept_token(
        matchers: &mut [GrammarMatcher],
        token_ids: &[i32],
    ) -> Result<Vec<bool>, MatcherError> {
        check_batch_size(matchers.len(), token_ids.len(), "token_ids")?;
        Ok(matchers.iter_mut().zip(token_ids).map(|(matcher, &token_id)| matcher.accept_token(token_id)).collect())
    }

    /// A batched version of [`GrammarMatcher::accept_bytes`].
    ///
    /// # Errors
    /// Returns [`MatcherError::InvalidArgument`] if the slice lengths differ.
    pub fn batch_accept_string(
        matchers: &mut [GrammarMatcher],
        inputs: &[&[u8]],
    ) -> Result<Vec<bool>, MatcherError> {
        check_batch_size(matchers.len(), inputs.len(), "input_strs")?;
        Ok(matchers.iter_mut().zip(inputs).map(|(matcher, input)| matcher.accept_bytes(input)).collect())
    }

    /// A batched version of [`GrammarMatcher::rollback`].
    ///
    /// # Errors
    /// Returns [`MatcherError`] if the slice lengths differ or a rollback exceeds the history.
    pub fn batch_rollback(
        matchers: &mut [GrammarMatcher],
        num_tokens: &[i32],
    ) -> Result<(), MatcherError> {
        check_batch_size(matchers.len(), num_tokens.len(), "num_tokens")?;
        matchers.iter_mut().zip(num_tokens).try_for_each(|(matcher, &count)| matcher.rollback(count))
    }

    /// A batched version of [`GrammarMatcher::fill_next_token_bitmask`]. `indices` selects the
    /// bitmask row of each matcher; by default `matchers[i]` writes row `i`.
    ///
    /// # Errors
    /// Returns [`MatcherError`] for mismatched sizes, out-of-range indices, an invalid bitmask,
    /// or a terminated matcher.
    pub fn batch_fill_next_token_bitmask(
        matchers: &mut [GrammarMatcher],
        bitmask: &mut [i32],
        shape: &[i64],
        indices: Option<&[i32]>,
    ) -> Result<(), MatcherError> {
        if let Some(indices) = indices {
            check_batch_size(indices.len(), matchers.len(), "matchers")?;
        }
        let rows = shape.first().copied().unwrap_or(0);
        for (batch_id, matcher) in matchers.iter_mut().enumerate() {
            let index = indices.map_or(batch_id as i32, |indices| indices[batch_id]);
            if index < 0 || i64::from(index) >= rows {
                return Err(MatcherError::InvalidArgument(format!(
                    "The index {index} is out of range [0, {rows}) for batch_id {batch_id}."
                )));
            }
            matcher.fill_next_token_bitmask(bitmask, shape, index)?;
        }
        Ok(())
    }

    /// Writes the temperature of each matcher (`-1` when none applies) into `temperatures`.
    /// `indices` selects the element of each matcher; by default `matchers[i]` writes element
    /// `i`.
    ///
    /// # Errors
    /// Returns [`MatcherError::InvalidArgument`] for mismatched sizes or out-of-range indices.
    pub fn batch_fill_temperature<M: Deref<Target = GrammarMatcher>>(
        matchers: impl ExactSizeIterator<Item = M>,
        temperatures: &mut [f32],
        indices: Option<&[i32]>,
    ) -> Result<(), MatcherError> {
        if indices.is_some_and(|indices| indices.len() != matchers.len()) {
            return Err(MatcherError::InvalidArgument("The indices size must match the number of matchers".to_owned()));
        }
        for (i, matcher) in matchers.enumerate() {
            let position = indices.map_or(i as i64, |indices| i64::from(indices[i]));
            let Some(slot) = usize::try_from(position).ok().and_then(|position| temperatures.get_mut(position)) else {
                return Err(MatcherError::InvalidArgument(format!(
                    "The index {position} is out of bounds for the temperatures tensor"
                )));
            };
            *slot = matcher.temperature().unwrap_or(-1.0);
        }
        Ok(())
    }
}
