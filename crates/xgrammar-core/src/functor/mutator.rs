//! The grammar visitor/mutator framework — a port of `GrammarFunctor` in
//! `cpp/grammar_functor.h`.
//!
//! A mutator walks every rule and rebuilds the grammar into a fresh [`GrammarBuilder`],
//! dispatching on each expression's type. The defaults form an identity transform; a pass
//! overrides the `visit_*` methods it cares about. State is threaded explicitly via
//! [`MutatorState`], and each `visit_*` receives the payload copied out of the source,
//! which avoids borrow conflicts between reading `base` and writing `builder`.

use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// Traversal state: the source grammar, the builder for the result, and the rule being
/// rewritten (a hint for generated rule names).
pub struct MutatorState<'a> {
    /// The grammar being transformed (read-only source).
    pub base: &'a Grammar,
    /// The builder accumulating the result.
    pub builder: GrammarBuilder,
    /// Name of the rule currently being rewritten.
    pub cur_rule_name: String,
}

/// A grammar transformation. Override the `visit_*` methods you need; the defaults rebuild
/// each expression unchanged.
pub trait GrammarMutator {
    /// Rebuilds the grammar. Rules are created empty first (preserving rule ids), then each
    /// body and lookahead assertion is visited.
    fn apply(&mut self, grammar: &Grammar) -> Grammar {
        let mut state = MutatorState {
            base: grammar,
            builder: GrammarBuilder::new(),
            cur_rule_name: String::new(),
        };
        for rule in grammar.rules() {
            state.builder.add_empty_rule(rule.name.clone());
        }
        for i in 0..grammar.num_rules() {
            let rule = grammar.rule(i);
            state.cur_rule_name = rule.name.clone();
            let (body, lookahead) = (rule.body_expr_id, rule.lookahead_assertion_id);
            let new_body = self.visit_expr_id(&mut state, body);
            state.builder.update_rule_body(i, new_body);
            let new_lookahead = self.visit_lookahead(&mut state, lookahead);
            state.builder.update_lookahead_assertion(i, new_lookahead);
        }
        let root = grammar.root_rule().name.clone();
        state
            .builder
            .into_grammar(&root)
            .expect("root rule preserved during mutation")
    }

    /// Visits a lookahead assertion, passing [`NO_EXPR`] through.
    fn visit_lookahead(&mut self, state: &mut MutatorState, lookahead_id: i32) -> i32 {
        if lookahead_id == NO_EXPR {
            NO_EXPR
        } else {
            self.visit_expr_id(state, lookahead_id)
        }
    }

    /// Visits the source expression with the given id.
    fn visit_expr_id(&mut self, state: &mut MutatorState, expr_id: i32) -> i32 {
        let (ty, data) = {
            let expr = state.base.expr(expr_id);
            (expr.ty, expr.data.to_vec())
        };
        self.visit_expr(state, ty, &data)
    }

    /// Dispatches on the expression type.
    fn visit_expr(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        match ty {
            GrammarExprType::Sequence => self.visit_sequence(state, data),
            GrammarExprType::Choices => self.visit_choices(state, data),
            GrammarExprType::EmptyStr => self.visit_empty_str(state, ty, data),
            GrammarExprType::ByteString => self.visit_byte_string(state, ty, data),
            GrammarExprType::CharacterClass => self.visit_character_class(state, ty, data),
            GrammarExprType::CharacterClassStar => self.visit_character_class_star(state, ty, data),
            GrammarExprType::RuleRef => self.visit_rule_ref(state, ty, data),
            GrammarExprType::Repeat => self.visit_repeat(state, ty, data),
            GrammarExprType::Token => self.visit_token(state, ty, data),
            GrammarExprType::ExcludeToken => self.visit_exclude_token(state, ty, data),
            GrammarExprType::TagDispatch => self.visit_tag_dispatch(state, ty, data),
            GrammarExprType::TokenTagDispatch => self.visit_token_tag_dispatch(state, ty, data),
        }
    }

    /// Rebuilds a sequence, visiting each element.
    fn visit_sequence(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut ids = Vec::with_capacity(data.len());
        for &child in data {
            ids.push(self.visit_expr_id(state, child));
        }
        state.builder.add_sequence(&ids)
    }

    /// Rebuilds an alternation, visiting each choice.
    fn visit_choices(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut ids = Vec::with_capacity(data.len());
        for &child in data {
            ids.push(self.visit_expr_id(state, child));
        }
        state.builder.add_choices(&ids)
    }

    /// Re-adds a leaf expression unchanged. The per-leaf hooks below default to this.
    fn visit_element(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        state.builder.add_grammar_expr(ty, data)
    }

    #[doc(hidden)]
    fn visit_empty_str(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_byte_string(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_character_class(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_character_class_star(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_rule_ref(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_repeat(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_token(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }
    #[doc(hidden)]
    fn visit_exclude_token(&mut self, st: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(st, ty, data)
    }

    // Tag-dispatch payloads embed byte-string and excludes expression ids that point into
    // the source grammar, so they are decoded and re-encoded into the fresh builder rather
    // than copied raw. Rule ids are preserved by the rule-ordering invariant of `apply`.
    #[doc(hidden)]
    fn visit_tag_dispatch(&mut self, st: &mut MutatorState, _ty: GrammarExprType, data: &[i32]) -> i32 {
        let tag_dispatch = st.base.decode_tag_dispatch_data(data);
        st.builder.add_tag_dispatch(&tag_dispatch)
    }
    #[doc(hidden)]
    fn visit_token_tag_dispatch(&mut self, st: &mut MutatorState, _ty: GrammarExprType, data: &[i32]) -> i32 {
        let ttd = Grammar::decode_token_tag_dispatch_data(data);
        st.builder.add_token_tag_dispatch(&ttd)
    }
}
