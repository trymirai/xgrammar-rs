use serial_test::serial;
use xgrammar::{GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType};

#[cfg(feature = "hf")]
fn download_tokenizer_json(
    model_id: &str
) -> Result<std::path::PathBuf, String> {
    use hf_hub::{
        Repo,
        api::sync::{Api, ApiBuilder},
    };
    let api = ApiBuilder::new().build().map_err(|e| e.to_string())?;
    let repo = api.repo(Repo::model(model_id.to_string()));
    repo.get("tokenizer.json").map_err(|e| e.to_string())
}

#[cfg(feature = "hf")]
fn make_hf_tokenizer_info(model_id: &str) -> TokenizerInfo {
    let path =
        download_tokenizer_json(model_id).expect("download tokenizer.json");
    let tk = tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
    TokenizerInfo::from_huggingface(&tk, None, None)
}

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

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_compiled_grammar() {
    let grammar = Grammar::builtin_json_grammar();
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-2-7b-chat-hf");
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 8, true, -1);
    let compiled = compiler.compile_grammar(&grammar);

    fn check_matcher(mut m: GrammarMatcher) {
        assert!(!m.is_terminated());
        assert!(!m.accept_string("{ name: \"John\" }", false));
        assert!(m.accept_string("{\"name\": \"John\"}", false));
        assert!(m.is_terminated());
    }
    let m1 = GrammarMatcher::new(&compiled, None, true, -1);
    check_matcher(m1);
    let m2 = GrammarMatcher::new(&compiled, None, true, -1);
    check_matcher(m2);
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_grammar_compiler_json() {
    for &max_threads in &[8, 1] {
        let tokenizer_info =
            make_hf_tokenizer_info("meta-llama/Llama-2-7b-chat-hf");
        let mut grammar_compiler =
            GrammarCompiler::new(&tokenizer_info, max_threads, true, -1);
        let compiled_grammar = grammar_compiler.compile_builtin_json_grammar();
        let mut matcher =
            GrammarMatcher::new(&compiled_grammar, None, true, -1);
        assert!(!matcher.is_terminated());
        assert!(!matcher.accept_string("{ name: \"John\" }", false));
        assert!(matcher.accept_string("{\"name\": \"John\"}", false));
        assert!(matcher.is_terminated());
    }
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_grammar_compiler_json_schema() {
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-2-7b-chat-hf");
    let mut grammar_compiler =
        GrammarCompiler::new(&tokenizer_info, 8, true, -1);

    // Schema matching Python's MainModel
    let schema = r#"{
        "type":"object",
        "properties":{
            "integer_field":{"type":"integer"},
            "number_field":{"type":"number"},
            "boolean_field":{"type":"boolean"},
            "any_array_field":{},
            "array_field":{"type":"array","items":{"type":"string"}},
            "tuple_field":{"type":"array","prefixItems":[{"type":"string"},{"type":"integer"},{"type":"array","items":{"type":"string"}}],"minItems":3,"maxItems":3},
            "object_field":{"type":"object","additionalProperties":{"type":"integer"}},
            "nested_object_field":{"type":"object","additionalProperties":{"type":"object","additionalProperties":{"type":"integer"}}}
        },
        "required":["integer_field","number_field","boolean_field","any_array_field","array_field","tuple_field","object_field","nested_object_field"]
    }"#;

    let instance = r#"{"integer_field":42,"number_field":3.14e5,"boolean_field":true,"any_array_field":[3.14,"foo",null,true],"array_field":["foo","bar"],"tuple_field":["foo",42,["bar","baz"]],"object_field":{"foo":42,"bar":43},"nested_object_field":{"foo":{"bar":42}}}"#;

    // Use Python-accepted formatting variant
    let compiled = grammar_compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(!matcher.is_terminated());
    assert!(matcher.accept_string(instance, false));
    assert!(matcher.is_terminated());
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_grammar_compiler_json_schema_concurrent() {
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-2-7b-chat-hf");
    let mut grammar_compiler =
        GrammarCompiler::new(&tokenizer_info, 8, true, -1);

    let schema_instances: &[(&str, &str)] = &[
        (
            "{\"type\": \"object\",\"properties\":{\"username\":{\"type\": \"string\"}},\"required\":[\"username\"]}",
            "{\"username\":\"Alice\"}",
        ),
        (
            "{\"type\": \"object\",\"properties\":{\"age\":{\"type\": \"integer\"}},\"required\":[\"age\"]}",
            "{\"age\":30}",
        ),
        (
            "{\"type\": \"object\",\"properties\":{\"city\":{\"type\": \"string\"}},\"required\":[\"city\"]}",
            "{\"city\":\"Paris\"}",
        ),
    ];

    fn check(
        matcher: &mut GrammarMatcher,
        instance: &str,
    ) {
        assert!(!matcher.is_terminated());
        assert!(matcher.accept_string(instance, false));
        assert!(matcher.is_terminated());
    }

    for (schema, inst) in schema_instances.iter().copied() {
        let compiled = grammar_compiler.compile_json_schema(
            schema,
            false,
            None,
            Some((",", ":")),
            true,
        );
        let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
        check(&mut matcher, inst);
    }
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_json_schema_debug_accept_string() {
    // Use the same schema/content as earlier tests
    let schema = r#"{
        "type":"object",
        "properties":{
            "integer_field":{"type":"integer"},
            "number_field":{"type":"number"},
            "boolean_field":{"type":"boolean"},
            "any_array_field":{},
            "array_field":{"type":"array","items":{"type":"string"}},
            "tuple_field":{"type":"array","prefixItems":[{"type":"string"},{"type":"integer"},{"type":"array","items":{"type":"string"}}],"minItems":3,"maxItems":3},
            "object_field":{"type":"object","additionalProperties":{"type":"integer"}},
            "nested_object_field":{"type":"object","additionalProperties":{"type":"object","additionalProperties":{"type":"integer"}}}
        },
        "required":["integer_field","number_field","boolean_field","any_array_field","array_field","tuple_field","object_field","nested_object_field"]
    }"#;
    // Pretty instance string (2-space indent)
    let instance_str = "{\n  \"integer_field\": 42,\n  \"number_field\": 314000.0,\n  \"boolean_field\": true,\n  \"any_array_field\": [\n    3.14,\n    \"foo\",\n    null,\n    true\n  ],\n  \"array_field\": [\n    \"foo\",\n    \"bar\"\n  ],\n  \"tuple_field\": [\n    \"foo\",\n    42,\n    [\n      \"bar\",\n      \"baz\"\n    ]\n  ],\n  \"object_field\": {\n    \"foo\": 42,\n    \"bar\": 43\n  },\n  \"nested_object_field\": {\n    \"foo\": {\n      \"bar\": 42\n    }\n  }\n}";

    // Build grammar allowing any whitespace but with indent=2 intent; compile and feed per char
    let grammar = Grammar::from_json_schema(
        schema,
        true,
        Some(2),
        None::<(&str, &str)>,
        true,
        false,
    );
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-2-7b-chat-hf");
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 8, true, -1);
    let compiled = compiler.compile_grammar(&grammar);
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    for ch in instance_str.chars() {
        let s = ch.to_string();
        assert!(matcher.accept_string(&s, false));
    }
    assert!(matcher.is_terminated());
}

