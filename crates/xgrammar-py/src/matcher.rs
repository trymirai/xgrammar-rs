//! `GrammarMatcher` binding (non-tensor methods; the bitmask tensor API lands separately).

use crate::compiler::CompiledGrammar;

/// Drives constrained decoding over a compiled grammar.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    pub(crate) inner: xgrammar::matcher::GrammarMatcher,
}

#[bindings::export(Implementation)]
impl GrammarMatcher {
    /// Creates a matcher over a compiled grammar.
    ///
    /// `override_stop_tokens` and `max_rollback_tokens` are accepted for API parity; the
    /// rollback history is currently unbounded.
    #[bindings::export(Method(Factory))]
    pub fn new(
        compiled_grammar: CompiledGrammar,
        _override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
        _max_rollback_tokens: i32,
    ) -> GrammarMatcher {
        GrammarMatcher {
            inner: xgrammar::matcher::GrammarMatcher::from_compiled_grammar(
                &compiled_grammar.inner,
                terminate_without_stop_token,
            ),
        }
    }

    /// Accepts a single token id, advancing the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_token(
        &mut self,
        token_id: i32,
        _debug_print: bool,
    ) -> bool {
        self.inner.accept_token(token_id)
    }

    /// Accepts a UTF-8 string byte-by-byte. Returns whether the whole string was accepted.
    #[bindings::export(Method)]
    pub fn accept_string(
        &mut self,
        input_str: String,
        _debug_print: bool,
    ) -> bool {
        self.inner.accept_string(&input_str)
    }

    /// Whether the matcher has reached an accepting terminal state.
    #[bindings::export(Method)]
    pub fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }

    /// Whether the grammar is fully matched (root completed).
    #[bindings::export(Method)]
    pub fn is_completed(&self) -> bool {
        self.inner.is_completed()
    }

    /// Resets the matcher to its initial state.
    #[bindings::export(Method)]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Rolls back the last `num_tokens` accepted tokens.
    #[bindings::export(Method)]
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) {
        self.inner.rollback(num_tokens);
    }

    /// Returns a deep copy of the matcher at its current state.
    #[bindings::export(Method)]
    pub fn fork(&self) -> GrammarMatcher {
        GrammarMatcher {
            inner: self.inner.fork(),
        }
    }

    /// The stop token ids the matcher accepts as terminators.
    #[bindings::export(Method)]
    pub fn stop_token_ids(&self) -> Vec<i32> {
        self.inner.stop_token_ids().to_vec()
    }

    /// The string the matcher can deterministically jump forward by, if any.
    #[bindings::export(Method)]
    pub fn find_jump_forward_string(&mut self) -> Vec<u8> {
        self.inner.find_jump_forward_string()
    }
}
