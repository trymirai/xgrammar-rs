//! Port of `xgrammar/tests/cpp/test_parser.cc` (the `XGrammarLexerTest` suite).
//!
//! The two "invalid UTF-8 byte sequence" sub-cases of `lexer_error_cases` are omitted: the
//! safe `tokenize(&str)` API cannot carry non-UTF-8 input, so that error path is
//! unreachable through the public API.

use xgrammar::parser::{Token, TokenType, TokenValue, tokenize};

fn lex(input: &str) -> Vec<Token> {
    tokenize(input).unwrap()
}

fn lex_err(input: &str) -> String {
    tokenize(input).unwrap_err().to_string()
}

fn cp(c: char) -> TokenValue {
    TokenValue::Codepoint(c as i32)
}

#[test]
fn basic_tokenization() {
    let input = "rule1 ::= \"string\" | [a-z] | 123 | (expr) | {1,3} | * | + | ? | true | false";
    let tokens = lex(input);
    assert_eq!(tokens.len(), 32); // 31 tokens + EOF

    assert_eq!(tokens[0].ty, TokenType::RuleName);
    assert_eq!(tokens[0].lexeme, "rule1");

    assert_eq!(tokens[1].ty, TokenType::Assign);
    assert_eq!(tokens[1].lexeme, "::=");

    assert_eq!(tokens[2].ty, TokenType::StringLiteral);
    assert_eq!(tokens[2].lexeme, "\"string\"");
    assert_eq!(tokens[2].value, TokenValue::Str("string".to_owned()));

    assert_eq!(tokens[3].ty, TokenType::Pipe);

    assert_eq!(tokens[4].ty, TokenType::LBracket);
    assert_eq!(tokens[5].ty, TokenType::CharInCharClass);
    assert_eq!(tokens[5].lexeme, "a");
    assert_eq!(tokens[5].value, cp('a'));
    assert_eq!(tokens[6].ty, TokenType::Dash);
    assert_eq!(tokens[7].ty, TokenType::CharInCharClass);
    assert_eq!(tokens[7].lexeme, "z");
    assert_eq!(tokens[7].value, cp('z'));
    assert_eq!(tokens[8].ty, TokenType::RBracket);

    assert_eq!(tokens[9].ty, TokenType::Pipe);

    assert_eq!(tokens[10].ty, TokenType::IntegerLiteral);
    assert_eq!(tokens[10].lexeme, "123");
    assert_eq!(tokens[10].value, TokenValue::Int(123));

    assert_eq!(tokens[11].ty, TokenType::Pipe);

    assert_eq!(tokens[12].ty, TokenType::LParen);
    assert_eq!(tokens[13].ty, TokenType::Identifier);
    assert_eq!(tokens[14].ty, TokenType::RParen);

    assert_eq!(tokens[15].ty, TokenType::Pipe);

    assert_eq!(tokens[16].ty, TokenType::LBrace);
    assert_eq!(tokens[17].ty, TokenType::IntegerLiteral);
    assert_eq!(tokens[18].ty, TokenType::Comma);
    assert_eq!(tokens[19].ty, TokenType::IntegerLiteral);
    assert_eq!(tokens[20].ty, TokenType::RBrace);

    assert_eq!(tokens[21].ty, TokenType::Pipe);
    assert_eq!(tokens[22].ty, TokenType::Star);
    assert_eq!(tokens[23].ty, TokenType::Pipe);
    assert_eq!(tokens[24].ty, TokenType::Plus);
    assert_eq!(tokens[25].ty, TokenType::Pipe);
    assert_eq!(tokens[26].ty, TokenType::Question);
    assert_eq!(tokens[27].ty, TokenType::Pipe);

    assert_eq!(tokens[28].ty, TokenType::BooleanLiteral);
    assert_eq!(tokens[28].lexeme, "true");
    assert_eq!(tokens[28].value, TokenValue::Bool(true));

    assert_eq!(tokens[29].ty, TokenType::Pipe);

    assert_eq!(tokens[30].ty, TokenType::BooleanLiteral);
    assert_eq!(tokens[30].lexeme, "false");
    assert_eq!(tokens[30].value, TokenValue::Bool(false));

    assert_eq!(tokens[31].ty, TokenType::EndOfFile);
}

