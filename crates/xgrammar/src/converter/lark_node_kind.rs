//! Kinds of Lark expression nodes.

/// The kind of a [`super::lark_node::LarkNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LarkNodeKind {
    Sequence,
    Choice,
    Repeat,
    String,
    Regex,
    Range,
    Name,
    Json,
    RegexExt,
    NestedLark,
    SpecialToken,
    GrammarRef,
    Not,
}
