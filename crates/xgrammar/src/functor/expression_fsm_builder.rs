//! Builds the automaton of one grammar expression into a target FSM (the C++
//! `GrammarFSMBuilderImpl` instance).

use super::{error::FunctorError, grammar_fsm_builder::GrammarFsmBuilder};
use crate::{
    fsm::{Fsm, FsmWithStartEnd, RegexFsmBuilder, SuffixAutomata},
    grammar::{Grammar, GrammarBuilder, GrammarExprType, TagDispatch},
};

/// Builds expressions into `target_fsm`, reading them through `exprs`. When `create_rules` is
/// set, large bounded regex repetitions become repeat edges to new rules added through `exprs`.
pub struct ExpressionFsmBuilder<'a> {
    target_fsm: Fsm,
    exprs: &'a mut GrammarBuilder,
    rule_name: Option<String>,
    create_rules: bool,
}

impl<'a> ExpressionFsmBuilder<'a> {
    /// Builds the automaton of the expression `expr_id` (simplified and minimized unless it is a
    /// tag dispatch, whose shape the parser relies on).
    ///
    /// # Errors
    /// Returns [`FunctorError::FsmBuild`] if a regex or tag dispatch fails to build.
    pub fn build_expression_fsm(
        exprs: &'a mut GrammarBuilder,
        expr_id: i32,
        rule_name: Option<String>,
        create_rules: bool,
    ) -> Result<FsmWithStartEnd, FunctorError> {
        let ty = exprs.grammar_expr(expr_id).ty;
        let result = Self::build(exprs, expr_id, rule_name, create_rules)?;
        if matches!(ty, GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch) {
            return Ok(result);
        }
        Ok(result.simplify_epsilon().merge_equivalent_states())
    }

    /// Builds the automaton of the expression `expr_id` as constructed, without simplifying it.
    ///
    /// # Errors
    /// Returns [`FunctorError::FsmBuild`] if a regex or tag dispatch fails to build.
    pub fn build(
        exprs: &'a mut GrammarBuilder,
        expr_id: i32,
        rule_name: Option<String>,
        create_rules: bool,
    ) -> Result<FsmWithStartEnd, FunctorError> {
        let mut builder = Self {
            target_fsm: Fsm::new(1),
            exprs,
            rule_name,
            create_rules,
        };
        let ends = builder.build_expression(expr_id, 0)?;
        Ok(FsmWithStartEnd::new(builder.target_fsm, 0, ends, false))
    }

    /// Builds the expression from `start`, returning its end states.
    fn build_expression(
        &mut self,
        expr_id: i32,
        start: i32,
    ) -> Result<Vec<i32>, FunctorError> {
        let (ty, data) = {
            let expr = self.exprs.grammar_expr(expr_id);
            (expr.ty, expr.data.to_vec())
        };
        let fsm = &mut self.target_fsm;
        Ok(match ty {
            GrammarExprType::EmptyStr => vec![start],
            GrammarExprType::ByteString => vec![GrammarFsmBuilder::build_byte_string(fsm, &data, start)],
            GrammarExprType::CharacterClass => {
                let end = fsm.add_state();
                GrammarFsmBuilder::add_character_class_transitions(fsm, &data, start, end);
                vec![end]
            },
            GrammarExprType::CharacterClassStar => {
                GrammarFsmBuilder::add_character_class_transitions(fsm, &data, start, start);
                vec![start]
            },
            GrammarExprType::RuleRef => {
                let end = fsm.add_state();
                fsm.add_rule_edge(start, end, data[0]);
                vec![end]
            },
            GrammarExprType::Repeat => {
                let end = fsm.add_state();
                fsm.add_repeat_edge(start, end, data[0], data[1], data[2]);
                vec![end]
            },
            GrammarExprType::Token => {
                let end = fsm.add_state();
                fsm.add_token_edge(start, end, &data);
                vec![end]
            },
            GrammarExprType::ExcludeToken => {
                let end = fsm.add_state();
                fsm.add_exclude_token_edge(start, end, &data);
                vec![end]
            },
            GrammarExprType::Sequence => self.build_sequence(&data, start)?,
            GrammarExprType::Choices => self.build_choices(&data, start)?,
            GrammarExprType::Regex => {
                let expr = self.exprs.grammar_expr(expr_id);
                let (pattern, json_string) = (expr.regex_pattern(), expr.regex_is_json_string());
                self.build_regex(&pattern, json_string, start)?
            },
            GrammarExprType::Substring => {
                let chunks = self.exprs.grammar_expr(expr_id).substring_chunks();
                self.append_fsm(SuffixAutomata::build(&chunks), start)
            },
            GrammarExprType::TagDispatch => {
                let tag_dispatch = TagDispatch::decode(&data, |id| self.exprs.grammar_expr(id));
                let fsm = GrammarFsmBuilder::tag_dispatch(&tag_dispatch)
                    .ok_or_else(|| FunctorError::FsmBuild("Failed to build tag dispatch FSM".to_owned()))?;
                self.append_fsm(fsm, start)
            },
            GrammarExprType::TokenTagDispatch => {
                let token_tag_dispatch = Grammar::decode_token_tag_dispatch_data(&data);
                GrammarFsmBuilder::build_token_tag_dispatch(fsm, &token_tag_dispatch, start)
            },
        })
    }

