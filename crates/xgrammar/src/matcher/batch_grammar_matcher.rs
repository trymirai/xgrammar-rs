//! Batched matcher operations for processing multiple [`GrammarMatcher`]s in parallel.

use super::{grammar_matcher::GrammarMatcher, matcher_error::MatcherTerminatedError};

/// A batched version of [`GrammarMatcher`] for better efficiency.
///
/// This class supports batch processing of multiple [`GrammarMatcher`] objects in parallel.
/// It provides batched versions of the core methods of [`GrammarMatcher`], including
/// [`Self::batch_fill_next_token_bitmask`], [`Self::batch_accept_string`], and
/// [`Self::batch_accept_token`]. It utilizes multi-threading to process multiple
/// [`GrammarMatcher`] objects simultaneously, significantly improving efficiency when dealing
/// with a large number of matchers.
///
/// `max_threads` is retained for API parity; the core batch helpers currently run sequentially.
/// The Python binding parallelizes `batch_fill_next_token_bitmask` with rayon.
#[derive(Debug, Clone, Copy)]
pub struct BatchGrammarMatcher {
    max_threads: i32,
}

impl Default for BatchGrammarMatcher {
    fn default() -> Self {
        Self::new(-1)
    }
}

impl BatchGrammarMatcher {
    /// Creates a batch matcher. `max_threads <= 0` means "use hardware concurrency"
    /// at the binding layer; the core sequential helpers ignore it.
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

    /// A batched version of [`GrammarMatcher::accept_token`] for better efficiency.
    ///
    /// `matchers` is the array of [`GrammarMatcher`] objects. `token_ids` is the array of token
    /// ids to be accepted. Returns a vector indicating whether each token is accepted.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_accept_token(
        matchers: &mut [GrammarMatcher],
        token_ids: &[i32],
    ) -> Vec<bool> {
        assert_eq!(matchers.len(), token_ids.len(), "matchers and token_ids length mismatch");
        matchers.iter_mut().zip(token_ids).map(|(m, &t)| m.accept_token(t)).collect()
    }

    /// A batched version of [`GrammarMatcher::accept_string`] for better efficiency.
    ///
    /// `matchers` is the array of [`GrammarMatcher`] objects. `inputs` is the array of input
    /// strings to be accepted. Returns a vector indicating whether each string is accepted.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_accept_string(
        matchers: &mut [GrammarMatcher],
        inputs: &[&[u8]],
    ) -> Vec<bool> {
        assert_eq!(matchers.len(), inputs.len(), "matchers and inputs length mismatch");
        matchers.iter_mut().zip(inputs).map(|(m, inp)| m.accept_bytes(inp)).collect()
    }

    /// A batched version of [`GrammarMatcher::rollback`] for better efficiency.
    ///
    /// `matchers` is the array of [`GrammarMatcher`] objects. `num_tokens` is the array of the
    /// number of tokens to rollback for each matcher.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_rollback(
        matchers: &mut [GrammarMatcher],
        num_tokens: &[i32],
    ) {
        assert_eq!(matchers.len(), num_tokens.len(), "matchers and num_tokens length mismatch");
        for (m, &n) in matchers.iter_mut().zip(num_tokens) {
            m.rollback(n);
        }
    }

    /// A batched version of [`GrammarMatcher::fill_next_token_bitmask`] for better efficiency.
    ///
    /// `matchers` is the array of [`GrammarMatcher`] objects. `bitmask` is the pre-allocated
    /// buffer to store the result bitmasks. `indices` optionally specifies which matcher
    /// corresponds to which slice of the bitmask tensor. If not provided, all matchers write to
    /// the corresponding indices (`matchers[i]` to `bitmask[i]`).
    ///
    /// # Errors
    /// Returns [`MatcherTerminatedError`] if any matcher has accepted the stop token.
    pub fn batch_fill_next_token_bitmask(
        matchers: &mut [GrammarMatcher],
        bitmask: &mut [i32],
        indices: Option<&[i32]>,
    ) -> Result<(), MatcherTerminatedError> {
        for (i, m) in matchers.iter_mut().enumerate() {
            let index = indices.map_or(i as i32, |idx| idx[i]);
            m.fill_next_token_bitmask(bitmask, index)?;
        }
        Ok(())
    }
}
