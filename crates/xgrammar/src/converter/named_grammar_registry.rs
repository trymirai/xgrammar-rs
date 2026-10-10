//! Named grammars shared by a Lark conversion and the named Lark sources it compiles.

use std::collections::HashMap;

use super::named_grammar::NamedGrammar;
use crate::grammar::Grammar;

/// The named grammar inputs, the Lark sources compiled so far, and the chain of Lark sources
/// being compiled (for cycle detection).
#[derive(Debug, Default)]
pub struct NamedGrammarRegistry<'a> {
    /// The named grammars by name.
    pub inputs: HashMap<&'a str, &'a NamedGrammar>,
    /// Compiled named Lark sources by name.
    pub compiled: HashMap<String, Grammar>,
    /// The named Lark sources currently being compiled, outermost first.
    pub active: Vec<String>,
}

impl NamedGrammarRegistry<'_> {
    /// The grammar named `name`: the input grammar, or the compiled Lark source.
    #[must_use]
    pub fn get(
        &self,
        name: &str,
    ) -> Option<&Grammar> {
        match self.inputs.get(name)? {
            NamedGrammar::Grammar {
                grammar,
                ..
            } => Some(grammar.as_ref()),
            NamedGrammar::Lark {
                ..
            } => self.compiled.get(name),
        }
    }
}
