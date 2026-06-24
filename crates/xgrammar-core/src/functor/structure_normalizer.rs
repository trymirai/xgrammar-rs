//! Normalizes grammar structure — a port of `StructureNormalizer` in
//! `cpp/grammar_functor.cc`.
//!
//! After the [`SingleElementExprEliminator`] pre-pass, every rule body is rewritten into
//! the normal form `rule ::= ("" | (e1 e2 …) | …)`: a choices of sequences of leaf
//! elements, where only the first choice may be the empty string. Nested choices in a
//! sequence are hoisted into fresh rules.

use super::mutator::GrammarMutator;
use super::single_element_expr_eliminator::SingleElementExprEliminator;
use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// Normalizes the structure of `grammar` (the `GrammarFunctor.structure_normalizer` pass).
#[must_use]
pub fn structure_normalizer(grammar: &Grammar) -> Grammar {
    StructureNormalizer::new(grammar).run()
}

struct StructureNormalizer {
    base: Grammar,
    builder: GrammarBuilder,
    cur_rule_name: String,
}

impl StructureNormalizer {
    fn new(grammar: &Grammar) -> Self {
        Self {
            base: SingleElementExprEliminator.apply(grammar),
            builder: GrammarBuilder::new(),
            cur_rule_name: String::new(),
        }
    }

    fn run(mut self) -> Grammar {
        let names: Vec<String> = self.base.rules().iter().map(|r| r.name.clone()).collect();
        for name in names {
            self.builder.add_empty_rule(name);
        }
        for i in 0..self.base.num_rules() {
            let (body_id, lookahead_id, name) = {
                let rule = self.base.rule(i);
                (rule.body_expr_id, rule.lookahead_assertion_id, rule.name.clone())
            };
            self.cur_rule_name = name;
            let new_body = self.visit_rule_body(body_id);
            self.builder.update_rule_body(i, new_body);
            let new_lookahead = self.visit_lookahead(lookahead_id);
            self.builder.update_lookahead_assertion(i, new_lookahead);
        }
        let root = self.base.root_rule().name.clone();
        self.builder
            .into_grammar(&root)
            .expect("root rule preserved during normalization")
    }

    /// Reads an expression's type and payload out of the source grammar.
    fn base_expr(&self, expr_id: i32) -> (GrammarExprType, Vec<i32>) {
        let expr = self.base.expr(expr_id);
        (expr.ty, expr.data.to_vec())
    }

    /// Reads a built expression's type and payload out of the in-progress builder.
    fn built_expr(&self, expr_id: i32) -> (GrammarExprType, Vec<i32>) {
        let expr = self.builder.grammar_expr(expr_id);
        (expr.ty, expr.data.to_vec())
    }

    /// Re-encodes a tag-dispatch (or token-tag-dispatch) payload into the result builder.
    fn rebuild_tag_dispatch(&mut self, ty: GrammarExprType, data: &[i32]) -> i32 {
        match ty {
            GrammarExprType::TagDispatch => {
                let tag_dispatch = self.base.decode_tag_dispatch_data(data);
                self.builder.add_tag_dispatch(&tag_dispatch)
            }
            GrammarExprType::TokenTagDispatch => {
                let ttd = Grammar::decode_token_tag_dispatch_data(data);
                self.builder.add_token_tag_dispatch(&ttd)
            }
            _ => unreachable!("rebuild_tag_dispatch called with non-tag-dispatch type"),
        }
    }

    fn visit_lookahead(&mut self, lookahead_id: i32) -> i32 {
        if lookahead_id == NO_EXPR {
            return NO_EXPR;
        }
        let (ty, data) = self.base_expr(lookahead_id);
        match ty {
            GrammarExprType::Sequence => {
                let ids = self.visit_sequence_(&data);
                self.builder.add_sequence(&ids)
            }
            GrammarExprType::Choices => {
                panic!("Choices in lookahead assertion are not supported yet")
            }
            GrammarExprType::EmptyStr => panic!("Empty string should not be in lookahead assertion"),
            GrammarExprType::TagDispatch => panic!("TagDispatch should not be in lookahead assertion"),
            _ => {
                let element = self.builder.add_grammar_expr(ty, &data);
                self.builder.add_sequence(&[element])
            }
        }
    }

