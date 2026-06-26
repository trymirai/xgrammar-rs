//! Renames the root rule to `root` — a port of `RootRuleRenamer` in
//! `cpp/grammar_functor.cc`.

use std::collections::HashSet;

use crate::grammar::Grammar;

/// Ensures the root rule is named `root`. If another rule already has that name, it is
/// renamed to the first free `root_N`.
#[must_use]
pub fn root_rule_renamer(grammar: &Grammar) -> Grammar {
    if grammar.root_rule().name == "root" {
        return grammar.clone();
    }

    let names: HashSet<&str> =
        grammar.rules().iter().map(|r| r.name.as_str()).collect();
    let collision =
        grammar.rules().iter().position(|r| r.name == "root").map(|i| i as i32);

    let mut renamed = grammar.clone();
    let root_id = renamed.root_rule_id();
    renamed.rename_rule(root_id, "root".to_owned());

    if let Some(collision_id) = collision {
        for i in 0..=grammar.num_rules() {
            let candidate = format!("root_{i}");
            if !names.contains(candidate.as_str()) {
                renamed.rename_rule(collision_id, candidate);
                break;
            }
        }
    }
    renamed
}
