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

    /// The built-in JSON grammar (a port of `Grammar::BuiltinJSONGrammar`).
    ///
    /// # Panics
    /// Never in practice — the embedded grammar is a valid constant.
    #[must_use]
    pub fn builtin_json_grammar() -> Grammar {
        Grammar::from_ebnf(BUILTIN_JSON_GRAMMAR, "root")
            .expect("the builtin JSON grammar is valid")
    }
}

/// The EBNF source of [`Grammar::builtin_json_grammar`] — a verbatim copy of
/// `kJSONGrammarString` in `cpp/grammar.cc`.
const BUILTIN_JSON_GRAMMAR: &str = r#"
root ::= (
    "{" [ \n\t]* members_and_embrace |
    "[" [ \n\t]* elements_or_embrace
)
value_non_str ::= (
    "{" [ \n\t]* members_and_embrace |
    "[" [ \n\t]* elements_or_embrace |
    "0" fraction exponent |
    [1-9] [0-9]* fraction exponent |
    "-" [0-9] fraction exponent |
    "-" [1-9] [0-9]* fraction exponent |
    "true" |
    "false" |
    "null"
) (= [ \n\t]* member_suffix_suffix)
members_and_embrace ::= ("\"" characters_and_colon [ \n\t]* members_suffix | "}") (= [ \n\t,}\]])
members_suffix ::= (
    value_non_str [ \n\t]* member_suffix_suffix |
    "\"" characters_and_embrace |
    "\"" characters_and_comma [ \n\t]* "\"" characters_and_colon [ \n\t]* members_suffix
) (= [ \n\t,}\]])
member_suffix_suffix ::= (
    "}" |
    "," [ \n\t]* "\"" characters_and_colon [ \n\t]* members_suffix
) (= [ \n\t,}\]])
elements_or_embrace ::= (
    "{" [ \n\t]* members_and_embrace elements_rest [ \n\t]* "]" |
    "[" [ \n\t]* elements_or_embrace elements_rest [ \n\t]* "]" |
    "\"" characters_item elements_rest [ \n\t]* "]" |
    "0" fraction exponent elements_rest [ \n\t]* "]" |
    [1-9] [0-9]* fraction exponent elements_rest [ \n\t]* "]" |
    "-" "0" fraction exponent elements_rest [ \n\t]* "]" |
    "-" [1-9] [0-9]* fraction exponent elements_rest [ \n\t]* "]" |
    "true" elements_rest [ \n\t]* "]" |
    "false" elements_rest [ \n\t]* "]" |
    "null" elements_rest [ \n\t]* "]" |
    "]"
)
elements ::= (
    "{" [ \n\t]* members_and_embrace elements_rest |
    "[" [ \n\t]* elements_or_embrace elements_rest |
    "\"" characters_item elements_rest |
    "0" fraction exponent elements_rest |
    [1-9] [0-9]* fraction exponent elements_rest |
    "-" [0-9] fraction exponent elements_rest |
    "-" [1-9] [0-9]* fraction exponent elements_rest |
    "true" elements_rest |
    "false" elements_rest |
    "null" elements_rest
)
elements_rest ::= (
    "" |
    [ \n\t]* "," [ \n\t]* elements
)
characters_and_colon ::= (
    "\"" [ \n\t]* ":" |
    [^"\\\x00-\x1F] characters_and_colon |
    "\\" escape characters_and_colon
) (=[ \n\t]* [\"{[0-9tfn-])
characters_and_comma ::= (
    "\"" [ \n\t]* "," |
    [^"\\\x00-\x1F] characters_and_comma |
    "\\" escape characters_and_comma
) (=[ \n\t]* "\"")
characters_and_embrace ::= (
    "\"" [ \n\t]* "}" |
    [^"\\\x00-\x1F] characters_and_embrace |
    "\\" escape characters_and_embrace
) (=[ \n\t]* [},])
characters_item ::= (
    "\"" |
    [^"\\\x00-\x1F] characters_item |
    "\\" escape characters_item
) (= [ \n\t]* [,\]])
escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
fraction ::= "" | "." [0-9] [0-9]*
exponent ::= "" |  "e" sign [0-9] [0-9]* | "E" sign [0-9] [0-9]*
sign ::= "" | "+" | "-"
"#;
