//! The grammar visitor/mutator framework — a port of `GrammarFunctor` in
//! `cpp/grammar_functor.h`.
//!
//! A *mutator* walks every rule of a grammar and rebuilds it into a fresh
//! [`GrammarBuilder`], dispatching on each expression's type. The default methods form an
//! **identity** transform (the output is structurally equivalent to the input); a concrete
//! pass overrides the `visit_*` methods it cares about and inherits the rest.
//!
//! Unlike the C++ CRTP version, the traversal state ([`MutatorState`]) is threaded
//! explicitly so a pass can hold its own `&mut self` state (caches, counters). Each
//! `visit_*` receives the expression's payload copied out of the source grammar, which
//! sidesteps borrow conflicts between reading `base` and writing `builder`.

use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// The traversal state shared across a mutation: the source grammar, the builder for the
/// result, and the name of the rule currently being rewritten.
pub struct MutatorState<'a> {
    /// The grammar being transformed (read-only source).
    pub base: &'a Grammar,
    /// The builder accumulating the transformed grammar.
    pub builder: GrammarBuilder,
    /// Name of the rule currently being rewritten (a hint for generated rule names).
    pub cur_rule_name: String,
}

/// A grammar transformation. Implementors override the `visit_*` methods they need; the
/// defaults rebuild each expression unchanged.
pub trait GrammarMutator {
    /// Applies the transformation, returning the rebuilt grammar.
    ///
    /// Rules are first created empty (preserving rule ids), then each body and lookahead
    /// assertion is visited and rewritten.
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

    /// Visits a lookahead assertion (or passes through [`NO_EXPR`]).
    fn visit_lookahead(&mut self, state: &mut MutatorState, lookahead_id: i32) -> i32 {
        if lookahead_id == NO_EXPR {
            NO_EXPR
        } else {
            self.visit_expr_id(state, lookahead_id)
        }
    }

    /// Visits the expression with the given id in the source grammar.
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

    /// Re-adds a leaf expression (no nested expression ids) unchanged.
    fn visit_element(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        state.builder.add_grammar_expr(ty, data)
    }

    /// Visits an empty-string expression (default: re-add unchanged).
    fn visit_empty_str(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a byte-string expression (default: re-add unchanged).
    fn visit_byte_string(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a character-class expression (default: re-add unchanged).
    fn visit_character_class(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a starred character-class expression (default: re-add unchanged).
    fn visit_character_class_star(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a rule reference (default: re-add unchanged; rule ids are preserved).
    fn visit_rule_ref(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a repeat expression (default: re-add unchanged).
    fn visit_repeat(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a token-set expression (default: re-add unchanged).
    fn visit_token(&mut self, state: &mut MutatorState, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits an exclude-token-set expression (default: re-add unchanged).
    fn visit_exclude_token(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a tag-dispatch expression.
    ///
    /// The default re-adds the raw payload, which is only correct while expression ids are
    /// preserved; proper re-encoding lands with the tag-dispatch builder support (the
    /// macro work). Not exercised by the current non-tag passes.
    fn visit_tag_dispatch(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.visit_element(state, ty, data)
    }

    /// Visits a token-tag-dispatch expression (see [`Self::visit_tag_dispatch`]).
    fn visit_token_tag_dispatch(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.visit_element(state, ty, data)
    }
}
