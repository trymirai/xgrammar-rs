//! EBNF lexer token kinds — a port of `EBNFLexer::TokenType` in `cpp/grammar_parser.h`.

/// The kind of a lexical token produced by the EBNF lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// The name of a rule definition (the identifier left of `::=`).
    RuleName,
    /// A reference to a rule or a macro name.
    Identifier,
    /// A `"..."` string literal.
    StringLiteral,
    /// `true` or `false`.
    BooleanLiteral,
    /// An integer literal.
    IntegerLiteral,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `|`
    Pipe,
    /// `,`
    Comma,
    /// End of input.
    EndOfFile,
    /// `::=`
    Assign,
    /// `=`
    Equal,
    /// `*`
    Star,
    /// `+`
    Plus,
    /// `?`
    Question,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `-`
    Dash,
    /// `^`
    Caret,
    /// A literal character inside a character class (possibly an escaped non-special char).
    CharInCharClass,
    /// A special escape inside a character class, e.g. `\S`, `\d`.
    EscapeInCharClass,
    /// `(=` — the start of a lookahead assertion.
    LookaheadLParen,
}