    fn visit_rule_body(&mut self, expr_id: i32) -> i32 {
        let (ty, data) = self.base_expr(expr_id);
        match ty {
            GrammarExprType::Sequence => {
                let ids = self.visit_sequence_(&data);
                let seq = self.builder.add_sequence(&ids);
                self.builder.add_choices(&[seq])
            }
            GrammarExprType::Choices => {
                let ids = self.visit_choices_(&data);
                self.builder.add_choices(&ids)
            }
            GrammarExprType::EmptyStr => {
                let empty = self.builder.add_empty_str();
                self.builder.add_choices(&[empty])
            }
            GrammarExprType::ByteString
            | GrammarExprType::CharacterClass
            | GrammarExprType::CharacterClassStar
            | GrammarExprType::RuleRef
            | GrammarExprType::Repeat
            | GrammarExprType::Token
            | GrammarExprType::ExcludeToken => {
                let element = self.builder.add_grammar_expr(ty, &data);
                let seq = self.builder.add_sequence(&[element]);
                self.builder.add_choices(&[seq])
            }
            // A tag dispatch is kept as the rule body directly (printed without the
            // `(( … ))` choices/sequence wrapping).
            GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch => {
                self.rebuild_tag_dispatch(ty, &data)
            }
        }
    }

    /// Returns the new choice ids for a choices expression (flattening nested choices and
    /// folding empty strings to a single leading empty choice).
    fn visit_choices_(&mut self, data: &[i32]) -> Vec<i32> {
        let mut new_choice_ids = Vec::new();
        let mut found_empty = false;
        for &choice in data {
            let (ty, cdata) = self.base_expr(choice);
            match ty {
                GrammarExprType::Sequence => {
                    let sub = self.visit_sequence_(&cdata);
                    if sub.is_empty() {
                        found_empty = true;
                    } else {
                        let seq = self.builder.add_sequence(&sub);
                        new_choice_ids.push(seq);
                    }
                }
                GrammarExprType::Choices => {
                    let sub = self.visit_choices_(&cdata);
                    let first_is_empty =
                        self.built_expr(sub[0]).0 == GrammarExprType::EmptyStr;
                    if first_is_empty {
                        found_empty = true;
                        new_choice_ids.extend_from_slice(&sub[1..]);
                    } else {
                        new_choice_ids.extend_from_slice(&sub);
                    }
                }
                GrammarExprType::EmptyStr => found_empty = true,
                GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch => {
                    let element = self.rebuild_tag_dispatch(ty, &cdata);
                    let rule_id = self.builder.add_rule_with_hint(&self.cur_rule_name.clone(), element);
                    let rule_ref = self.builder.add_rule_ref(rule_id);
                    let seq = self.builder.add_sequence(&[rule_ref]);
                    new_choice_ids.push(seq);
                }
                _ => {
                    let element = self.builder.add_grammar_expr(ty, &cdata);
                    let seq = self.builder.add_sequence(&[element]);
                    new_choice_ids.push(seq);
                }
            }
        }
        if found_empty {
            let empty = self.builder.add_empty_str();
            new_choice_ids.insert(0, empty);
        }
        assert!(!new_choice_ids.is_empty(), "choices must be non-empty");
        new_choice_ids
    }

    /// Returns the new element ids for a sequence expression (flattening nested sequences,
    /// dropping empty strings, and hoisting nested choices into fresh rules).
    fn visit_sequence_(&mut self, data: &[i32]) -> Vec<i32> {
        let mut new_sequence_ids = Vec::new();
        for &element in data {
            let (ty, edata) = self.base_expr(element);
            match ty {
                GrammarExprType::Sequence => {
                    let sub = self.visit_sequence_(&edata);
                    new_sequence_ids.extend_from_slice(&sub);
                }
                GrammarExprType::Choices => {
                    let sub = self.visit_choices_(&edata);
                    if sub.len() == 1 {
                        let (sub_ty, sub_data) = self.built_expr(sub[0]);
                        if sub_ty != GrammarExprType::EmptyStr {
                            new_sequence_ids.extend_from_slice(&sub_data);
                        }
                    } else {
                        let choices = self.builder.add_choices(&sub);
                        let rule_id =
                            self.builder.add_rule_with_hint(&self.cur_rule_name.clone(), choices);
                        new_sequence_ids.push(self.builder.add_rule_ref(rule_id));
                    }
                }
                GrammarExprType::EmptyStr => {}
                GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch => {
                    let element_id = self.rebuild_tag_dispatch(ty, &edata);
                    let rule_id =
                        self.builder.add_rule_with_hint(&self.cur_rule_name.clone(), element_id);
                    new_sequence_ids.push(self.builder.add_rule_ref(rule_id));
                }
                _ => new_sequence_ids.push(self.builder.add_grammar_expr(ty, &edata)),
            }
        }
        new_sequence_ids
    }
}
