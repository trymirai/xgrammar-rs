//! The normalization pipeline and the `Grammar::from_ebnf` constructor — a port of
//! `GrammarNormalizer` and `Grammar::FromEBNF`.

use super::{
    root_rule_renamer::root_rule_renamer,
    structure_normalizer::structure_normalizer,
};
use crate::{
    grammar::Grammar,
    parser::{EbnfError, ebnf_to_grammar_no_normalization},
};

/// Normalizes a grammar: rename the root to `root`, then structure-normalize it.
#[must_use]
pub fn grammar_normalizer(grammar: &Grammar) -> Grammar {
    structure_normalizer(&root_rule_renamer(grammar))
}

impl Grammar {
    /// Parses an EBNF grammar string and normalizes it.
    ///
    /// # Errors
    /// Returns [`EbnfError`] on a lexing or parsing failure.
    pub fn from_ebnf(
        ebnf_string: &str,
        root_rule_name: &str,
    ) -> Result<Grammar, EbnfError> {
        let parsed =
            ebnf_to_grammar_no_normalization(ebnf_string, root_rule_name)?;
        Ok(grammar_normalizer(&parsed))
    }
}
