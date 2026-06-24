//! A string builder for EBNF grammar scripts — a port of `EBNFScriptCreator` in
//! `cpp/ebnf_script_creator.h`.
//!
//! It allocates unique rule names, accumulates `(name, body)` rules in order, and renders
//! the final script. The JSON-schema and structural-tag converters build their grammars
//! through it.

use std::collections::HashSet;

use crate::support::escape_str;

/// Upper bound on the numeric suffix tried when de-duplicating a rule name.
const NAME_SUFFIX_MAXIMUM: i32 = 100_000;

/// Builds an EBNF script rule by rule, keeping rule names unique.
#[derive(Debug, Default)]
pub struct EbnfScriptCreator {
    rule_names: HashSet<String>,
    rules: Vec<(String, String)>,
}

impl EbnfScriptCreator {
    /// Creates an empty script creator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a rule with a hinted (de-duplicated) name, returning the assigned name.
    pub fn add_rule(
        &mut self,
        rule_name_hint: &str,
        rule_body: &str,
    ) -> String {
        let name = self.allocate_rule_name(rule_name_hint);
        self.add_rule_with_allocated_name(&name, rule_body)
    }

    /// Allocates a unique rule name based on `rule_name_hint` (appending `_0`, `_1`, …).
    ///
    /// # Panics
    /// Panics if no unique name can be found within [`NAME_SUFFIX_MAXIMUM`] attempts.
    pub fn allocate_rule_name(
        &mut self,
        rule_name_hint: &str,
    ) -> String {
        if self.rule_names.insert(rule_name_hint.to_owned()) {
            return rule_name_hint.to_owned();
        }
        for i in 0..NAME_SUFFIX_MAXIMUM {
            let candidate = format!("{rule_name_hint}_{i}");
            if self.rule_names.insert(candidate.clone()) {
                return candidate;
            }
        }
        panic!("cannot find a unique rule name for {rule_name_hint}");
    }

    /// Adds a rule with a name previously obtained from [`Self::allocate_rule_name`].
    ///
    /// # Panics
    /// Panics if `rule_name` was not allocated.
    pub fn add_rule_with_allocated_name(
        &mut self,
        rule_name: &str,
        rule_body: &str,
    ) -> String {
        assert!(
            self.rule_names.contains(rule_name),
            "rule name {rule_name} is not allocated"
        );
        self.rules.push((rule_name.to_owned(), rule_body.to_owned()));
        rule_name.to_owned()
    }

    /// The body of a previously added rule, if present.
    #[must_use]
    pub fn rule_body(
        &self,
        rule_name: &str,
    ) -> Option<&str> {
        self.rules
            .iter()
            .find(|(name, _)| name == rule_name)
            .map(|(_, body)| body.as_str())
    }

    /// The complete EBNF script (one `name ::= body` per line).
    #[must_use]
    pub fn script(&self) -> String {
        let mut out = String::new();
        for (name, body) in &self.rules {
            out.push_str(name);
            out.push_str(" ::= ");
            out.push_str(body);
            out.push('\n');
        }
        out
    }

    /// Concatenates items into a parenthesized sequence: `(a b c)`.
    #[must_use]
    pub fn concat(items: &[String]) -> String {
        format!("({})", items.join(" "))
    }

    /// Joins items into a parenthesized alternation: `(a | b | c)`.
    #[must_use]
    pub fn or(items: &[String]) -> String {
        format!("({})", items.join(" | "))
    }

    /// Escapes and quotes a literal string for EBNF.
    #[must_use]
    pub fn str_literal(s: &str) -> String {
        format!("\"{}\"", escape_str(s))
    }

    /// Renders a repetition quantifier for `item` over `[min, max]` (`max == -1` is
    /// unbounded), collapsing to `?`/`*`/`+`/`{m}`/`{m,}`/`{m,n}` as appropriate.
    #[must_use]
    pub fn repeat(
        item: &str,
        min: i32,
        max: i32,
    ) -> String {
        match (min, max) {
            (0, 1) => format!("{item}?"),
            (0, -1) => format!("{item}*"),
            (1, -1) => format!("{item}+"),
            (0, 0) => String::new(),
            (min, max) if min == max => format!("{item}{{{min}}}"),
            (min, -1) => format!("{item}{{{min},}}"),
            (min, max) => format!("{item}{{{min},{max}}}"),
        }
    }
}