    fn build_sequence(
        &mut self,
        elements: &[i32],
        start: i32,
    ) -> Result<Vec<i32>, FunctorError> {
        let Some((&first, rest)) = elements.split_first() else {
            return Ok(vec![start]);
        };
        let mut end_states = self.build_expression(first, start)?;
        for &element in rest {
            let element_start = self.target_fsm.add_state();
            let next_end_states = self.build_expression(element, element_start)?;
            for &previous_end in &end_states {
                self.target_fsm.add_epsilon_edge(previous_end, element_start);
            }
            end_states = next_end_states;
        }
        Ok(end_states)
    }

    fn build_choices(
        &mut self,
        choices: &[i32],
        start: i32,
    ) -> Result<Vec<i32>, FunctorError> {
        let is_empty = |builder: &Self, choice: i32| builder.exprs.grammar_expr(choice).ty == GrammarExprType::EmptyStr;
        let nullable = choices.iter().any(|&choice| is_empty(self, choice));
        let non_empty: Vec<i32> = choices.iter().copied().filter(|&choice| !is_empty(self, choice)).collect();
        if non_empty.is_empty() {
            return Ok(vec![start]);
        }
        if non_empty.len() == 1 && !nullable {
            return self.build_expression(non_empty[0], start);
        }
        let mut end_states = Vec::new();
        for choice in non_empty {
            let branch_start = self.target_fsm.add_state();
            let branch_end_states = self.build_expression(choice, branch_start)?;
            self.target_fsm.add_epsilon_edge(start, branch_start);
            end_states.extend(branch_end_states);
        }
        if nullable {
            let nullable_branch = self.target_fsm.add_state();
            self.target_fsm.add_epsilon_edge(start, nullable_branch);
            end_states.push(nullable_branch);
        }
        Ok(end_states)
    }

    fn build_regex(
        &mut self,
        pattern: &[u8],
        json_string: bool,
        start: i32,
    ) -> Result<Vec<i32>, FunctorError> {
        let rule_hint = self.rule_name.clone().unwrap_or_default();
        let builder = if self.create_rules {
            Some(&mut *self.exprs)
        } else {
            None
        };
        let result = if json_string {
            RegexFsmBuilder::build_with_forbidden_chars(
                pattern,
                GrammarFsmBuilder::json_string_forbidden_chars(),
                builder,
                &rule_hint,
            )
        } else {
            RegexFsmBuilder::build(pattern, builder, &rule_hint)
        };
        let fsm = result.map_err(|error| {
            let regex = String::from_utf8_lossy(pattern);
            FunctorError::FsmBuild(match &self.rule_name {
                Some(rule_name) => {
                    format!("Failed to build the automaton for rule {rule_name} with regex {regex}: {error}")
                },
                None => format!("Failed to build the automaton for regex {regex}: {error}"),
            })
        })?;
        Ok(self.append_fsm(fsm, start))
    }

    /// Appends `fsm` after `start`, returning the mapped end states. An untouched one-state
    /// target is replaced by `fsm` directly.
    fn append_fsm(
        &mut self,
        fsm: FsmWithStartEnd,
        start: i32,
    ) -> Vec<i32> {
        let target = &mut self.target_fsm;
        let target_is_empty = target.num_states() == 1
            && start == 0
            && target.state_edges(0).is_empty()
            && target.edge_aux_data().is_empty()
            && fsm.start() == 0;
        if target_is_empty {
            let ends = fsm.ends().to_vec();
            *target = fsm.fsm().clone();
            return ends;
        }
        let offset = target.add_fsm(fsm.fsm());
        target.add_epsilon_edge(start, offset + fsm.start());
        fsm.ends().iter().map(|&end| offset + end).collect()
    }
}
