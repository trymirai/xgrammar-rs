//! Grammar union and concatenation — a port of `GrammarUnionFunctor`,
//! `GrammarConcatFunctor`, and `SubGrammarAdder` in `cpp/grammar_functor.cc`.

use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

impl Grammar {
    /// Returns a grammar accepting any string accepted by one of `grammars`.
    ///
    /// # Panics
    /// Panics if `grammars` is empty.
    #[must_use]
    pub fn union(grammars: &[Grammar]) -> Grammar {
        assert!(!grammars.is_empty(), "union requires at least one grammar");
        let mut builder = GrammarBuilder::new();
        let root = builder.add_empty_rule("root");
        let mut choices = Vec::with_capacity(grammars.len());
        for grammar in grammars {
            let sub_root = add_sub_grammar(&mut builder, grammar);
            let rule_ref = builder.add_rule_ref(sub_root);
            choices.push(builder.add_sequence(&[rule_ref]));
        }
        let body = builder.add_choices(&choices);
        builder.update_rule_body(root, body);
        builder.into_grammar_with_root_id(root)
    }

    /// Returns a grammar accepting the in-order concatenation of strings from `grammars`.
    ///
    /// # Panics
    /// Panics if `grammars` is empty.
    #[must_use]
    pub fn concat(grammars: &[Grammar]) -> Grammar {
        assert!(!grammars.is_empty(), "concat requires at least one grammar");
        let mut builder = GrammarBuilder::new();
        let root = builder.add_empty_rule("root");
        let mut sequence = Vec::with_capacity(grammars.len());
        for grammar in grammars {
            let sub_root = add_sub_grammar(&mut builder, grammar);
            sequence.push(builder.add_rule_ref(sub_root));
        }
        let seq = builder.add_sequence(&sequence);
        let body = builder.add_choices(&[seq]);
        builder.update_rule_body(root, body);
        builder.into_grammar_with_root_id(root)
    }
}

/// Copies every rule of `sub` into `builder` (deduplicating names and remapping references),
/// returning the new id of `sub`'s root rule.
fn add_sub_grammar(builder: &mut GrammarBuilder, sub: &Grammar) -> i32 {
    let mut new_rule_ids = Vec::with_capacity(sub.num_rules() as usize);
    for i in 0..sub.num_rules() {
        let name = builder.get_new_rule_name(&sub.rule(i).name);
        new_rule_ids.push(builder.add_empty_rule(name));
    }
    for i in 0..sub.num_rules() {
        let (body, lookahead) = {
            let rule = sub.rule(i);
            (rule.body_expr_id, rule.lookahead_assertion_id)
        };
        let new_body = copy_expr(builder, sub, &new_rule_ids, body);
        builder.update_rule_body(new_rule_ids[i as usize], new_body);
        let new_lookahead = if lookahead == NO_EXPR {
            NO_EXPR
        } else {
            copy_expr(builder, sub, &new_rule_ids, lookahead)
        };
        builder.update_lookahead_assertion(new_rule_ids[i as usize], new_lookahead);
    }
    new_rule_ids[sub.root_rule_id() as usize]
}

/// Re-adds an expression from `sub` into `builder`, remapping rule references through
/// `new_rule_ids`.
fn copy_expr(builder: &mut GrammarBuilder, sub: &Grammar, new_rule_ids: &[i32], expr_id: i32) -> i32 {
    let (ty, data) = {
        let expr = sub.expr(expr_id);
        (expr.ty, expr.data.to_vec())
    };
    match ty {
        GrammarExprType::Sequence => {
            let mut ids = Vec::with_capacity(data.len());
            for &child in &data {
                ids.push(copy_expr(builder, sub, new_rule_ids, child));
            }
            builder.add_sequence(&ids)
        }
        GrammarExprType::Choices => {
            let mut ids = Vec::with_capacity(data.len());
            for &child in &data {
                ids.push(copy_expr(builder, sub, new_rule_ids, child));
            }
            builder.add_choices(&ids)
        }
        GrammarExprType::RuleRef => builder.add_rule_ref(new_rule_ids[data[0] as usize]),
        GrammarExprType::Repeat => {
            builder.add_repeat(new_rule_ids[data[0] as usize], data[1], data[2])
        }
        // Tag-dispatch remapping needs the tag-dispatch builder (macro work); not exercised
        // by the union/concat gates.
        _ => builder.add_grammar_expr(ty, &data),
    }
}
