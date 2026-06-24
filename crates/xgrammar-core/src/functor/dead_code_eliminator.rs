//! Removes rules unreachable from the root — a port of `DeadCodeEliminator` and its
//! `UsedRulesAnalyzer` in `cpp/grammar_functor.cc`.

use std::collections::{BTreeSet, HashMap, VecDeque};

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// Drops rules that are unreachable from the root, remapping rule ids (the
/// `GrammarFunctor.dead_code_eliminator` pass).
#[must_use]
pub fn dead_code_eliminator(grammar: &Grammar) -> Grammar {
    let used = used_rules(grammar);
    let mut pass = DeadCodeEliminator {
        rule_id_map: HashMap::new(),
    };
    let mut state = MutatorState {
        base: grammar,
        builder: GrammarBuilder::new(),
        cur_rule_name: String::new(),
    };
    // Recreate the surviving rules (in ascending old-id order), recording old → new ids.
    for &old_id in &used {
        let new_id =
            state.builder.add_empty_rule(grammar.rule(old_id).name.clone());
        pass.rule_id_map.insert(old_id, new_id);
    }
    for &old_id in &used {
        let (body, lookahead, name) = {
            let rule = grammar.rule(old_id);
            (rule.body_expr_id, rule.lookahead_assertion_id, rule.name.clone())
        };
        state.cur_rule_name = name;
        let new_body = pass.visit_expr_id(&mut state, body);
        let new_id = pass.rule_id_map[&old_id];
        state.builder.update_rule_body(new_id, new_body);
        let new_lookahead = pass.visit_lookahead(&mut state, lookahead);
        state.builder.update_lookahead_assertion(new_id, new_lookahead);
    }
    let new_root = pass.rule_id_map[&grammar.root_rule_id()];
    state.builder.into_grammar_with_root_id(new_root)
}

struct DeadCodeEliminator {
    rule_id_map: HashMap<i32, i32>,
}

impl GrammarMutator for DeadCodeEliminator {
    fn visit_rule_ref(
        &mut self,
        state: &mut MutatorState,
        _ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        let new_id = self.rule_id_map[&data[0]];
        state.builder.add_rule_ref(new_id)
    }

    fn visit_repeat(
        &mut self,
        state: &mut MutatorState,
        _ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        let new_id = self.rule_id_map[&data[0]];
        state.builder.add_repeat(new_id, data[1], data[2])
    }
}

/// Returns, in ascending order, the ids of all rules reachable from the root.
fn used_rules(grammar: &Grammar) -> Vec<i32> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(grammar.root_rule_id());
    while let Some(rule_id) = queue.pop_front() {
        if !visited.insert(rule_id) {
            continue;
        }
        let (body, lookahead) = {
            let rule = grammar.rule(rule_id);
            (rule.body_expr_id, rule.lookahead_assertion_id)
        };
        collect_rule_refs(grammar, body, &mut queue);
        if lookahead != NO_EXPR {
            collect_rule_refs(grammar, lookahead, &mut queue);
        }
    }
    visited.into_iter().collect()
}

/// Pushes every rule referenced (directly or via nested sequence/choices) by `expr_id`.
fn collect_rule_refs(
    grammar: &Grammar,
    expr_id: i32,
    queue: &mut VecDeque<i32>,
) {
    let (ty, data) = {
        let expr = grammar.expr(expr_id);
        (expr.ty, expr.data.to_vec())
    };
    match ty {
        GrammarExprType::RuleRef | GrammarExprType::Repeat => {
            queue.push_back(data[0])
        },
        GrammarExprType::Sequence | GrammarExprType::Choices => {
            for &child in &data {
                collect_rule_refs(grammar, child, queue);
            }
        },
        GrammarExprType::TagDispatch => {
            for (_, rule_id) in grammar.tag_dispatch(expr_id).tag_rule_pairs {
                queue.push_back(rule_id);
            }
        },
        GrammarExprType::TokenTagDispatch => {
            for (_, rule_id) in
                grammar.token_tag_dispatch(expr_id).trigger_rule_pairs
            {
                queue.push_back(rule_id);
            }
        },
        _ => {},
    }
}
