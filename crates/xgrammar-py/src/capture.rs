//! `Capture` record — one value captured by a named grammar rule.

/// The text matched by a capturing rule, as raw bytes: a capture may end inside a UTF-8
/// character.
#[bindings::export(Structure)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// The capture name.
    pub name: String,
    /// The captured bytes.
    pub value: Vec<u8>,
}

impl From<(String, Vec<u8>)> for Capture {
    fn from((name, value): (String, Vec<u8>)) -> Self {
        Self {
            name,
            value,
        }
    }
}
