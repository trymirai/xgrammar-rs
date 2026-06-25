//! Determines which rules can match the empty string — a port of `AllowEmptyRuleAnalyzer`
//! (and its helper `RuleRefGraphFinder`) in `cpp/grammar_functor.cc`.

use std::collections::{HashSet, VecDeque};

use crate::grammar::{Grammar, GrammarExprType};

/// Returns the sorted ids of rules that can derive the empty string.
#[must_use]
pub fn allow_empty_rule_ids(grammar: &Grammar) -> Vec<i32> {
    let mut empty: HashSet<i32> = HashSet::new();
    find_explicit_empty_rules(grammar, &mut empty);
    let graph = rule_ref_graph(grammar);
    find_indirect_empty_rules(grammar, &mut empty, &graph);
    let mut result: Vec<i32> = empty.into_iter().collect();
    result.sort_unstable();
    result
}

/// Builds the inverse reference graph: `graph[referee] = [referrers...]`.
fn rule_ref_graph(grammar: &Grammar) -> Vec<Vec<i32>> {
    let n = grammar.num_rules();
    let mut graph: Vec<Vec<i32>> = vec![Vec::new(); n as usize];
    for cur in 0..n {
        let body_id = grammar.rule(cur).body_expr_id;
        visit_refs(grammar, body_id, cur, &mut graph);
    }
    for refs in &mut graph {
        refs.sort_unstable();
        refs.dedup();
    }
    graph
}

fn visit_refs(
    grammar: &Grammar,
    expr_id: i32,
    cur_rule: i32,
    graph: &mut [Vec<i32>],
) {
    let expr = grammar.expr(expr_id);
    match expr.ty {
        GrammarExprType::Choices | GrammarExprType::Sequence => {
            for &child in expr.data {
                visit_refs(grammar, child, cur_rule, graph);
            }
        },
        GrammarExprType::RuleRef | GrammarExprType::Repeat => {
            graph[expr.data[0] as usize].push(cur_rule);
        },
        GrammarExprType::TagDispatch => {
            for (_, rule_id) in grammar.tag_dispatch(expr_id).tag_rule_pairs {
                graph[rule_id as usize].push(cur_rule);
            }
        },
        GrammarExprType::TokenTagDispatch => {
            for (_, rule_id) in
                grammar.token_tag_dispatch(expr_id).trigger_rule_pairs
            {
                graph[rule_id as usize].push(cur_rule);
            }
        },
        _ => {},
    }
}

fn find_explicit_empty_rules(
    grammar: &Grammar,
    empty: &mut HashSet<i32>,
) {
    for i in 0..grammar.num_rules() {
        let body = grammar.expr(grammar.rule(i).body_expr_id);
        if matches!(
            body.ty,
            GrammarExprType::TagDispatch | GrammarExprType::TokenTagDispatch
        ) {
            empty.insert(i);
            continue;
        }
        // Otherwise it is a choices expr.
        if grammar.expr(body.data[0]).ty == GrammarExprType::EmptyStr {
            empty.insert(i);
            continue;
        }
        for &seq_id in body.data {
            let seq = grammar.expr(seq_id);
            if seq.data.iter().all(|&e| {
                grammar.expr(e).ty == GrammarExprType::CharacterClassStar
            }) {
                empty.insert(i);
                break;
            }
        }
    }
}

/// Whether a sequence expr derives epsilon given the currently-known empty rules.
fn seq_expr_is_epsilon(
    grammar: &Grammar,
    seq_id: i32,
    empty: &HashSet<i32>,
) -> bool {
    let seq = grammar.expr(seq_id);
    if seq.ty == GrammarExprType::EmptyStr {
        return true;
    }
    seq.data.iter().all(|&i| {
        let element = grammar.expr(i);
        match element.ty {
            GrammarExprType::RuleRef => empty.contains(&element.data[0]),
            GrammarExprType::CharacterClassStar => true,
            GrammarExprType::Repeat => {
                empty.contains(&element.data[0]) || element.data[1] == 0
            },
            _ => false,
        }
    })
}

fn find_indirect_empty_rules(
    grammar: &Grammar,
    empty: &mut HashSet<i32>,
    graph: &[Vec<i32>],
) {
    let mut queue: VecDeque<i32> = empty.iter().copied().collect();
    while let Some(rule_id) = queue.pop_front() {
        for &referrer in &graph[rule_id as usize] {
            if empty.contains(&referrer) {
                continue;
            }
            let body = grammar.expr(grammar.rule(referrer).body_expr_id);
            let is_epsilon = body
                .data
                .iter()
                .any(|&seq_id| seq_expr_is_epsilon(grammar, seq_id, empty));
            if is_epsilon {
                empty.insert(referrer);
                queue.push_back(referrer);
            }
        }
    }
}
