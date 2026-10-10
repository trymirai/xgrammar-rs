//! `NamedGrammars` binding — the grammars a Lark source references as `@name`.

use std::sync::{Arc, Mutex, MutexGuard};

use xgrammar::converter::NamedGrammar;

use crate::grammar::Grammar;

/// Grammars and Lark sources referenced by name from a Lark grammar, passed to
/// `Grammar.from_lark` and `GrammarCompiler.compile_lark`.
#[bindings::export(Class)]
#[derive(Debug, Clone, Default)]
pub struct NamedGrammars {
    entries: Arc<Mutex<Vec<NamedGrammar>>>,
}

impl NamedGrammars {
    /// The entries, in insertion order.
    pub fn entries(&self) -> MutexGuard<'_, Vec<NamedGrammar>> {
        self.entries.lock().expect("named grammars mutex poisoned")
    }
}

#[bindings::export(Implementation)]
impl NamedGrammars {
    /// Creates an empty set of named grammars.
    #[bindings::export(Method(Constructor))]
    pub fn new() -> NamedGrammars {
        NamedGrammars::default()
    }

    /// Adds `grammar`, referenced as `@name` (without the `@`).
    #[bindings::export(Method)]
    pub fn add_grammar(
        &self,
        name: String,
        grammar: &Grammar,
    ) {
        self.entries().push(NamedGrammar::Grammar {
            name,
            grammar: Box::new(grammar.as_ref().clone()),
        });
    }

    /// Adds a Lark source with its own `start` rule, referenced as `@name` (without the `@`).
    #[bindings::export(Method)]
    pub fn add_lark(
        &self,
        name: String,
        source: String,
    ) {
        self.entries().push(NamedGrammar::Lark {
            name,
            source,
        });
    }
}
