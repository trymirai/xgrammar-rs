//! Builder for the BNF AST — a port of `cpp/grammar_builder.{h,cc}`.

use std::collections::HashMap;

use super::character_class_element::CharacterClassElement;
use super::grammar::Grammar;
use super::grammar_expr::GrammarExpr;
use super::grammar_expr_type::GrammarExprType;
use super::rule::{NO_EXPR, Rule};
use crate::support::Compact2dArray;

/// Builds a [`Grammar`] incrementally: add expressions (each returns its id), wire them
/// into rules, then finalize with a root rule.
#[derive(Debug, Default)]
pub struct GrammarBuilder {
    rules: Vec<Rule>,
    // Each row is `[type_tag, data...]`.
    exprs: Compact2dArray<i32>,
    rule_name_to_id: HashMap<String, i32>,
    next_cnt_per_hint: HashMap<String, i32>,
}

impl GrammarBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds a builder from an existing grammar (for functor passes and union/concat).
    #[must_use]
    pub fn from_grammar(grammar: &Grammar) -> Self {
        let rule_name_to_id = grammar
            .rules()
            .iter()
            .enumerate()
            .map(|(i, r)| (r.name.clone(), i as i32))
            .collect();
        Self {
            rules: grammar.rules().to_vec(),
            exprs: grammar.exprs().clone(),
            rule_name_to_id,
            next_cnt_per_hint: HashMap::new(),
        }
    }

    /// Finalizes the grammar, setting the root to the rule named `root_rule_name`.
    ///
    /// # Errors
    /// Returns the missing name if no such rule was added.
    pub fn into_grammar(self, root_rule_name: &str) -> Result<Grammar, String> {
        let root_rule_id = *self
            .rule_name_to_id
            .get(root_rule_name)
            .ok_or_else(|| format!("root rule {root_rule_name:?} was never added"))?;
        Ok(Grammar::from_parts(self.rules, self.exprs, root_rule_id))
    }

    /// Finalizes the grammar with an explicit root rule id.
    #[must_use]
    pub fn into_grammar_with_root_id(self, root_rule_id: i32) -> Grammar {
        Grammar::from_parts(self.rules, self.exprs, root_rule_id)
    }

    /* ----------------------------- expressions ----------------------------- */

    /// Low-level: appends an expression of `ty` with `data`, returning its id.
    pub fn add_grammar_expr(&mut self, ty: GrammarExprType, data: &[i32]) -> i32 {
        self.exprs.push_row_noncontiguous(ty.as_i32(), data) as i32
    }

    /// Adds a byte-string expression from raw byte values (`0..=255`).
    pub fn add_byte_string_bytes(&mut self, bytes: &[i32]) -> i32 {
        self.add_grammar_expr(GrammarExprType::ByteString, bytes)
    }

    /// Adds a byte-string expression from a string's UTF-8 bytes.
    pub fn add_byte_string(&mut self, s: &str) -> i32 {
        let bytes: Vec<i32> = s.bytes().map(i32::from).collect();
        self.add_byte_string_bytes(&bytes)
    }

    fn char_class_data(elements: &[CharacterClassElement], is_negative: bool) -> Vec<i32> {
        let mut data = Vec::with_capacity(1 + elements.len() * 2);
        data.push(i32::from(is_negative));
        for e in elements {
            data.push(e.lower);
            data.push(e.upper);
        }
        data
    }

    /// Adds a character-class expression.
    pub fn add_character_class(
        &mut self,
        elements: &[CharacterClassElement],
        is_negative: bool,
    ) -> i32 {
        let data = Self::char_class_data(elements, is_negative);
        self.add_grammar_expr(GrammarExprType::CharacterClass, &data)
    }

    /// Adds a starred character-class expression (`[...]*`).
    pub fn add_character_class_star(
        &mut self,
        elements: &[CharacterClassElement],
        is_negative: bool,
    ) -> i32 {
        let data = Self::char_class_data(elements, is_negative);
        self.add_grammar_expr(GrammarExprType::CharacterClassStar, &data)
    }

    /// Adds the empty-string expression.
    pub fn add_empty_str(&mut self) -> i32 {
        self.add_grammar_expr(GrammarExprType::EmptyStr, &[])
    }

    /// Adds an allowed-token-set expression.
    pub fn add_token_set(&mut self, token_ids: &[i32]) -> i32 {
        self.add_grammar_expr(GrammarExprType::Token, token_ids)
    }

    /// Adds an excluded-token-set expression.
    pub fn add_exclude_token_set(&mut self, token_ids: &[i32]) -> i32 {
        self.add_grammar_expr(GrammarExprType::ExcludeToken, token_ids)
    }

    /// Adds a reference to another rule.
    pub fn add_rule_ref(&mut self, rule_id: i32) -> i32 {
        self.add_grammar_expr(GrammarExprType::RuleRef, &[rule_id])
    }

    /// Adds a sequence of expressions.
    pub fn add_sequence(&mut self, elements: &[i32]) -> i32 {
        self.add_grammar_expr(GrammarExprType::Sequence, elements)
    }

    /// Adds an alternation of expressions.
    pub fn add_choices(&mut self, choices: &[i32]) -> i32 {
        self.add_grammar_expr(GrammarExprType::Choices, choices)
    }

    /// Adds a repetition of a rule: `[ref_rule_id, min, max]` (`max == -1` is unbounded).
    pub fn add_repeat(&mut self, ref_rule_id: i32, min_repeat: i32, max_repeat: i32) -> i32 {
        self.add_grammar_expr(GrammarExprType::Repeat, &[ref_rule_id, min_repeat, max_repeat])
    }

    /// Number of grammar expressions added so far.
    #[must_use]
    pub fn num_grammar_exprs(&self) -> i32 {
        self.exprs.len() as i32
    }

    /// A view of the expression with the given id.
    ///
    /// # Panics
    /// Panics if `expr_id` is out of bounds or its type tag is invalid.
    #[must_use]
    pub fn grammar_expr(&self, expr_id: i32) -> GrammarExpr<'_> {
        let row = self.exprs.row(expr_id as usize);
        let ty = GrammarExprType::try_from(row[0]).expect("builder stores valid expr type tags");
        GrammarExpr {
            ty,
            data: &row[1..],
        }
    }

    /* -------------------------------- rules -------------------------------- */

    /// Adds a rule, returning its id.
    ///
    /// # Panics
    /// Panics if a rule with the same name already exists.
    pub fn add_rule(&mut self, rule: Rule) -> i32 {
        let id = self.rules.len() as i32;
        assert!(
            !self.rule_name_to_id.contains_key(&rule.name),
            "duplicate rule name: {:?}",
            rule.name
        );
        self.rule_name_to_id.insert(rule.name.clone(), id);
        self.rules.push(rule);
        id
    }

    /// Adds a rule with the given name and body.
    pub fn add_rule_named(&mut self, name: impl Into<String>, body_expr_id: i32) -> i32 {
        self.add_rule(Rule::new(name, body_expr_id))
    }

    /// Adds a rule whose name is derived from `name_hint` (deduplicated with `_N` suffixes).
    pub fn add_rule_with_hint(&mut self, name_hint: &str, body_expr_id: i32) -> i32 {
        let name = self.get_new_rule_name(name_hint);
        self.add_rule(Rule::new(name, body_expr_id))
    }

    /// Adds a body-less rule (set the body later with [`Self::update_rule_body`]).
    pub fn add_empty_rule(&mut self, name: impl Into<String>) -> i32 {
        self.add_rule(Rule::empty(name))
    }

    /// Adds a body-less rule with a hinted, deduplicated name.
    pub fn add_empty_rule_with_hint(&mut self, name_hint: &str) -> i32 {
        let name = self.get_new_rule_name(name_hint);
        self.add_rule(Rule::empty(name))
    }

    /// Sets the body expression of an existing rule.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_rule_body(&mut self, rule_id: i32, body_expr_id: i32) {
        self.rules[rule_id as usize].body_expr_id = body_expr_id;
    }

    /// Attaches a lookahead assertion (a sequence expr id, or [`NO_EXPR`] for none).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_lookahead_assertion(&mut self, rule_id: i32, lookahead_assertion_id: i32) {
        self.rules[rule_id as usize].lookahead_assertion_id = lookahead_assertion_id;
    }

    /// Marks a rule's lookahead assertion as exact (or not).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_lookahead_exact(&mut self, rule_id: i32, is_exact: bool) {
        self.rules[rule_id as usize].is_exact_lookahead = is_exact;
    }

    /// Number of rules added so far.
    #[must_use]
    pub fn num_rules(&self) -> i32 {
        self.rules.len() as i32
    }

    /// The rule with the given id.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    #[must_use]
    pub fn get_rule(&self, rule_id: i32) -> &Rule {
        &self.rules[rule_id as usize]
    }

    /// The id of the rule with the given name, or [`NO_EXPR`] (`-1`) if absent.
    #[must_use]
    pub fn get_rule_id(&self, name: &str) -> i32 {
        self.rule_name_to_id.get(name).copied().unwrap_or(NO_EXPR)
    }

    /// Returns a fresh rule name based on `name_hint`, appending `_1`, `_2`, … to avoid
    /// collisions (matching the C++ scheme exactly).
    pub fn get_new_rule_name(&mut self, name_hint: &str) -> String {
        if !self.rule_name_to_id.contains_key(name_hint) {
            return name_hint.to_owned();
        }
        let cnt = self.next_cnt_per_hint.entry(name_hint.to_owned()).or_insert(0);
        if *cnt == 0 {
            *cnt = 1;
        }
        while self
            .rule_name_to_id
            .contains_key(&format!("{name_hint}_{cnt}"))
        {
            *cnt += 1;
        }
        format!("{name_hint}_{cnt}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_access_byte_string_rule() {
        let mut b = GrammarBuilder::new();
        let body = b.add_byte_string("hello");
        b.add_rule_named("root", body);
        let g = b.into_grammar("root").unwrap();

        assert_eq!(g.num_rules(), 1);
        assert_eq!(g.root_rule().name, "root");
        let expr = g.expr(g.root_rule().body_expr_id);
        assert_eq!(expr.ty, GrammarExprType::ByteString);
        assert_eq!(expr.byte_string(), b"hello");
    }

    #[test]
    fn character_class_roundtrip() {
        let mut b = GrammarBuilder::new();
        let cc = b.add_character_class(
            &[CharacterClassElement::new(b'a' as i32, b'z' as i32)],
            true,
        );
        b.add_rule_named("root", cc);
        let g = b.into_grammar("root").unwrap();
        let (neg, ranges) = g.expr(cc).character_class();
        assert!(neg);
        assert_eq!(ranges, vec![CharacterClassElement::new(97, 122)]);
    }

    #[test]
    fn sequence_and_choices_ids() {
        let mut b = GrammarBuilder::new();
        let a = b.add_byte_string("a");
        let c = b.add_byte_string("c");
        let seq = b.add_sequence(&[a, c]);
        let choices = b.add_choices(&[seq]);
        assert_eq!(b.grammar_expr(seq).data, &[a, c]);
        assert_eq!(b.grammar_expr(choices).ty, GrammarExprType::Choices);
    }

    #[test]
    fn empty_rule_then_update_body() {
        let mut b = GrammarBuilder::new();
        let rid = b.add_empty_rule("root");
        assert_eq!(b.get_rule(rid).body_expr_id, NO_EXPR);
        let body = b.add_empty_str();
        b.update_rule_body(rid, body);
        assert_eq!(b.get_rule(rid).body_expr_id, body);
    }

    #[test]
    fn lookahead_updates() {
        let mut b = GrammarBuilder::new();
        let body = b.add_empty_str();
        let rid = b.add_rule_named("root", body);
        let la = b.add_sequence(&[body]);
        b.update_lookahead_assertion(rid, la);
        b.update_lookahead_exact(rid, true);
        assert_eq!(b.get_rule(rid).lookahead_assertion_id, la);
        assert!(b.get_rule(rid).is_exact_lookahead);
    }

    #[test]
    fn new_rule_name_dedup() {
        let mut b = GrammarBuilder::new();
        assert_eq!(b.get_new_rule_name("root"), "root");
        b.add_rule_named("root", NO_EXPR);
        assert_eq!(b.get_new_rule_name("root"), "root_1");
        b.add_rule_named("root_1", 0);
        assert_eq!(b.get_new_rule_name("root"), "root_2");
    }

    #[test]
    fn get_rule_id_and_missing() {
        let mut b = GrammarBuilder::new();
        let body = b.add_empty_str();
        b.add_rule_named("root", body);
        assert_eq!(b.get_rule_id("root"), 0);
        assert_eq!(b.get_rule_id("nope"), NO_EXPR);
    }

    #[test]
    fn into_grammar_missing_root_errors() {
        let mut b = GrammarBuilder::new();
        let body = b.add_empty_str();
        b.add_rule_named("start", body);
        assert!(b.into_grammar("root").is_err());
    }

    #[test]
    fn serde_roundtrip_grammar() {
        let mut b = GrammarBuilder::new();
        let body = b.add_byte_string("hi");
        b.add_rule_named("root", body);
        let g = b.into_grammar("root").unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let back: Grammar = serde_json::from_str(&json).unwrap();
        assert_eq!(g, back);
    }
}
