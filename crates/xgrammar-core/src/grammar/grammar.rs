//! The BNF grammar AST in flat CSR form — a port of `Grammar::Impl` in
//! `cpp/grammar_impl.h` (the matching-aux FSM fields are introduced in M5).

use serde::{Deserialize, Serialize};

use super::{
    grammar_expr::GrammarExpr, grammar_expr_type::GrammarExprType, rule::Rule,
};
use crate::{
    fsm::{CompactFsm, CompactFsmWithStartEndWithSize},
    support::Compact2dArray,
};

/// A Backus–Naur Form grammar: an ordered set of [`Rule`]s plus all grammar expressions
/// stored contiguously, with one root rule.
///
/// Each expression occupies one row of `exprs`, laid out as `[type_tag, data...]`; the row
/// length encodes the payload length (so the C++ `data_len` header is unnecessary here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grammar {
    rules: Vec<Rule>,
    exprs: Compact2dArray<i32>,
    root_rule_id: i32,
    /// The shared FSM holding every rule's compiled automaton (empty until optimized). Not
    /// serialized; rebuilt by the grammar optimizer.
    #[serde(skip)]
    complete_fsm: CompactFsm,
    /// Per-rule compiled FSMs into [`Self::complete_fsm`] (empty until optimized).
    #[serde(skip)]
    per_rule_fsms: Vec<Option<CompactFsmWithStartEndWithSize>>,
    /// Ids of rules that can match the empty string (set by the allow-empty analyzer).
    #[serde(skip)]
    allow_empty_rule_ids: Vec<i32>,
    /// Whether the grammar optimizer has run (FSMs built).
    #[serde(skip)]
    optimized: bool,
}

impl Grammar {
    /// Assembles a grammar from its parts (used by [`super::GrammarBuilder`]).
    #[must_use]
    pub(crate) fn from_parts(
        rules: Vec<Rule>,
        exprs: Compact2dArray<i32>,
        root_rule_id: i32,
    ) -> Self {
        Self {
            rules,
            exprs,
            root_rule_id,
            complete_fsm: CompactFsm::default(),
            per_rule_fsms: Vec::new(),
            allow_empty_rule_ids: Vec::new(),
            optimized: false,
        }
    }

    /// The shared complete FSM (valid only after optimization).
    #[must_use]
    pub fn complete_fsm(&self) -> &CompactFsm {
        &self.complete_fsm
    }

    /// The compiled FSM for `rule_id`, if built.
    #[must_use]
    pub fn per_rule_fsm(
        &self,
        rule_id: i32,
    ) -> Option<&CompactFsmWithStartEndWithSize> {
        self.per_rule_fsms.get(rule_id as usize).and_then(Option::as_ref)
    }

    /// The ids of rules that can match the empty string.
    #[must_use]
    pub fn allow_empty_rule_ids(&self) -> &[i32] {
        &self.allow_empty_rule_ids
    }

    /// Whether the grammar optimizer has run.
    #[must_use]
    pub fn is_optimized(&self) -> bool {
        self.optimized
    }

    /// Installs the compiled FSMs (used by the grammar FSM builder).
    pub(crate) fn set_fsms(
        &mut self,
        complete_fsm: CompactFsm,
        per_rule_fsms: Vec<Option<CompactFsmWithStartEndWithSize>>,
    ) {
        self.complete_fsm = complete_fsm;
        self.per_rule_fsms = per_rule_fsms;
    }

    /// Sets the empty-rule ids (used by the allow-empty analyzer).
    pub(crate) fn set_allow_empty_rule_ids(
        &mut self,
        ids: Vec<i32>,
    ) {
        self.allow_empty_rule_ids = ids;
    }

    /// Marks the grammar as optimized.
    pub(crate) fn set_optimized(
        &mut self,
        optimized: bool,
    ) {
        self.optimized = optimized;
    }

    /// Per-rule compiled FSM slices (empty until optimized).
    pub(crate) fn per_rule_fsms_slice(
        &self
    ) -> &[Option<CompactFsmWithStartEndWithSize>] {
        &self.per_rule_fsms
    }

    /// A mutable reference to a rule (used by in-place passes like the repetition normalizer).
    pub(crate) fn rule_mut(
        &mut self,
        rule_id: i32,
    ) -> &mut Rule {
        &mut self.rules[rule_id as usize]
    }

    /// Overwrites the `index`-th data element of an expression in place.
    pub(crate) fn set_expr_data(
        &mut self,
        expr_id: i32,
        index: usize,
        value: i32,
    ) {
        self.exprs.row_mut(expr_id as usize)[1 + index] = value;
    }

    /// All rules, indexed by rule id.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The flat expression store.
    #[must_use]
    pub(crate) fn exprs(&self) -> &Compact2dArray<i32> {
        &self.exprs
    }

    /// Number of rules.
    #[must_use]
    pub fn num_rules(&self) -> i32 {
        self.rules.len() as i32
    }

    /// The rule with the given id.
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    #[must_use]
    pub fn rule(
        &self,
        rule_id: i32,
    ) -> &Rule {
        &self.rules[rule_id as usize]
    }

    /// Renames a rule in place (used by the root-rule renamer pass).
    ///
    /// # Panics
    /// Panics if `rule_id` is out of bounds.
    pub(crate) fn rename_rule(
        &mut self,
        rule_id: i32,
        new_name: String,
    ) {
        self.rules[rule_id as usize].name = new_name;
    }

    /// The root rule id.
    #[must_use]
    pub fn root_rule_id(&self) -> i32 {
        self.root_rule_id
    }

    /// The root rule.
    ///
    /// # Panics
    /// Panics if the root rule id is unset/out of bounds.
    #[must_use]
    pub fn root_rule(&self) -> &Rule {
        self.rule(self.root_rule_id)
    }

    /// Number of grammar expressions.
    #[must_use]
    pub fn num_exprs(&self) -> i32 {
        self.exprs.len() as i32
    }

    /// The expression with the given id, as a borrowed view.
    ///
    /// # Panics
    /// Panics if `expr_id` is out of bounds or the stored type tag is invalid.
    #[must_use]
    pub fn expr(
        &self,
        expr_id: i32,
    ) -> GrammarExpr<'_> {
        let row = self.exprs.row(expr_id as usize);
        let ty = GrammarExprType::try_from(row[0])
            .expect("grammar stores valid expr type tags");
        GrammarExpr {
            ty,
            data: &row[1..],
        }
    }

    /// The byte string of a [`GrammarExprType::ByteString`] expression.
    #[must_use]
    pub fn byte_string(
        &self,
        expr_id: i32,
    ) -> Vec<u8> {
        self.expr(expr_id).byte_string()
    }
}