#[test]
fn comments_and_whitespace() {
    let input =
        "rule1 ::= expr1 # This is a comment\n  | expr2 # Another comment";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 6); // 5 tokens + EOF
    assert_eq!(tokens[0].ty, TokenType::RuleName);
    assert_eq!(tokens[0].lexeme, "rule1");
    assert_eq!(tokens[1].ty, TokenType::Assign);
    assert_eq!(tokens[2].ty, TokenType::Identifier);
    assert_eq!(tokens[2].lexeme, "expr1");
    assert_eq!(tokens[3].ty, TokenType::Pipe);
    assert_eq!(tokens[4].ty, TokenType::Identifier);
    assert_eq!(tokens[4].lexeme, "expr2");
}

#[test]
fn string_literals() {
    let input = "rule ::= \"normal string\" | \"escaped \\\"quotes\\\"\" | \"\\n\\r\\t\\\\\"";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 8); // 7 tokens + EOF
    assert_eq!(tokens[2].ty, TokenType::StringLiteral);
    assert_eq!(tokens[2].value, TokenValue::Str("normal string".to_owned()));

    assert_eq!(tokens[4].ty, TokenType::StringLiteral);
    assert_eq!(
        tokens[4].value,
        TokenValue::Str("escaped \"quotes\"".to_owned())
    );

    assert_eq!(tokens[6].ty, TokenType::StringLiteral);
    assert_eq!(tokens[6].value, TokenValue::Str("\n\r\t\\".to_owned()));
}

#[test]
fn character_classes() {
    let input = "rule ::= [a-z] | [0-9] | [^a-z] | [\\-\\]\\\\] | [\\u0041-\\u005A] | [测试] | \
                 [\\t\\r\\n] | [\\b\\f]";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 49); // 48 tokens + EOF

    // [a-z]
    assert_eq!(tokens[2].ty, TokenType::LBracket);
    assert_eq!(tokens[3].ty, TokenType::CharInCharClass);
    assert_eq!(tokens[3].lexeme, "a");
    assert_eq!(tokens[3].value, cp('a'));
    assert_eq!(tokens[4].ty, TokenType::Dash);
    assert_eq!(tokens[5].ty, TokenType::CharInCharClass);
    assert_eq!(tokens[5].lexeme, "z");
    assert_eq!(tokens[5].value, cp('z'));
    assert_eq!(tokens[6].ty, TokenType::RBracket);

    assert_eq!(tokens[7].ty, TokenType::Pipe);

    // [0-9]
    assert_eq!(tokens[8].ty, TokenType::LBracket);
    assert_eq!(tokens[9].value, cp('0'));
    assert_eq!(tokens[10].ty, TokenType::Dash);
    assert_eq!(tokens[11].value, cp('9'));
    assert_eq!(tokens[12].ty, TokenType::RBracket);

    assert_eq!(tokens[13].ty, TokenType::Pipe);

    // [^a-z]
    assert_eq!(tokens[14].ty, TokenType::LBracket);
    assert_eq!(tokens[15].ty, TokenType::Caret);
    assert_eq!(tokens[16].value, cp('a'));
    assert_eq!(tokens[17].ty, TokenType::Dash);
    assert_eq!(tokens[18].value, cp('z'));
    assert_eq!(tokens[19].ty, TokenType::RBracket);

    assert_eq!(tokens[20].ty, TokenType::Pipe);

    // [\-\]\\]
    assert_eq!(tokens[21].ty, TokenType::LBracket);
    assert_eq!(tokens[22].ty, TokenType::CharInCharClass);
    assert_eq!(tokens[22].lexeme, "\\-");
    assert_eq!(tokens[22].value, cp('-'));
    assert_eq!(tokens[23].lexeme, "\\]");
    assert_eq!(tokens[23].value, cp(']'));
    assert_eq!(tokens[24].lexeme, "\\\\");
    assert_eq!(tokens[24].value, cp('\\'));
    assert_eq!(tokens[25].ty, TokenType::RBracket);

    assert_eq!(tokens[26].ty, TokenType::Pipe);

    // [A-Z]
    assert_eq!(tokens[27].ty, TokenType::LBracket);
    assert_eq!(tokens[28].lexeme, "\\u0041");
    assert_eq!(tokens[28].value, TokenValue::Codepoint(0x41));
    assert_eq!(tokens[29].ty, TokenType::Dash);
    assert_eq!(tokens[30].lexeme, "\\u005A");
    assert_eq!(tokens[30].value, TokenValue::Codepoint(0x5A));
    assert_eq!(tokens[31].ty, TokenType::RBracket);

    assert_eq!(tokens[32].ty, TokenType::Pipe);

    // [测试]
    assert_eq!(tokens[33].ty, TokenType::LBracket);
    assert_eq!(tokens[34].lexeme, "测");
    assert_eq!(tokens[34].value, TokenValue::Codepoint(0x6D4B));
    assert_eq!(tokens[35].lexeme, "试");
    assert_eq!(tokens[35].value, TokenValue::Codepoint(0x8BD5));
    assert_eq!(tokens[36].ty, TokenType::RBracket);

    assert_eq!(tokens[37].ty, TokenType::Pipe);

    // [\t\r\n]
    assert_eq!(tokens[38].ty, TokenType::LBracket);
    assert_eq!(tokens[39].lexeme, "\\t");
    assert_eq!(tokens[39].value, cp('\t'));
    assert_eq!(tokens[40].lexeme, "\\r");
    assert_eq!(tokens[40].value, cp('\r'));
    assert_eq!(tokens[41].lexeme, "\\n");
    assert_eq!(tokens[41].value, cp('\n'));
    assert_eq!(tokens[42].ty, TokenType::RBracket);

    assert_eq!(tokens[43].ty, TokenType::Pipe);

    // [\b\f]
    assert_eq!(tokens[44].ty, TokenType::LBracket);
    assert_eq!(tokens[45].lexeme, "\\b");
    assert_eq!(tokens[45].value, TokenValue::Codepoint(0x08));
    assert_eq!(tokens[46].lexeme, "\\f");
    assert_eq!(tokens[46].value, TokenValue::Codepoint(0x0C));
    assert_eq!(tokens[47].ty, TokenType::RBracket);
}

