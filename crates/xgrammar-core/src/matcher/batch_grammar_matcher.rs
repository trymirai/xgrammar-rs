//! Batched matcher operations — a port of `BatchGrammarMatcher` in `cpp/grammar_matcher.cc`.
//!
//! Each method applies the corresponding [`GrammarMatcher`] operation across a slice of
//! matchers, preserving per-matcher order. (The C++ parallelizes via a thread pool; that is a
//! performance optimization deferred to the perf phase — the results are order-identical.)

use super::grammar_matcher::GrammarMatcher;

/// Stateless batched front end over a slice of [`GrammarMatcher`]s.
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchGrammarMatcher;

impl BatchGrammarMatcher {
    /// Accepts `token_ids[i]` into `matchers[i]`, returning per-matcher success.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_accept_token(
        matchers: &mut [GrammarMatcher],
        token_ids: &[i32],
    ) -> Vec<bool> {
        assert_eq!(
            matchers.len(),
            token_ids.len(),
            "matchers and token_ids length mismatch"
        );
        matchers
            .iter_mut()
            .zip(token_ids)
            .map(|(m, &t)| m.accept_token(t))
            .collect()
    }

    /// Accepts `inputs[i]` (bytes) into `matchers[i]`, returning per-matcher success.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_accept_string(
        matchers: &mut [GrammarMatcher],
        inputs: &[&[u8]],
    ) -> Vec<bool> {
        assert_eq!(
            matchers.len(),
            inputs.len(),
            "matchers and inputs length mismatch"
        );
        matchers
            .iter_mut()
            .zip(inputs)
            .map(|(m, inp)| m.accept_bytes(inp))
            .collect()
    }

    /// Rolls each `matchers[i]` back by `num_tokens[i]`.
    ///
    /// # Panics
    /// Panics if the slice lengths differ.
    pub fn batch_rollback(
        matchers: &mut [GrammarMatcher],
        num_tokens: &[i32],
    ) {
        assert_eq!(
            matchers.len(),
            num_tokens.len(),
            "matchers and num_tokens length mismatch"
        );
        for (m, &n) in matchers.iter_mut().zip(num_tokens) {
            m.rollback(n);
        }
    }

    /// Fills `bitmask` with one row per matcher: `matchers[i]` writes row `indices[i]` (or row
    /// `i` if `indices` is `None`).
    pub fn batch_fill_next_token_bitmask(
        matchers: &mut [GrammarMatcher],
        bitmask: &mut [i32],
        indices: Option<&[i32]>,
    ) {
        for (i, m) in matchers.iter_mut().enumerate() {
            let index = indices.map_or(i as i32, |idx| idx[i]);
            m.fill_next_token_bitmask(bitmask, index);
        }
    }
}
