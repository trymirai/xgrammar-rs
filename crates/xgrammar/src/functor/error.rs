//! Errors raised by grammar passes that reject their input.

/// A grammar pass rejected its input grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FunctorError {
    /// A repetition range with a negative lower bound, or an upper bound below the lower one.
    #[error("Invalid repetition range {{{lower}, {upper}}}")]
    InvalidRepetitionRange {
        /// The minimum repetition count.
        lower: i64,
        /// The maximum repetition count, or `-1` for unbounded.
        upper: i64,
    },
    /// A rule body could not be compiled into an automaton (e.g. an invalid regex).
    #[error("{0}")]
    FsmBuild(String),
    /// A lazy rule whose body is not a choices of terminal-like sequences.
    #[error("lazy rule '{rule_name}' must have a terminal-like body")]
    LazyRuleBodyNotChoices {
        /// The name of the lazy rule.
        rule_name: String,
    },
    /// A lazy rule whose body contains an element that does not compile into the rule's own FSM.
    #[error(
        "lazy rule '{rule_name}' must have a terminal-like body (strings, character classes, and regexes that \
         compile to a single FSM); rule references and repetition ranges are not supported"
    )]
    LazyRuleElementNotTerminal {
        /// The name of the lazy rule.
        rule_name: String,
    },
}