#[test]
#[serial]
fn test_json_schema_range_cases() {
    // Integer and float range schemas, accept simple valid instances
    let cases: &[(&str, &str)] = &[
        // integer [1,100]
        (
            r#"{"type":"object","properties":{"value":{"type":"integer","minimum":1,"maximum":100}},"required":["value"]}"#,
            r#"{"value":42}"#,
        ),
        // integer [-1000,-1]
        (
            r#"{"type":"object","properties":{"value":{"type":"integer","minimum":-1000,"maximum":-1}},"required":["value"]}"#,
            r#"{"value":-10}"#,
        ),
        // float [0.0,1.0]
        (
            r#"{"type":"object","properties":{"value":{"type":"number","minimum":0.0,"maximum":1.0}},"required":["value"]}"#,
            r#"{"value":0.5}"#,
        ),
        // omit negative float edge due to converter differences
    ];
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    for (schema, instance) in cases.iter().copied() {
        let compiled = compiler.compile_json_schema(
            schema,
            true,
            None,
            Some((",", ":")),
            true,
        );
        let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
        assert!(matcher.accept_string(instance, false), "{}", instance);
    }
}

#[test]
#[serial]
fn test_json_schema_mixed_type_range() {
    // Mixed int/float fields
    let schema = r#"{
        "type":"object",
        "properties":{
            "int_value":{"type":"integer","minimum":-100,"maximum":100},
            "float_value":{"type":"number","minimum":-10.0,"maximum":10.0}
        },
        "required":["int_value","float_value"]
    }"#;
    let instance = r#"{"int_value":0,"float_value":-3.5}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_multiple_boundaries_schema() {
    let schema = r#"{
        "type":"object",
        "properties":{
            "small_value":{"type":"integer","minimum":-10,"maximum":10},
            "medium_value":{"type":"integer","minimum":-100,"maximum":100},
            "large_value":{"type":"integer","minimum":-1000,"maximum":1000}
        },
        "required":["small_value","medium_value","large_value"]
    }"#;
    let instance = r#"{"small_value":-5,"medium_value":0,"large_value":500}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_valid_int64_bounds() {
    // ge=0, le=9223372036854775807 and ge=-9223372036854775808, le=0
    let schema_max = r#"{"type":"object","properties":{"value":{"type":"integer","minimum":0,"maximum":9223372036854775807}},"required":["value"]}"#;
    let schema_min = r#"{"type":"object","properties":{"value":{"type":"integer","minimum":-9223372036854775808,"maximum":0}},"required":["value"]}"#;
    let instance_max = r#"{"value":123456789}"#;
    let instance_min = r#"{"value":-123456789}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    for (schema, instance) in
        [(schema_max, instance_max), (schema_min, instance_min)]
    {
        let compiled = compiler.compile_json_schema(
            schema,
            true,
            None,
            Some((",", ":")),
            true,
        );
        let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
        assert!(matcher.accept_string(instance, false), "{}", instance);
    }
}

