//! Lowers the regular subset of grammars (e.g. from `RegexToEBNF`) into one automaton without
//! changing terminal semantics.
//!
//! A shared callee entry/exit would let one call return to another call's continuation. Each
//! finite call is inlined instead, and only direct tail self-references connect to that call's own
//! entry. The entry is private even when the caller's start also has an optional exit.

use super::grammar_fsm_builder::GrammarFsmBuilder;
use crate::{
    fsm::{Fsm, FsmWithStartEnd},
    grammar::{Grammar, GrammarExpr, GrammarExprType},
};

/// The deepest chain of inlined rule calls.
const MAX_RULE_CALL_DEPTH: usize = 256;

/// Flattens a regular grammar into one FSM of bounded size (the C++ `RegularGrammarFSMBuilder`).
pub struct RegularGrammarFsmBuilder<'a> {
    grammar: &'a Grammar,
    max_num_states: i32,
    fsm: Fsm,
    active: Vec<bool>,
    depth: usize,
}

impl<'a> RegularGrammarFsmBuilder<'a> {
    /// Flattens `grammar` from its root rule into a simplified, minimized automaton whose
    /// intermediate FSM has at most `max_num_states` states.
    ///
    /// # Errors
    /// Returns why the grammar is not a plain regular grammar or exceeds the state limit.
    pub fn build(
        grammar: &'a Grammar,
        max_num_states: i32,
    ) -> Result<FsmWithStartEnd, String> {
        if max_num_states < 2 {
            return Err("Regular grammar FSM state limit must be at least 2".to_owned());
        }
        let mut builder = Self {
            grammar,
            max_num_states,
            fsm: Fsm::new(2),
            active: vec![false; grammar.num_rules() as usize],
            depth: 0,
        };
        builder
            .build_rule(grammar.root_rule_id(), 0, 1)
            .map_err(|message| format!("Cannot flatten regular grammar: {message}"))?;
        let result = FsmWithStartEnd::new(builder.fsm, 0, vec![1], false).simplify_epsilon();
        Ok(result.merge_equivalent_states())
    }

    fn state_limit_error(&self) -> String {
        format!("intermediate FSM exceeds {} states", self.max_num_states)
    }

    fn add_state(&mut self) -> Result<i32, String> {
        if self.fsm.num_states() >= self.max_num_states {
            return Err(self.state_limit_error());
        }
        Ok(self.fsm.add_state())
    }

    fn append(
        &mut self,
        leaf: &FsmWithStartEnd,
        start: i32,
        end: i32,
    ) -> Result<(), String> {
        if leaf.fsm().num_states() > self.max_num_states - self.fsm.num_states() {
            return Err(self.state_limit_error());
        }
        let offset = self.fsm.add_fsm(leaf.fsm());
        self.fsm.add_epsilon_edge(start, offset + leaf.start());
        for &leaf_end in leaf.ends() {
            self.fsm.add_epsilon_edge(offset + leaf_end, end);
        }
        Ok(())
    }

    fn build_rule(
        &mut self,
        rule_id: i32,
        start: i32,
        end: i32,
    ) -> Result<(), String> {
        let grammar = self.grammar;
        let rule = grammar.rule(rule_id);
        if self.active[rule_id as usize] {
            return Err(format!("mutual or non-tail recursion involving rule {}", rule.name));
        }
        if self.depth >= MAX_RULE_CALL_DEPTH {
            return Err(format!("rule call depth exceeds {MAX_RULE_CALL_DEPTH}"));
        }
        if rule.lookahead_assertion_id >= 0
            || rule.max_tokens >= 0
            || rule.max_chars >= 0
            || !rule.capture_name.is_empty()
            || rule.is_lazy
            || rule.temperature.is_some()
            || grammar.suffix_stop_info(rule_id).is_some()
        {
            return Err(format!("annotated rule {} is not a plain regular rule", rule.name));
        }
        let entry = self.add_state()?;
        self.fsm.add_epsilon_edge(start, entry);
        self.active[rule_id as usize] = true;
        self.depth += 1;
        let result = self.build_expression(&grammar.expr(rule.body_expr_id), entry, end, rule_id, entry, true);
        self.depth -= 1;
        self.active[rule_id as usize] = false;
        result
    }

    fn build_expression(
        &mut self,
        expr: &GrammarExpr<'a>,
        start: i32,
        end: i32,
        rule_id: i32,
        rule_start: i32,
        tail: bool,
    ) -> Result<(), String> {
        let grammar = self.grammar;
        match expr.ty {
            GrammarExprType::EmptyStr => self.fsm.add_epsilon_edge(start, end),
            GrammarExprType::ByteString => {
                if expr.data.len() as i32 >= self.max_num_states - self.fsm.num_states() {
                    return Err("byte string exceeds the intermediate FSM state limit".to_owned());
                }
                self.append(&GrammarFsmBuilder::byte_string(expr), start, end)?;
            },
            GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar => {
                self.append(&GrammarFsmBuilder::character_class(expr), start, end)?;
            },
            GrammarExprType::RuleRef if expr.data[0] == rule_id => {
                if !tail {
                    return Err(format!("non-tail self-recursion in rule {}", grammar.rule(rule_id).name));
                }
                self.fsm.add_epsilon_edge(start, rule_start);
            },
            GrammarExprType::RuleRef => self.build_rule(expr.data[0], start, end)?,
            GrammarExprType::Choices => {
                for &choice in expr.data {
                    self.build_expression(&grammar.expr(choice), start, end, rule_id, rule_start, tail)?;
                }
            },
            GrammarExprType::Sequence => {
                if expr.data.is_empty() {
                    self.fsm.add_epsilon_edge(start, end);
                }
                let mut current = start;
                for (index, &element) in expr.data.iter().enumerate() {
                    let is_last = index + 1 == expr.data.len();
                    let next = if is_last {
                        end
                    } else {
                        self.add_state()?
                    };
                    self.build_expression(&grammar.expr(element), current, next, rule_id, rule_start, tail && is_last)?;
                    current = next;
                }
            },
            GrammarExprType::Repeat => {
                let (callee, lower, upper) = expr.repeat();
                if lower < 0 || (upper != -1 && upper < lower) {
                    return Err("invalid repetition bounds".to_owned());
                }
                // Even empty callees must not make an enormous explicit repetition consume
                // unbounded compilation time.
                let count = if upper == -1 {
                    lower
                } else {
                    upper
                };
                if count > self.max_num_states {
                    return Err("repetition exceeds the intermediate FSM state limit".to_owned());
                }
                let mut current = start;
                for index in 0..=count {
                    if index >= lower {
                        self.fsm.add_epsilon_edge(current, end);
                    }
                    if index == count {
                        break;
                    }
                    let next = self.add_state()?;
                    self.build_rule(callee, current, next)?;
                    current = next;
                }
                if upper == -1 {
                    self.build_rule(callee, current, current)?;
                }
            },
            _ => {
                return Err(format!("unsupported terminal or expression in rule {}", grammar.rule(rule_id).name));
            },
        }
        Ok(())
    }
}
