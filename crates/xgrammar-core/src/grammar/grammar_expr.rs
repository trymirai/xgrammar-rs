//! A borrowed view over one grammar expression — a port of
//! `Grammar::Impl::GrammarExpr` in `cpp/grammar_impl.h`.

use super::{
    character_class_element::CharacterClassElement,
    grammar_expr_type::GrammarExprType,
};

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
        debug_assert!(matches!(
            self.ty,
            GrammarExprType::CharacterClass
                | GrammarExprType::CharacterClassStar
        ));
        let is_negative = self.data[0] != 0;
        let ranges = self.data[1..]
            .chunks_exact(2)
            .map(|c| CharacterClassElement::new(c[0], c[1]))
            .collect();
        (is_negative, ranges)
    }

    /// Decodes a [`GrammarExprType::Repeat`] payload into `(rule_id, min, max)`.
    #[must_use]
    pub fn repeat(&self) -> (i32, i32, i32) {
        debug_assert_eq!(self.ty, GrammarExprType::Repeat);
        (self.data[0], self.data[1], self.data[2])
    }
}
