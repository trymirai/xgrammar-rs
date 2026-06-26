//! Infers lookahead assertions for rules — a port of `LookaheadAssertionAnalyzer` in
//! `cpp/grammar_functor.cc`.
//!
//! When a rule is referenced in exactly one place where it is not the final element of a
//! sequence, the text that must follow it is fixed; that suffix is attached to the rule as
//! a lookahead assertion so the matcher can resolve completion earlier.

use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// Infers and attaches lookahead assertions (the `GrammarFunctor.lookahead_assertion_analyzer`
/// pass).
#[must_use]
pub fn lookahead_assertion_analyzer(grammar: &Grammar) -> Grammar {
    LookaheadAssertionAnalyzer::new(grammar).run()
}

#[derive(Default)]
struct RuleLookaheadInfo {
    is_triggered_by_dispatch: bool,
    appears_as_last_in_other_rule: bool,
    non_last_occurrence_count: i32,
    suffix_after_first_occurrence: Vec<i32>,
}

struct LookaheadAssertionAnalyzer<'a> {
    base: &'a Grammar,
    builder: GrammarBuilder,
    infos: Vec<RuleLookaheadInfo>,
}

impl<'a> LookaheadAssertionAnalyzer<'a> {
    fn new(grammar: &'a Grammar) -> Self {
        Self {
            base: grammar,
            builder: GrammarBuilder::from_grammar(grammar),
            infos: Vec::new(),
        }
    }

    fn run(mut self) -> Grammar {
        let root_body = self.base.expr(self.base.root_rule().body_expr_id);
        if matches!(
            root_body.ty,
            GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch
        ) {
            return self.base.clone();
        }

        self.build_rule_lookahead_info();

        let root_id = self.base.root_rule_id();
        for i in 0..self.base.num_rules() {
            if i == root_id {
                continue;
            }
            if self.base.rule(i).lookahead_assertion_id != NO_EXPR {
                let exact = self.can_use_derived_lookahead(i);
                self.builder.update_lookahead_exact(i, exact);
                continue;
            }
            if let Some(lookahead_id) = self.detect_lookahead_assertion(i) {
                self.builder.update_lookahead_assertion(i, lookahead_id);
                self.builder.update_lookahead_exact(i, true);
            }
        }
        self.builder.into_grammar_with_root_id(root_id)
    }

    fn can_use_derived_lookahead(
        &self,
        rule_id: i32,
    ) -> bool {
        let info = &self.infos[rule_id as usize];
        !info.is_triggered_by_dispatch
            && !info.appears_as_last_in_other_rule
            && info.non_last_occurrence_count == 1
    }

    fn detect_lookahead_assertion(
        &mut self,
        rule_id: i32,
    ) -> Option<i32> {
        if !self.can_use_derived_lookahead(rule_id) {
            return None;
        }
        let suffix =
            self.infos[rule_id as usize].suffix_after_first_occurrence.clone();
        Some(self.builder.add_sequence(&suffix))
    }

    fn build_rule_lookahead_info(&mut self) {
        let num_rules = self.base.num_rules();
        self.infos =
            (0..num_rules).map(|_| RuleLookaheadInfo::default()).collect();

        for i in 0..num_rules {
            let body = self.base.expr(self.base.rule(i).body_expr_id);
            match body.ty {
                GrammarExprType::TagDispatch => {
                    for (_, rule_id) in self
                        .base
                        .tag_dispatch(self.base.rule(i).body_expr_id)
                        .tag_rule_pairs
                    {
                        self.infos[rule_id as usize].is_triggered_by_dispatch =
                            true;
                    }
                    continue;
                },
                GrammarExprType::TokenTagDispatch => {
                    for (_, rule_id) in self
                        .base
                        .token_tag_dispatch(self.base.rule(i).body_expr_id)
                        .trigger_rule_pairs
                    {
                        self.infos[rule_id as usize].is_triggered_by_dispatch =
                            true;
                    }
                    continue;
                },
                _ => {},
            }
            // Otherwise the body is a choices of sequences.
            let choices: Vec<i32> = body.data.to_vec();
            for sequence_id in choices {
                let seq = self.base.expr(sequence_id);
                if seq.ty != GrammarExprType::Sequence || seq.data.is_empty() {
                    continue;
                }
                let elements: Vec<i32> = seq.data.to_vec();
                let last = self.base.expr(*elements.last().unwrap());
                if last.ty == GrammarExprType::RuleRef
                    && i != last.rule_ref_id()
                {
                    self.infos[last.rule_ref_id() as usize]
                        .appears_as_last_in_other_rule = true;
                }
                for j in 0..elements.len() - 1 {
                    let element = self.base.expr(elements[j]);
                    if element.ty != GrammarExprType::RuleRef {
                        continue;
                    }
                    let info = &mut self.infos[element.rule_ref_id() as usize];
                    if info.non_last_occurrence_count == 0 {
                        info.suffix_after_first_occurrence =
                            elements[j + 1..].to_vec();
                    }
                    info.non_last_occurrence_count += 1;
                }
            }
        }
    }
}