#[test]
fn boolean_values() {
    let input = "rule ::= true | false";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 6); // 5 tokens + EOF
    assert_eq!(tokens[2].ty, TokenType::BooleanLiteral);
    assert_eq!(tokens[2].lexeme, "true");
    assert_eq!(tokens[2].value, TokenValue::Bool(true));

    assert_eq!(tokens[4].ty, TokenType::BooleanLiteral);
    assert_eq!(tokens[4].lexeme, "false");
    assert_eq!(tokens[4].value, TokenValue::Bool(false));
}

#[test]
fn lookahead_assertion() {
    let input = "rule ::= \"a\" (= lookahead)";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 7); // 6 tokens + EOF
    assert_eq!(tokens[3].ty, TokenType::LookaheadLParen);
    assert_eq!(tokens[3].lexeme, "(=");
    assert_eq!(tokens[5].ty, TokenType::RParen);
}

#[test]
fn line_and_column_tracking() {
    let input = "rule1 ::= expr1\nrule2 ::= expr2";
    let tokens = lex(input);

    assert_eq!(tokens.len(), 7); // 6 tokens + EOF

    // First line tokens
    assert_eq!((tokens[0].line, tokens[0].column), (1, 1));
    assert_eq!((tokens[1].line, tokens[1].column), (1, 7));
    assert_eq!((tokens[2].line, tokens[2].column), (1, 11));

    // Second line tokens
    assert_eq!((tokens[3].line, tokens[3].column), (2, 1));
    assert_eq!((tokens[4].line, tokens[4].column), (2, 7));
    assert_eq!((tokens[5].line, tokens[5].column), (2, 11));
}

#[test]
fn complex_grammar() {
    let input = "# JSON Grammar\n\
        root ::= value\n\
        value ::= object | array | string | number | \"true\" | \"false\" | \"null\"\n\
        object ::= \"{\" (member (\",\" member)*)? \"}\"\n\
        member ::= string \":\" value\n\
        array ::= \"[\" (value (\",\" value)*)? \"]\"\n\
        string ::= \"\\\"\" char* \"\\\"\"\n\
        char ::= [^\"\\\\] | \"\\\\\\\"\"\n\
        number ::= int frac? exp?\n\
        int ::= \"-\"? ([1-9] [0-9]* | \"0\")\n\
        frac ::= \".\" [0-9]+\n\
        exp ::= [eE] [+\\-]? [0-9]+";

    let tokens = lex(input);
    assert!(tokens.len() > 50);
    assert_eq!(tokens.last().unwrap().ty, TokenType::EndOfFile);
}

