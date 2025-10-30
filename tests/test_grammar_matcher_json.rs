use serial_test::serial;
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
};

fn matcher_from_grammar(grammar: &Grammar) -> GrammarMatcher {
    let empty_vocab: Vec<&str> = vec![];
    let stop_ids: Option<Box<[i32]>> = None;
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &stop_ids, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);
    let compiled = compiler.compile_grammar(grammar);
    GrammarMatcher::new(&compiled, None, true, -1)
}

fn is_grammar_accept_string(
    grammar: &Grammar,
    input: &str,
) -> bool {
    let mut matcher = matcher_from_grammar(grammar);
    let accepted = matcher.accept_string(input, false);
    accepted && matcher.is_terminated()
}

#[test]
#[serial]
fn test_json_accept_and_refuse() {
    let g = Grammar::builtin_json_grammar();
    let accepted = [
        "{\"name\": \"John\"}",
        "{ \"name\" : \"John\" }",
        "{}",
        "[]",
        "{\"name\": \"Alice\", \"age\": 30, \"city\": \"New York\"}",
        "{\"name\": \"Mike\", \"hobbies\": [\"reading\", \"cycling\", \"hiking\"]}",
        "{\"name\": \"Emma\", \"address\": {\"street\": \"Maple Street\", \"city\": \"Boston\"}}",
        "[{\"name\": \"David\"}, {\"name\": \"Sophia\"}]",
        "{\"name\": \"William\", \"age\": null, \"married\": true, \"children\": [\"Liam\", \"Olivia\"], \"hasPets\": false}",
        "{\"name\": \"Olivia\", \"contact\": {\"email\": \"olivia@example.com\", \"address\": {\"city\": \"Chicago\", \"zipcode\": \"60601\"}}}",
        "{\"name\": \"Liam\", \"skills\": [\"Java\", \"Python\"], \"experience\": [{\"company\": \"CompanyA\", \"years\": 5}, {\"company\": \"CompanyB\", \"years\": 3}]}",
        "{\"person\": {\"name\": \"Ethan\", \"age\": 40}, \"education\": {\"degree\": \"Masters\", \"university\": \"XYZ University\"}, \"work\": [{\"company\": \"ABC Corp\", \"position\": \"Manager\"}, {\"company\": \"DEF Corp\", \"position\": \"Senior Manager\"}]}",
        "{\"name\": \"Charlotte\", \"details\": {\"personal\": {\"age\": 35, \"hobbies\": [\"gardening\", \"painting\"]}, \"professional\": {\"occupation\": \"Engineer\", \"skills\": [\"CAD\", \"Project Management\"], \"projects\": [{\"name\": \"Project A\", \"status\": \"Completed\"}, {\"name\": \"Project B\", \"status\": \"In Progress\"}]}}}",
    ];
    for s in accepted {
        assert!(is_grammar_accept_string(&g, s), "{}", s);
    }

    let refused = [
        r#"{ name: "John" }"#,
        r#"{ "name": "John" } "#,
        r#"{ "name": "John", "age": 30, }"#,
        r#"{ "name": "John", "address": { "street": "123 Main St", "city": "New York" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling",], }"#,
        r#"{ "name": "John", "age": 30.5.7 }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", { "type": "outdoor", "list": ["hiking", "swimming",]}] }"#,
        r#"{ "name": "John", "age": 30, "status": "\P\J" }"#,
    ];
    for s in refused {
        assert!(!is_grammar_accept_string(&g, s), "{}", s);
    }
}
