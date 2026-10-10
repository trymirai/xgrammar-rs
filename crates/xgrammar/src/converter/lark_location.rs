//! A 1-based position in a Lark source.

/// A 1-based line and byte column in a Lark source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LarkLocation {
    /// 1-based line number.
    pub line: i32,
    /// 1-based byte column.
    pub column: i32,
}

impl LarkLocation {
    /// The location of the first byte of a source.
    pub const START: Self = Self {
        line: 1,
        column: 1,
    };
}
