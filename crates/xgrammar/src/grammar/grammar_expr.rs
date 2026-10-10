//! A borrowed view over one grammar expression.

use super::{character_class_element::CharacterClassElement, grammar_expr_type::GrammarExprType};

/// A read-only view of a grammar expression: its [`GrammarExprType`] and its `i32` data
/// payload, borrowed from the grammar's flat buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrammarExpr<'a> {
    /// The expression kind.
    pub ty: GrammarExprType,
    /// The raw `i32` payload, laid out per the variant's documented format.
    pub data: &'a [i32],
}

impl<'a> GrammarExpr<'a> {
    /// Number of payload elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Decodes a [`GrammarExprType::ByteString`] payload into its bytes.
    #[must_use]
    pub fn byte_string(&self) -> Vec<u8> {
        debug_assert_eq!(self.ty, GrammarExprType::ByteString);
        self.data.iter().map(|&b| b as u8).collect()
    }

    /// Decodes a `RuleRef` / `CharacterClassStar`-of-rule payload's referenced rule id.
    #[must_use]
    pub fn rule_ref_id(&self) -> i32 {
        debug_assert_eq!(self.ty, GrammarExprType::RuleRef);
        self.data[0]
    }

    /// Decodes a character class / character-class-star payload into
    /// `(is_negative, ranges)`.
    #[must_use]
    pub fn character_class(&self) -> (bool, Vec<CharacterClassElement>) {
        debug_assert!(matches!(self.ty, GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar));
        let is_negative = self.data[0] != 0;
        let ranges = self.data[1..].chunks_exact(2).map(|c| CharacterClassElement::new(c[0], c[1])).collect();
        (is_negative, ranges)
    }

    /// Decodes a [`GrammarExprType::Regex`] payload's pattern bytes.
    #[must_use]
    pub fn regex_pattern(&self) -> Vec<u8> {
        debug_assert_eq!(self.ty, GrammarExprType::Regex);
        self.data[1..].iter().map(|&b| b as u8).collect()
    }

    /// Whether a [`GrammarExprType::Regex`] payload matches the body of a JSON string literal.
    #[must_use]
    pub fn regex_is_json_string(&self) -> bool {
        debug_assert_eq!(self.ty, GrammarExprType::Regex);
        self.data[0] != 0
    }

    /// Decodes a [`GrammarExprType::Substring`] payload into its byte-string chunks.
    #[must_use]
    pub fn substring_chunks(&self) -> Vec<Vec<u8>> {
        debug_assert_eq!(self.ty, GrammarExprType::Substring);
        let mut chunks = Vec::new();
        let mut index = 0;
        while index < self.data.len() {
            let chunk_len = self.data[index] as usize;
            index += 1;
            chunks.push(self.data[index..index + chunk_len].iter().map(|&b| b as u8).collect());
            index += chunk_len;
        }
        chunks
    }

    /// Decodes a [`GrammarExprType::Repeat`] payload into `(rule_id, min, max)`.
    #[must_use]
    pub fn repeat(&self) -> (i32, i32, i32) {
        debug_assert_eq!(self.ty, GrammarExprType::Repeat);
        (self.data[0], self.data[1], self.data[2])
    }
}
