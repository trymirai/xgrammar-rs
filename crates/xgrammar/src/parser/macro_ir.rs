//! Intermediate representation for macro arguments — a port of `EBNFParser::MacroIR`
//! in `cpp/grammar_parser.cc`.

/// A parsed macro argument value: a string, integer, boolean, identifier, or tuple of
/// nested values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroValue {
    /// A string literal.
    Str(String),
    /// An integer literal.
    Int(i64),
    /// A boolean literal.
    Bool(bool),
    /// A bare identifier (a rule reference).
    Identifier(String),
    /// A parenthesized tuple of values.
    Tuple(Vec<MacroValue>),
}

/// The positional and named arguments of a macro call.
#[derive(Debug, Clone, Default)]
pub struct MacroArguments {
    /// Positional arguments, in order.
    pub positional: Vec<MacroValue>,
    /// Named arguments, in encounter order.
    pub named: Vec<(String, MacroValue)>,
}

impl MacroArguments {
    /// Returns the value of the named argument `name`, if present.
    #[must_use]
    pub fn named(
        &self,
        name: &str,
    ) -> Option<&MacroValue> {
        self.named.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}
