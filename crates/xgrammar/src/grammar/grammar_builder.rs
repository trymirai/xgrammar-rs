//! Builder for the BNF AST.

use std::{cell::OnceCell, collections::HashMap, mem};

use super::{
    character_class_element::CharacterClassElement,
    grammar::Grammar,
    grammar_expr::GrammarExpr,
    grammar_expr_type::GrammarExprType,
    rule::{NO_EXPR, Rule},
    suffix_stop_info::SuffixStopInfo,
    tag_dispatch::TagDispatch,
    token_tag_dispatch::TokenTagDispatch,
};
use crate::support::Compact2dArray;

/// Builds a [`Grammar`] incrementally: add expressions (each returns its id), wire them
/// into rules, then finalize with a root rule.
#[derive(Debug, Default)]
pub struct GrammarBuilder {
    rules: Vec<Rule>,
    suffix_stop_infos: Vec<SuffixStopInfo>,
    // Each row is `[type_tag, data...]`.
    exprs: Compact2dArray<i32>,
    // Built from `rules` on the first name-based operation, so id-only rewriting of an existing
    // grammar pays no name hashing cost (the C++ `EnsureRuleNameMap`).
    rule_name_to_id: OnceCell<HashMap<String, i32>>,
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
        Self {
            rules: grammar.rules().to_vec(),
            suffix_stop_infos: grammar.suffix_stop_infos().to_vec(),
            exprs: grammar.exprs().clone(),
            ..Self::default()
        }
    }

    /// Binds a builder to `grammar` without copying it (the C++ `GrammarBuilder::FromMutableGrammar`):
    /// within `edit`, appended exprs and rule updates are written directly into `grammar`, whose root
    /// rule and analysis results are left untouched.
    pub fn edit_in_place<R>(
        grammar: &mut Grammar,
        edit: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let (rules, exprs, suffix_stop_infos) = grammar.storage_mut();
        let mut builder = Self {
            rules: mem::take(rules),
            suffix_stop_infos: mem::take(suffix_stop_infos),
            exprs: mem::take(exprs),
            ..Self::default()
        };
        let result = edit(&mut builder);
        (*rules, *exprs, *suffix_stop_infos) = (builder.rules, builder.exprs, builder.suffix_stop_infos);
        result
    }

    /// Finalizes the grammar, setting the root to the rule named `root_rule_name`.
    ///
    /// # Errors
    /// Returns the missing name if no such rule was added.
    pub fn into_grammar(
        self,
        root_rule_name: &str,
    ) -> Result<Grammar, String> {
        let root_rule_id = *self
            .rule_name_to_id()
            .get(root_rule_name)
            .ok_or_else(|| format!("root rule {root_rule_name:?} was never added"))?;
        Ok(self.into_grammar_with_root_id(root_rule_id))
    }

    fn rule_name_to_id(&self) -> &HashMap<String, i32> {
        self.rule_name_to_id
            .get_or_init(|| self.rules.iter().enumerate().map(|(id, rule)| (rule.name.clone(), id as i32)).collect())
    }

    /// Finalizes the grammar with an explicit root rule id.
    #[must_use]
    pub fn into_grammar_with_root_id(
        self,
        root_rule_id: i32,
    ) -> Grammar {
        let mut grammar = Grammar::from_parts(self.rules, self.exprs, root_rule_id);
        grammar.set_suffix_stop_infos(self.suffix_stop_infos);
        grammar
    }

    /* ----------------------------- expressions ----------------------------- */

    /// Low-level: appends an expression of `ty` with `data`, returning its id.
    pub fn add_grammar_expr(
        &mut self,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        self.exprs.push_row_noncontiguous(ty.as_i32(), data) as i32
    }

    /// Adds a byte-string expression from raw byte values (`0..=255`).
    pub fn add_byte_string_bytes(
        &mut self,
        bytes: &[i32],
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::ByteString, bytes)
    }

    /// Adds a byte-string expression from a string's UTF-8 bytes.
    pub fn add_byte_string(
        &mut self,
        s: &str,
    ) -> i32 {
        let bytes: Vec<i32> = s.bytes().map(i32::from).collect();
        self.add_byte_string_bytes(&bytes)
    }

    fn char_class_data(
        elements: &[CharacterClassElement],
        is_negative: bool,
    ) -> Vec<i32> {
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
    pub fn add_token_set(
        &mut self,
        token_ids: &[i32],
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::Token, token_ids)
    }

    /// Adds an excluded-token-set expression.
    pub fn add_exclude_token_set(
        &mut self,
        token_ids: &[i32],
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::ExcludeToken, token_ids)
    }

    /// Adds a reference to another rule.
    pub fn add_rule_ref(
        &mut self,
        rule_id: i32,
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::RuleRef, &[rule_id])
    }

    /// Adds a sequence of expressions.
    pub fn add_sequence(
        &mut self,
        elements: &[i32],
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::Sequence, elements)
    }

    /// Adds an alternation of expressions.
    pub fn add_choices(
        &mut self,
        choices: &[i32],
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::Choices, choices)
    }

    /// Adds a repetition of a rule: `[ref_rule_id, min, max]` (`max == -1` is unbounded).
    pub fn add_repeat(
        &mut self,
        ref_rule_id: i32,
        min_repeat: i32,
        max_repeat: i32,
    ) -> i32 {
        self.add_grammar_expr(GrammarExprType::Repeat, &[ref_rule_id, min_repeat, max_repeat])
    }

    /// Adds a repetition of an arbitrary expression, wrapping it in a fresh rule (named
    /// from `cur_rule_name`) unless it is already a rule reference.
    pub fn add_repeat_from_expr(
        &mut self,
        cur_rule_name: &str,
        grammar_expr_id: i32,
        min_repeat: i32,
        max_repeat: i32,
    ) -> i32 {
        let rule_ref = {
            let expr = self.grammar_expr(grammar_expr_id);
            (expr.ty == GrammarExprType::RuleRef).then(|| expr.rule_ref_id())
        };
        let ref_rule_id = match rule_ref {
            Some(id) => id,
            None => {
                let name = self.get_new_rule_name(cur_rule_name);
                self.add_rule_named(name, grammar_expr_id)
            },
        };
        self.add_repeat(ref_rule_id, min_repeat, max_repeat)
    }

    /// Adds a regex expression. The pattern is stored as-is and compiled into an automaton by
    /// the grammar FSM builder. If `json_string` is true, the characters that must be escaped in
    /// a JSON string are excluded from every character match of the compiled automaton.
    pub fn add_regex(
        &mut self,
        pattern: &[u8],
        json_string: bool,
    ) -> i32 {
        let mut data = Vec::with_capacity(pattern.len() + 1);
        data.push(i32::from(json_string));
        data.extend(pattern.iter().map(|&b| i32::from(b)));
        self.add_grammar_expr(GrammarExprType::Regex, &data)
    }

    /// Adds a substring expression, which matches every contiguous subsequence of the chunk
    /// list (including the empty one). Chunks may be empty or repeated.
    pub fn add_substring<C: AsRef<[u8]>>(
        &mut self,
        chunks: &[C],
    ) -> i32 {
        let mut data = Vec::new();
        for chunk in chunks {
            let chunk = chunk.as_ref();
            data.push(chunk.len() as i32);
            data.extend(chunk.iter().map(|&b| i32::from(b)));
        }
        self.add_grammar_expr(GrammarExprType::Substring, &data)
    }

    /// Adds a tag-dispatch expression from its decoded form.
    pub fn add_tag_dispatch(
        &mut self,
        tag_dispatch: &TagDispatch,
    ) -> i32 {
        let mut data = Vec::with_capacity(tag_dispatch.tag_rule_pairs.len() * 2 + 2);
        for (tag, rule_id) in &tag_dispatch.tag_rule_pairs {
            let bytes: Vec<i32> = tag.iter().map(|&b| i32::from(b)).collect();
            data.push(self.add_byte_string_bytes(&bytes));
            data.push(*rule_id);
        }
        data.push(i32::from(tag_dispatch.loop_after_dispatch));
        let mut exclude_ids = Vec::with_capacity(tag_dispatch.excludes.len());
        for exclude in &tag_dispatch.excludes {
            let bytes: Vec<i32> = exclude.iter().map(|&b| i32::from(b)).collect();
            exclude_ids.push(self.add_byte_string_bytes(&bytes));
        }
        let exclude_choices = self.add_choices(&exclude_ids);
        data.push(exclude_choices);
        self.add_grammar_expr(GrammarExprType::TagDispatch, &data)
    }

    /// Adds a token-tag-dispatch expression from its decoded form.
    pub fn add_token_tag_dispatch(
        &mut self,
        ttd: &TokenTagDispatch,
    ) -> i32 {
        let mut data = Vec::with_capacity(ttd.trigger_rule_pairs.len() * 2 + ttd.excludes.len() + 3);
        data.push(ttd.trigger_rule_pairs.len() as i32);
        for (token_id, rule_id) in &ttd.trigger_rule_pairs {
            data.push(*token_id);
            data.push(*rule_id);
        }
        data.push(i32::from(ttd.loop_after_dispatch));
        data.push(ttd.excludes.len() as i32);
        data.extend_from_slice(&ttd.excludes);
        self.add_grammar_expr(GrammarExprType::TokenTagDispatch, &data)
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
    pub fn grammar_expr(
        &self,
        expr_id: i32,
    ) -> GrammarExpr<'_> {
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
    pub fn add_rule(
        &mut self,
        rule: Rule,
    ) -> i32 {
        let id = self.rules.len() as i32;
        assert!(!self.rule_name_to_id().contains_key(&rule.name), "duplicate rule name: {:?}", rule.name);
        self.rule_name_to_id.get_mut().expect("rule name map built above").insert(rule.name.clone(), id);
        self.rules.push(rule);
        id
    }

    /// Adds a rule with the given name and body.
    pub fn add_rule_named(
        &mut self,
        name: impl Into<String>,
        body_expr_id: i32,
    ) -> i32 {
        self.add_rule(Rule::new(name, body_expr_id))
    }

    /// Adds a rule whose name is derived from `name_hint` (deduplicated with `_N` suffixes).
    pub fn add_rule_with_hint(
        &mut self,
        name_hint: &str,
        body_expr_id: i32,
    ) -> i32 {
        let name = self.get_new_rule_name(name_hint);
        self.add_rule(Rule::new(name, body_expr_id))
    }

    /// Adds a body-less rule (set the body later with [`Self::update_rule_body`]).
    pub fn add_empty_rule(
        &mut self,
        name: impl Into<String>,
    ) -> i32 {
        self.add_rule(Rule::empty(name))
    }

    /// Adds a body-less rule with a hinted, deduplicated name.
    pub fn add_empty_rule_with_hint(
        &mut self,
        name_hint: &str,
    ) -> i32 {
        let name = self.get_new_rule_name(name_hint);
        self.add_rule(Rule::empty(name))
    }

    /// Sets the body expression of an existing rule.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_rule_body(
        &mut self,
        rule_id: i32,
        body_expr_id: i32,
    ) {
        self.rules[rule_id as usize].body_expr_id = body_expr_id;
    }

    /// Sets the body expression of the rule with the given name.
    ///
    /// # Panics
    /// Panics if no rule with that name exists.
    pub fn update_rule_body_by_name(
        &mut self,
        name: &str,
        body_expr_id: i32,
    ) {
        let id = self.get_rule_id(name);
        assert!(id != NO_EXPR, "rule {name:?} not found");
        self.update_rule_body(id, body_expr_id);
    }

    /// Attaches a lookahead assertion to the rule with the given name.
    ///
    /// # Panics
    /// Panics if no rule with that name exists.
    pub fn update_lookahead_assertion_by_name(
        &mut self,
        name: &str,
        lookahead_assertion_id: i32,
    ) {
        let id = self.get_rule_id(name);
        assert!(id != NO_EXPR, "rule {name:?} not found");
        self.update_lookahead_assertion(id, lookahead_assertion_id);
    }

    /// Attaches a lookahead assertion (a sequence expr id, or [`NO_EXPR`] for none).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_lookahead_assertion(
        &mut self,
        rule_id: i32,
        lookahead_assertion_id: i32,
    ) {
        self.rules[rule_id as usize].lookahead_assertion_id = lookahead_assertion_id;
    }

    /// Marks a rule's lookahead assertion as exact (or not).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_lookahead_exact(
        &mut self,
        rule_id: i32,
        is_exact: bool,
    ) {
        self.rules[rule_id as usize].is_exact_lookahead = is_exact;
    }

    /// Sets the token budget of a rule (`-1` means none).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_max_tokens(
        &mut self,
        rule_id: i32,
        max_tokens: i32,
    ) {
        self.rules[rule_id as usize].max_tokens = max_tokens;
    }

    /// Sets the character budget of a rule (`-1` means none).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_max_chars(
        &mut self,
        rule_id: i32,
        max_chars: i32,
    ) {
        self.rules[rule_id as usize].max_chars = max_chars;
    }

    /// Sets the capture group name of a rule (empty means no capture).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_capture_name(
        &mut self,
        rule_id: i32,
        capture_name: impl Into<String>,
    ) {
        self.rules[rule_id as usize].capture_name = capture_name.into();
    }

    /// Sets whether a rule is lazy (committed-shortest).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_lazy(
        &mut self,
        rule_id: i32,
        is_lazy: bool,
    ) {
        self.rules[rule_id as usize].is_lazy = is_lazy;
    }

    /// Sets the sampling temperature associated with a rule.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub fn update_rule_temperature(
        &mut self,
        rule_id: i32,
        temperature: Option<f32>,
    ) {
        self.rules[rule_id as usize].temperature = temperature;
    }

    /// Copies the budgets, capture name, laziness, temperature, and suffix/stop metadata of
    /// `source`'s rule `source_rule_id` onto `rule_id`, mapping suffix/stop helper rule ids
    /// through `map_rule_id`.
    pub fn copy_rule_attributes(
        &mut self,
        rule_id: i32,
        source: &Grammar,
        source_rule_id: i32,
        map_rule_id: impl Fn(i32) -> i32,
    ) {
        let rule = source.rule(source_rule_id);
        self.update_max_tokens(rule_id, rule.max_tokens);
        self.update_max_chars(rule_id, rule.max_chars);
        self.update_capture_name(rule_id, rule.capture_name.clone());
        if let Some(info) = source.suffix_stop_info(source_rule_id) {
            let mut info = info.clone();
            if info.body_rule_id >= 0 {
                info.body_rule_id = map_rule_id(info.body_rule_id);
                info.marker_rule_id = map_rule_id(info.marker_rule_id);
            }
            self.update_suffix_stop_info(rule_id, info);
        }
        self.update_lazy(rule_id, rule.is_lazy);
        self.update_rule_temperature(rule_id, rule.temperature);
    }

    /// Sets or clears (when `info` is empty) the sparse suffix/stop metadata of a rule.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds, a hidden byte count is negative, or exactly one of
    /// the helper rule ids is set.
    pub fn update_suffix_stop_info(
        &mut self,
        rule_id: i32,
        mut info: SuffixStopInfo,
    ) {
        let num_rules = self.num_rules();
        assert!((0..num_rules).contains(&rule_id), "Rule id {rule_id} is out of range.");
        assert!(
            info.hidden_suffix_bytes >= 0 && info.hidden_stop_bytes >= 0,
            "The number of hidden suffix/stop bytes must be non-negative."
        );
        assert!(
            (info.body_rule_id == -1 && info.marker_rule_id == -1)
                || ((0..num_rules).contains(&info.body_rule_id) && (0..num_rules).contains(&info.marker_rule_id)),
            "Capture-hidden helper rule ids must both be -1 or valid rule ids."
        );
        let position = self.suffix_stop_infos.binary_search_by_key(&rule_id, |existing| existing.rule_id);
        if info.is_empty() {
            if let Ok(index) = position {
                self.suffix_stop_infos.remove(index);
            }
            return;
        }
        info.rule_id = rule_id;
        match position {
            Ok(index) => self.suffix_stop_infos[index] = info,
            Err(index) => self.suffix_stop_infos.insert(index, info),
        }
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
    pub fn get_rule(
        &self,
        rule_id: i32,
    ) -> &Rule {
        &self.rules[rule_id as usize]
    }

    /// The id of the rule with the given name, or [`NO_EXPR`] (`-1`) if absent.
    #[must_use]
    pub fn get_rule_id(
        &self,
        name: &str,
    ) -> i32 {
        self.rule_name_to_id().get(name).copied().unwrap_or(NO_EXPR)
    }

    /// Returns a fresh rule name based on `name_hint`, appending `_1`, `_2`, … to avoid
    /// collisions (matching the C++ scheme exactly).
    pub fn get_new_rule_name(
        &mut self,
        name_hint: &str,
    ) -> String {
        if !self.rule_name_to_id().contains_key(name_hint) {
            return name_hint.to_owned();
        }
        let mut cnt = self.next_cnt_per_hint.get(name_hint).copied().unwrap_or(0).max(1);
        while self.rule_name_to_id().contains_key(&format!("{name_hint}_{cnt}")) {
            cnt += 1;
        }
        self.next_cnt_per_hint.insert(name_hint.to_owned(), cnt);
        format!("{name_hint}_{cnt}")
    }
}
