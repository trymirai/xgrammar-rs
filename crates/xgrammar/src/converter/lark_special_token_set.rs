//! The token ids selected by a Lark special-token reference.

/// The token ids of a `<...>` special token, and whether they are excluded (`<[^...]>`).
#[derive(Debug, Clone, Default)]
pub struct LarkSpecialTokenSet {
    /// Whether the token ids are excluded rather than allowed.
    pub excluded: bool,
    /// The selected token ids.
    pub token_ids: Vec<i32>,
}
