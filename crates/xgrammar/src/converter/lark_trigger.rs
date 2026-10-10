//! The marker that ends a lazy Lark rule.

/// A lazy-rule trigger: a fixed string or a set of token ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LarkTrigger {
    /// A fixed byte string.
    String(Vec<u8>),
    /// A set of token ids.
    Token(Vec<i32>),
}
