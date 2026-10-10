//! A node of the parsed regex AST.

/// A node of the regex AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexNode {
    /// One atom: the empty string (`""`), a character class (`"[...]"`), or one literal
    /// character or escape sequence (the raw regex bytes).
    Leaf(Vec<u8>),
    /// `child*`.
    Star(Box<RegexNode>),
    /// `child+`.
    Plus(Box<RegexNode>),
    /// `child?`.
    Optional(Box<RegexNode>),
    /// An alternation of branches.
    Union(Vec<RegexNode>),
    /// A concatenation (a group, or a branch of an alternation).
    Bracket(Vec<RegexNode>),
    /// A `{lower, upper}` repetition, unrolled into the FSM (`upper == -1` is unbounded).
    Repeat {
        /// The repeated node.
        child: Box<RegexNode>,
        /// The minimum repetition count.
        lower: i32,
        /// The maximum repetition count, or `-1` for no upper bound.
        upper: i32,
    },
    /// A reference to a grammar rule, compiled into a rule-ref FSM edge.
    RuleRef(i32),
    /// A bounded repetition of a grammar rule, compiled into a repeat-ref FSM edge. The rule
    /// holds the repeated sub-pattern; the Earley parser counts the repetitions at runtime, so
    /// the FSM is not unrolled.
    RepeatSubrule {
        /// The repeated rule.
        rule_id: i32,
        /// The minimum repetition count.
        lower: i32,
        /// The maximum repetition count.
        upper: i32,
    },
}

impl RegexNode {
    /// Whether the node can match the empty string. Purely syntactic; no FSM is built.
    #[must_use]
    pub fn is_nullable(&self) -> bool {
        match self {
            Self::Leaf(regex) => regex.is_empty(),
            Self::Star(_) | Self::Optional(_) => true,
            Self::Plus(child) => child.is_nullable(),
            Self::Union(branches) => branches.iter().any(Self::is_nullable),
            Self::Bracket(children) => Self::is_nullable_sequence(children),
            Self::Repeat {
                child,
                lower,
                ..
            } => *lower == 0 || child.is_nullable(),
            Self::RuleRef(_) => false,
            Self::RepeatSubrule {
                lower,
                ..
            } => *lower == 0,
        }
    }

    /// Whether a sequence of nodes can match the empty string.
    #[must_use]
    pub fn is_nullable_sequence(nodes: &[RegexNode]) -> bool {
        nodes.iter().all(Self::is_nullable)
    }
}
