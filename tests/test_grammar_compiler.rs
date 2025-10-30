use serial_test::serial;
use xgrammar::{GrammarCompiler, TokenizerInfo, VocabType};

fn get_allow_empty_rule_ids_via_json(
    compiled: &xgrammar::CompiledGrammar
) -> Vec<i32> {
    let s = compiled.serialize_json();
    let v: serde_json::Value =
        serde_json::from_str(&s).expect("valid JSON from SerializeJSON");
    v["grammar"]["allow_empty_rule_ids"]
        .as_array()
        .expect("allow_empty_rule_ids is array")
        .iter()
        .map(|x| x.as_i64().expect("int").try_into().unwrap())
        .collect()
}

#[test]
#[serial]
fn test_get_allow_empty_rule_ids() {
    let cases: &[(&str, &[i32])] = &[
        (
            r#"root ::= rule1 rule2 | "abc"
    rule1 ::= "abc" | ""
    rule2 ::= "def" rule3 | ""
    rule3 ::= "ghi""#,
            &[0, 1, 2],
        ),
        (
            r#"root ::= rule1 rule2 [a-z]*
    rule1 ::= "abc" | ""
    rule2 ::= "def" | """#,
            &[0, 1, 2],
        ),
        (
            r#"root ::= rule1 rule3
    rule1 ::= "abc" | ""
    rule2 ::= "def" | ""
    rule3 ::= rule1 rule2"#,
            &[0, 1, 2, 3],
        ),
        (
            r#"root ::= [a]* [b]* rule1
    rule1 ::= [abc]* [def]*
"#,
            &[0, 1],
        ),
    ];

    // Empty vocab is fine for this structural property
    let empty_vocab: Vec<&str> = vec![];
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);

    for (ebnf, expected) in cases.iter() {
        let cg = compiler.compile_grammar_from_ebnf(ebnf, "root");
        let ids = get_allow_empty_rule_ids_via_json(&cg);
        assert_eq!(&ids, expected);
    }
}
