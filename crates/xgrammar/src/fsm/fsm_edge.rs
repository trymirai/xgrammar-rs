//! The edge of a finite-state machine — a port of `FSMEdge` in `cpp/fsm.h`.
//!
//! An edge's kind is encoded in its `min` field: `min >= 0` is a character range `[min,
//! max]`; a negative `min` is one of the special [`edge_type`] tags, where `max` carries a
//! type-specific payload (a rule id, or an index into the owning FSM's `edge_aux_data`).

/// Special edge-type tags stored in [`FsmEdge::min`] when it is negative.
pub mod edge_type {
    /// Character range `[min, max]` (`min >= 0`; not represented here).
    pub const CHAR_RANGE: i32 = 0;
    /// Epsilon transition; `max` is unused.
    pub const EPSILON: i32 = -1;
    /// Rule reference; `max` is the rule id.
    pub const RULE_REF: i32 = -2;
    /// Accepts the end-of-string token; `max` is unused.
    pub const EOS: i32 = -3;
    /// Repeated rule reference; `max` indexes `edge_aux_data` (`[rule_id, lower, upper]`).
    pub const REPEAT_REF: i32 = -4;
    /// Accepts a set of token ids; `max` indexes `edge_aux_data` (`[count, ids...]`).
    pub const TOKEN: i32 = -5;
    /// Accepts any token not in a set; `max` indexes `edge_aux_data` (`[count, ids...]`).
    pub const EXCLUDE_TOKEN: i32 = -6;
}

/// The largest character value an edge range may span.
pub const MAX_CHAR: i32 = 255;

/// An FSM transition. Ordering is lexicographic over `(min, max, target)`, matching the
/// C++ edge sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FsmEdge {
    /// Range lower bound, or a negative [`edge_type`] tag.
    pub min: i32,
    /// Range upper bound, or a type-specific payload.
    pub max: i32,
    /// Target state id.
    pub target: i32,
}

impl FsmEdge {
    /// Creates an edge.
    ///
    /// # Panics
    /// Panics (debug builds) if this is a character range with `min > max`.
    #[must_use]
    pub fn new(
        min: i32,
        max: i32,
        target: i32,
    ) -> Self {
        debug_assert!(
            min < 0 || min <= max,
            "invalid FsmEdge: min > max (min={min}, max={max})"
        );
        Self {
            min,
            max,
            target,
        }
    }

    /// Whether the edge is a character range `[min, max]`.
    #[must_use]
    pub fn is_char_range(&self) -> bool {
        self.min >= 0
    }

    /// Whether the edge is an epsilon transition.
    #[must_use]
    pub fn is_epsilon(&self) -> bool {
        self.min == edge_type::EPSILON
    }

    /// Whether the edge is a rule reference.
    #[must_use]
    pub fn is_rule_ref(&self) -> bool {
        self.min == edge_type::RULE_REF
    }

    /// Whether the edge accepts the end-of-string token.
    #[must_use]
    pub fn is_eos(&self) -> bool {
        self.min == edge_type::EOS
    }

    /// Whether the edge is a repeated rule reference.
    #[must_use]
    pub fn is_repeat_ref(&self) -> bool {
        self.min == edge_type::REPEAT_REF
    }

    /// Whether the edge accepts a token set.
    #[must_use]
    pub fn is_token(&self) -> bool {
        self.min == edge_type::TOKEN
    }

    /// Whether the edge accepts any token outside a set.
    #[must_use]
    pub fn is_exclude_token(&self) -> bool {
        self.min == edge_type::EXCLUDE_TOKEN
    }

    /// The referenced rule id, or `-1` if this is not a rule reference.
    #[must_use]
    pub fn ref_rule_id(&self) -> i32 {
        if self.is_rule_ref() {
            self.max
        } else {
            -1
        }
    }

    /// Whether the edge carries an index into `edge_aux_data`.
    #[must_use]
    pub fn is_aux_edge(&self) -> bool {
        self.is_repeat_ref() || self.is_token() || self.is_exclude_token()
    }

    /// The `edge_aux_data` index for an aux edge, or `-1` otherwise.
    #[must_use]
    pub fn aux_index(&self) -> i32 {
        if self.is_aux_edge() {
            self.max
        } else {
            -1
        }
    }

    /// The `(min, max)` range key used to sort/compare edges by range only.
    #[must_use]
    pub fn range_key(&self) -> (i32, i32) {
        (self.min, self.max)
    }
}

/// View into `edge_aux_data` for a repeat edge (`[rule_id, lower, upper]`).
#[derive(Debug, Clone, Copy)]
pub struct RepeatEdgeRef<'a> {
    /// The backing `[rule_id, lower, upper]` slice.
    pub data: &'a [i32],
}

impl RepeatEdgeRef<'_> {
    /// The repeated rule id.
    #[must_use]
    pub fn rule_id(&self) -> i32 {
        self.data[0]
    }
    /// The minimum repetition count.
    #[must_use]
    pub fn lower(&self) -> i32 {
        self.data[1]
    }
    /// The maximum repetition count (`-1` is unbounded).
    #[must_use]
    pub fn upper(&self) -> i32 {
        self.data[2]
    }
}

/// View into `edge_aux_data` for a token edge (`[count, ids...]`).
#[derive(Debug, Clone, Copy)]
pub struct TokenEdgeRef<'a> {
    /// The backing `[count, ids...]` slice.
    pub data: &'a [i32],
}

impl TokenEdgeRef<'_> {
    /// The accepted token ids.
    #[must_use]
    pub fn token_ids(&self) -> &[i32] {
        &self.data[1..=self.data[0] as usize]
    }
    /// Whether `token_id` is in the set.
    #[must_use]
    pub fn contains(
        &self,
        token_id: i32,
    ) -> bool {
        self.token_ids().contains(&token_id)
    }
}

/// View into `edge_aux_data` for an exclude-token edge (`[count, ids...]`).
#[derive(Debug, Clone, Copy)]
pub struct ExcludeTokenEdgeRef<'a> {
    /// The backing `[count, ids...]` slice.
    pub data: &'a [i32],
}

impl ExcludeTokenEdgeRef<'_> {
    /// The excluded token ids.
    #[must_use]
    pub fn token_ids(&self) -> &[i32] {
        &self.data[1..=self.data[0] as usize]
    }
    /// Whether `token_id` is in the excluded set.
    #[must_use]
    pub fn contains(
        &self,
        token_id: i32,
    ) -> bool {
        self.token_ids().contains(&token_id)
    }
    /// Whether `token_id` is accepted (i.e. not excluded).
    #[must_use]
    pub fn accepts(
        &self,
        token_id: i32,
    ) -> bool {
        !self.contains(token_id)
    }
}