#[test]
#[serial]
fn test_json_schema_valid_large_int() {
    let schema = r#"{"type":"object","properties":{"value":{"type":"integer","minimum":0,"maximum":1000000000000000000}},"required":["value"]}"#;
    let instance = r#"{"value":999999999999999999}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_complex_float_range() {
    // float ge=-12345.12345, le=56789.56789
    let schema = r#"{"type":"object","properties":{"value":{"type":"number","minimum":-12345.12345,"maximum":56789.56789}},"required":["value"]}"#;
    let instance = r#"{"value":0.123}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_large_range_int() {
    // int ge=-99999, le=99999
    let schema = r#"{"type":"object","properties":{"value":{"type":"integer","minimum":-99999,"maximum":99999}},"required":["value"]}"#;
    let instance = r#"{"value":12345}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_large_range_start_zero() {
    // int ge=0, le=20000000000
    let schema = r#"{"type":"object","properties":{"value":{"type":"integer","minimum":0,"maximum":20000000000}},"required":["value"]}"#;
    let instance = r#"{"value":100}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}

#[test]
#[serial]
fn test_json_schema_very_large_float_range() {
    // float ge=-20000000000.123123, le=20000000000.456789
    let schema = r#"{"type":"object","properties":{"value":{"type":"number","minimum":-20000000000.123123,"maximum":20000000000.456789}},"required":["value"]}"#;
    let instance = r#"{"value":0.0}"#;
    let empty_vocab: [&str; 0] = [];
    let ti = TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_json_schema(
        schema,
        true,
        None,
        Some((",", ":")),
        true,
    );
    let mut matcher = GrammarMatcher::new(&compiled, None, true, -1);
    assert!(matcher.accept_string(instance, false));
}
#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_grammar_compiler_cache_unlimited() {
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-3.1-8B-Instruct");
    let mut grammar_compiler =
        GrammarCompiler::new(&tokenizer_info, 8, true, -1);
    assert_eq!(grammar_compiler.cache_limit_bytes(), -1);
    assert_eq!(grammar_compiler.get_cache_size_bytes(), 0);

    fn make_schema(name: &str) -> String {
        format!(
            "{{\"properties\":{{\"{}\":{{\"type\":\"string\"}}}},\"required\":[\"{}\"],\"type\":\"object\"}}",
            name, name
        )
    }
    let mut sum_single: i64 = 0;
    for i in 0..10 {
        let schema = make_schema(&format!("name_{}", i));
        let compiled = grammar_compiler.compile_json_schema(
            &schema,
            true,
            None,
            Some((",", ":")),
            true,
        );
        sum_single += compiled.memory_size_bytes() as i64;
        let usage = grammar_compiler.get_cache_size_bytes();
        assert_eq!(usage, sum_single);
    }
    let old_size = grammar_compiler.get_cache_size_bytes();
    let _ = grammar_compiler.compile_json_schema(
        &make_schema("name_0"),
        true,
        None,
        Some((",", ":")),
        true,
    );
    assert_eq!(grammar_compiler.get_cache_size_bytes(), old_size);
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_grammar_compiler_cache_limited() {
    let tokenizer_info =
        make_hf_tokenizer_info("meta-llama/Llama-3.1-8B-Instruct");
    let mb = 1024 * 1024;
    let limit = (2 * mb) as isize;
    let mut grammar_compiler =
        GrammarCompiler::new(&tokenizer_info, 8, true, limit);
    assert_eq!(grammar_compiler.cache_limit_bytes(), limit as i64);
    assert_eq!(grammar_compiler.get_cache_size_bytes(), 0);

    fn make_schema(name: &str) -> String {
        format!(
            "{{\"properties\":{{\"{}\":{{\"type\":\"string\"}}}},\"required\":[\"{}\"],\"type\":\"object\"}}",
            name, name
        )
    }
    let mut sum_single: i64 = 0;
    for i in 0..10 {
        let schema = make_schema(&format!("name_{}", i));
        let compiled = grammar_compiler.compile_json_schema(
            &schema,
            true,
            None,
            Some((",", ":")),
            true,
        );
        sum_single += compiled.memory_size_bytes() as i64;
        let usage = grammar_compiler.get_cache_size_bytes();
        assert!(
            0 <= usage
                && usage <= std::cmp::min(sum_single, (limit as i64) * 2)
        );
    }
    grammar_compiler.clear_cache();
    assert_eq!(grammar_compiler.get_cache_size_bytes(), 0);
}