#[test]
fn edge_cases() {
    // Empty input
    {
        let tokens = lex("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].ty, TokenType::EndOfFile);
    }

    // Only whitespace and comments
    {
        let tokens = lex("  \t\n # Comment\n  # Another comment");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].ty, TokenType::EndOfFile);
    }

    // Various newline formats
    {
        let tokens = lex(
            "rule1 ::= expr1\nrule2 ::= expr2\r\nrule3 ::= expr3\rrule4 ::= expr4",
        );
        assert_eq!(tokens.len(), 13); // 12 tokens + EOF
    }

    // Integer boundary (15 digits, the max allowed)
    {
        let tokens = lex("rule ::= 999999999999999");
        assert_eq!(tokens.len(), 4); // 3 tokens + EOF
        assert_eq!(tokens[2].ty, TokenType::IntegerLiteral);
        assert_eq!(tokens[2].lexeme, "999999999999999");
    }

    // Special identifiers
    {
        let tokens = lex("rule-name ::= _special.identifier-123");
        assert_eq!(tokens.len(), 4); // 3 tokens + EOF
        assert_eq!(tokens[0].ty, TokenType::RuleName);
        assert_eq!(tokens[0].lexeme, "rule-name");
        assert_eq!(tokens[2].ty, TokenType::Identifier);
        assert_eq!(tokens[2].lexeme, "_special.identifier-123");
    }
}

#[test]
fn quantifier_tokens() {
    let input =
        "rule ::= expr? | expr* | expr+ | expr{1} | expr{1,} | expr{1,5}";
    let tokens = lex(input);

    assert_eq!(tokens[3].ty, TokenType::Question);
    assert_eq!(tokens[6].ty, TokenType::Star);
    assert_eq!(tokens[9].ty, TokenType::Plus);
    assert_eq!(tokens[12].ty, TokenType::LBrace);
    assert_eq!(tokens[13].ty, TokenType::IntegerLiteral);
    assert_eq!(tokens[14].ty, TokenType::RBrace);
    assert_eq!(tokens[17].ty, TokenType::LBrace);
    assert_eq!(tokens[18].ty, TokenType::IntegerLiteral);
    assert_eq!(tokens[19].ty, TokenType::Comma);
    assert_eq!(tokens[20].ty, TokenType::RBrace);
}

#[test]
fn utf8_handling() {
    // Build the input without literal `\u`/`\U` escapes in the source, which the harness
    // would otherwise decode; the backslash is injected at runtime.
    let bs = char::from_u32(0x5C).unwrap();
    let input =
        format!("rule ::= \"UTF-8: {bs}u00A9 {bs}u2603 {bs}U0001F600\"");
    let tokens = lex(&input);

    assert_eq!(tokens.len(), 4); // 3 tokens + EOF
    assert_eq!(tokens[2].ty, TokenType::StringLiteral);
    assert_eq!(tokens[2].value, TokenValue::Str("UTF-8: © ☃ 😀".to_owned()));
}

#[test]
fn lexer_error_cases() {
    // Unterminated string
    assert!(
        lex_err("rule ::= \"unterminated string")
            .contains("Expect \" in string literal")
    );

    // Unterminated character class
    assert!(lex_err("rule ::= [a-z").contains("Unterminated character class"));

    // Unterminated character class with escaped bracket
    assert!(
        lex_err("rule ::= [a-z\\-\\\\\\]")
            .contains("Unterminated character class")
    );

    // Invalid escape sequence in string
    assert!(lex_err("rule ::= \"\\z\"").contains("Invalid escape sequence"));

    // Newline in character class
    assert!(
        lex_err("rule ::= [a-z\n]")
            .contains("Character class should not contain newline")
    );

    // Invalid escape sequence in character class
    assert!(lex_err("rule ::= [\\z]").contains("Invalid escape sequence"));

    // Integer too large (> 1e15)
    assert!(
        lex_err("rule ::= expr{1000000000000000000}")
            .contains("Integer is too large")
    );

    // Unexpected character
    assert!(lex_err("rule ::= @").contains("Unexpected character"));

    // Unexpected colon
    assert!(lex_err("rule : expr").contains("Unexpected character: ':'"));

    // Assign preceded by a non-identifier
    assert!(
        lex_err("\"string\" ::= expr")
            .contains("Assign should be preceded by an identifier")
    );

    // Assign as the first token
    assert!(
        lex_err("::= expr").contains("Assign should not be the first token")
    );

    // Rule name not at the beginning of the line
    assert!(
        lex_err("token token ::= expr")
            .contains("The rule name should be at the beginning of the line")
    );
}
