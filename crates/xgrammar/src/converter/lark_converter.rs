//! Converts Lark syntax into the grammar IR.

use super::{
    lark_compiler::LarkCompiler, lark_error::LarkError, lark_parser::LarkParser, named_grammar::NamedGrammar,
    named_grammar_registry::NamedGrammarRegistry,
};
use crate::{grammar::Grammar, tokenizer::TokenizerInfo};

impl Grammar {
    /// Constructs a grammar from Lark syntax. The root rule must be named `start`.
    ///
    /// `tokenizer_info` resolves named special tokens (`<|...|>`) and the `<[*]>` wildcard.
    /// `named_grammars` are grammar objects or Lark sources that can be referenced with `@name`.
    ///
    /// # Errors
    /// Returns a [`LarkError`] if a named grammar is invalid or the Lark source is malformed or
    /// uses an unsupported feature.
    pub fn from_lark(
        lark: &str,
        tokenizer_info: Option<&TokenizerInfo>,
        named_grammars: &[NamedGrammar],
    ) -> Result<Grammar, LarkError> {
        let mut registry = NamedGrammarRegistry::default();
        for named_grammar in named_grammars {
            let name = named_grammar.name();
            if name.is_empty() {
                return Err(LarkError::NamedGrammar("Named grammar names must not be empty".to_owned()));
            }
            if !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
                return Err(LarkError::NamedGrammar(format!(
                    "Invalid named grammar name '{name}': names may contain only letters, digits, underscores, and \
                     hyphens"
                )));
            }
            if registry.inputs.insert(name, named_grammar).is_some() {
                return Err(LarkError::NamedGrammar(format!("Duplicate named grammar '{name}'")));
            }
        }
        let document = LarkParser::parse(lark)?;
        LarkCompiler::compile(lark, document, tokenizer_info, &mut registry)
    }
}
