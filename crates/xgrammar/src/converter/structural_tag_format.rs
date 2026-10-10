//! Intermediate representation for a parsed structural tag.
//!
//! The full struct (including the resolver/analyzer-filled `resolved_*`/`detected_*`
//! fields) participates in equality/hashing so the converter can de-duplicate identical
//! sub-formats.

use super::{any_text_format::AnyTextFormat, any_tokens_format::AnyTokensFormat, json_schema_format::JsonSchemaFormat};

/// A token reference: a literal id or a vocabulary string (resolved to an id later).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IntOrString {
    Int(i32),
    Str(String),
}

/// `{"type":"const_string","value":.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstStringFormat {
    pub value: String,
}

/// `{"type":"grammar","grammar":<ebnf>}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GrammarFormat {
    pub grammar: String,
}

/// `{"type":"regex","pattern":<regex>}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegexFormat {
    pub pattern: String,
}

/// `{"type":"sequence","elements":[.]}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceFormat {
    pub elements: Vec<Format>,
    pub is_unlimited: bool,
}

/// `{"type":"or","elements":[.]}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrFormat {
    pub elements: Vec<Format>,
    pub is_unlimited: bool,
}

/// A tag's begin marker: a literal string or a token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagBegin {
    Str(String),
    Token(TokenFormat),
}

/// A tag's end marker: a set of literal strings or a token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagEnd {
    Strings(Vec<String>),
    Token(TokenFormat),
}

/// `{"type":"tag","begin":.,"content":.,"end":.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagFormat {
    pub begin: TagBegin,
    pub content: Box<Format>,
    pub end: TagEnd,
}

/// `{"type":"triggered_tags",.}`; `detected_end_strs` is filled by the analyzer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggeredTagsFormat {
    pub triggers: Vec<String>,
    pub tags: Vec<TagFormat>,
    pub excludes: Vec<String>,
    pub at_least_one: bool,
    pub stop_after_first: bool,
    pub detected_end_strs: Vec<String>,
}

/// `{"type":"tags_with_separator",.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagsWithSeparatorFormat {
    pub tags: Vec<TagFormat>,
    pub separator: String,
    pub at_least_one: bool,
    pub stop_after_first: bool,
}

/// `{"type":"optional","content":.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptionalFormat {
    pub content: Box<Format>,
}

/// `{"type":"plus","content":.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlusFormat {
    pub content: Box<Format>,
}

/// `{"type":"star","content":.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StarFormat {
    pub content: Box<Format>,
}

/// `{"type":"repeat","min":.,"max":.,"content":.}` (`max == -1` is unbounded).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepeatFormat {
    pub min: i32,
    pub max: i32,
    pub content: Box<Format>,
}

/// `{"type":"token","token":<int|string>}`; `resolved_token_id` is filled by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenFormat {
    pub token: IntOrString,
    pub resolved_token_id: i32,
}

impl TokenFormat {
    pub fn new(token: IntOrString) -> Self {
        // An integer token is its own resolved id; a string token resolves later.
        let resolved_token_id = match &token {
            IntOrString::Int(i) => *i,
            IntOrString::Str(_) => -1,
        };
        Self {
            token,
            resolved_token_id,
        }
    }
}

/// `{"type":"exclude_token","exclude_tokens":[.]}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ExcludeTokenFormat {
    pub exclude_tokens: Vec<IntOrString>,
    pub resolved_token_ids: Vec<i32>,
    pub detected_end_token_ids: Vec<i32>,
}

/// `{"type":"token_triggered_tags",.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenTriggeredTagsFormat {
    pub trigger_tokens: Vec<IntOrString>,
    pub tags: Vec<TagFormat>,
    pub exclude_tokens: Vec<IntOrString>,
    pub at_least_one: bool,
    pub stop_after_first: bool,
    pub resolved_trigger_token_ids: Vec<i32>,
    pub resolved_exclude_token_ids: Vec<i32>,
    pub detected_end_token_ids: Vec<i32>,
}

/// `{"type":"dispatch","rules":[[trigger, content], .],"loop":.,"excludes":[.]}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DispatchFormat {
    pub rules: Vec<(String, Box<Format>)>,
    pub loop_after_dispatch: bool,
    pub excludes: Vec<String>,
}

/// `{"type":"token_dispatch","rules":[[trigger, content], .],.}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenDispatchFormat {
    pub rules: Vec<(IntOrString, Box<Format>)>,
    pub loop_after_dispatch: bool,
    pub exclude_tokens: Vec<IntOrString>,
    pub resolved_trigger_token_ids: Vec<i32>,
    pub resolved_exclude_token_ids: Vec<i32>,
}

/// The tagged union of all structural-tag format kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    ConstString(ConstStringFormat),
    JsonSchema(JsonSchemaFormat),
    AnyText(AnyTextFormat),
    Grammar(GrammarFormat),
    Regex(RegexFormat),
    Sequence(SequenceFormat),
    Or(OrFormat),
    Tag(TagFormat),
    TriggeredTags(TriggeredTagsFormat),
    TagsWithSeparator(TagsWithSeparatorFormat),
    Optional(OptionalFormat),
    Plus(PlusFormat),
    Star(StarFormat),
    Repeat(RepeatFormat),
    Token(TokenFormat),
    ExcludeToken(ExcludeTokenFormat),
    AnyTokens(AnyTokensFormat),
    TokenTriggeredTags(TokenTriggeredTagsFormat),
    Dispatch(DispatchFormat),
    TokenDispatch(TokenDispatchFormat),
}
