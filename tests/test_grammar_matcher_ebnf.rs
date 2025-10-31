mod test_utils;

use serial_test::serial;
use test_utils::*;
use xgrammar::Grammar;
#[cfg(feature = "hf")]
use xgrammar::{
    GrammarCompiler, GrammarMatcher, allocate_token_bitmask, testing,
};

#[test]
#[serial]
fn test_simple() {
    let grammar_str = r#"root ::= rule1 rule2
rule1 ::= (rule2 | rule3) "a"
rule2 ::= "b"
rule3 ::= "c"
"#;

    let grammar = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&grammar, "bab"));
    assert!(!is_grammar_accept_string(&grammar, "abb"));
    assert!(is_grammar_accept_string(&grammar, "cab"));
}

#[test]
#[serial]
fn test_repetition() {
    let grammar_str = r#"
        root ::= rule {2, 3}
        rule ::= ("a" | [bc] {4,})
    "#;
    let grammar = Grammar::from_ebnf(grammar_str, "root");
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("d", false),
        ("aaaa", false),
    ];
    for (input, accepted) in cases {
        assert_eq!(
            is_grammar_accept_string(&grammar, input),
            accepted,
            "{}",
            input
        );
    }
}

#[test]
#[serial]
fn test_repetition_with_empty() {
    let grammar_str = r#"
        root ::= rule {2, 3} "d"?
        rule ::= ("a" | [bc] {4,}) | ""
    "#;
    let grammar = Grammar::from_ebnf(grammar_str, "root");
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("aaaa", false),
        ("", true),
        ("a", true),
        ("d", true),
    ];
    for (input, accepted) in cases {
        assert_eq!(
            is_grammar_accept_string(&grammar, input),
            accepted,
            "{}",
            input
        );
    }
}

#[test]
#[serial]
fn test_utf8() {
    // Test utf8-encoded string with EBNF grammar
    let ebnf_grammar_str = "root ::= [，]+"; // fullwidth comma U+FF0C
    let grammar = Grammar::from_ebnf(ebnf_grammar_str, "root");
    let accepted_inputs =
        ["，", "，，，", "，，，，，，，，，，，，，，，，，，，，，，"]; // a bunch of fullwidth commas
    for input in accepted_inputs {
        assert!(is_grammar_accept_string(&grammar, input), "{}", input);
    }
}

#[test]
#[serial]
fn test_custom_root_rule() {
    let json_grammar_simple_ebnf = r#"
root ::= basic_object
basic_any ::= basic_string | basic_object
basic_string ::= (([\"] basic_string_1 [\"]))
basic_string_1 ::= "" | [^"\\\r\n] basic_string_1 | "\\" escape basic_string_1
escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_object ::= "{" ("" | ws basic_string ws ":" ws basic_any ( ws "," ws basic_string ws ":" ws basic_any)*) ws "}"
ws ::= [ \n\t]*
"#;
    let grammar = Grammar::from_ebnf(json_grammar_simple_ebnf, "basic_string");
    assert!(is_grammar_accept_string(&grammar, r#""abc\r\n""#));
    assert!(!is_grammar_accept_string(&grammar, r#"{"name": "John" }"#));
}

fn json_grammar_ebnf() -> &'static str {
    r#"
root ::= basic_array | basic_object
basic_any ::= basic_number | basic_string | basic_boolean | basic_null | basic_array | basic_object
basic_integer ::= ("0" | "-"? [1-9] [0-9]*) ".0"?
basic_number ::= ("0" | "-"? [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
basic_string ::= (([\"] basic_string_1 [\"]))
basic_string_1 ::= "" | [^"\\\x00-\x1F] basic_string_1 | "\\" escape basic_string_1
escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_boolean ::= "true" | "false"
basic_null ::= "null"
basic_array ::= "[" ("" | ws basic_any (ws "," ws basic_any)*) ws "]"
basic_object ::= "{" ("" | ws basic_string ws ":" ws basic_any ( ws "," ws basic_string ws ":" ws basic_any)*) ws "}"
ws ::= [ \n\t]*
"#
}

#[test]
#[serial]
fn test_json_accept() {
    let grammar = Grammar::from_ebnf(json_grammar_ebnf(), "root");
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
        assert!(is_grammar_accept_string(&grammar, s), "{}", s);
    }
}

#[test]
#[serial]
fn test_json_refuse() {
    let grammar = Grammar::from_ebnf(json_grammar_ebnf(), "root");
    let refused = [
        r#"{ name: "John" }"#,
        r#"{ "name": "John" } "#, // trailing space is not accepted
        r#"{ "name": "John", "age": 30, }"#,
        r#"{ "name": "John", "address": { "street": "123 Main St", "city": "New York" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling",], }"#,
        r#"{ "name": "John", "age": 30.5.7 }"#,
        r#"{ "name": "John, "age": 30, "hobbies": ["reading", "traveling"] }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", { "type": "outdoor", "list": ["hiking", "swimming",]}] }"#,
        r#"{ "name": "John", "age": 30, "status": "\P\J" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling"], "address": { "street": "123 Main St", "city": "New York", "coordinates": { "latitude": 40.7128, "longitude": -74.0060 }}}, "work": { "company": "Acme", "position": "developer" }}"#,
    ];

    for s in refused {
        assert!(!is_grammar_accept_string(&grammar, s), "{}", s);
    }
}

#[test]
#[serial]
fn test_json_pressure() {
    let grammar = Grammar::from_ebnf(json_grammar_ebnf(), "root");

    // Extra long string: 1k chars
    let long_1k: &str = concat!(
        "[\"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer nec odio. Praesent ",
        "libero. Sed cursus ante dapibus diam. Sed nisi. Nulla quis sem at nibh elementum ",
        "imperdiet. Duis sagittis ipsum. Praesent mauris. Fusce nec tellus sed augue semper ",
        "porta. Mauris massa. Vestibulum lacinia arcu eget nulla. Class aptent taciti sociosqu ",
        "ad litora torquent per conubia nostra, per inceptos himenaeos. Curabitur sodales ligula ",
        "in libero. Sed dignissim lacinia nunc. Curabitur tortor. Pellentesque nibh. Aenean quam. ",
        "In scelerisque sem at dolor. Maecenas mattis. Sed convallis tristique sem. Proin ut ",
        "ligula vel nunc egestas porttitor. Morbi lectus risus, iaculis vel, suscipit quis, ",
        "luctus non, massa. Fusce ac turpis quis ligula lacinia aliquet. Mauris ipsum. Nulla ",
        "metus metus, ullamcorper vel, tincidunt sed, euismod in, nibh. Quisque volutpat ",
        "condimentum velit. Class aptent taciti sociosqu ad litora torquent per conubia nostra, ",
        "per inceptos himenaeos. Nam nec ante. Sed lacinia, urna non tincidunt mattis, tortor ",
        "neque adipiscing diam, a cursus ipsum ante quis turpis. Nulla facilisi. Ut fringilla. ",
        "Suspendisse potenti. Nunc feugiat mi a tellus consequat imperdiet. Vestibulum sapien. ",
        "Proin quam. Etiam ultrices. Suspendisse in justo eu magna luctus suscipit. Sed lectus. ",
        "Integer euismod lacus luctus magna. Quisque cursus, metus vitae pharetra auctor, sem ",
        "massa mattis sem, at interdum magna augue eget diam.\"]"
    );
    assert!(is_grammar_accept_string(&grammar, long_1k));

    // long and complex json: 3k chars
    let long_3k = r#"{
    "web-app": {
    "servlet": [
        {
        "servlet-name": "cofaxCDS",
        "servlet-class": "org.cofax.cds.CDSServlet",
        "init-param": {
            "configGlossary:installationAt": "Philadelphia, PA",
            "configGlossary:adminEmail": "ksm@pobox.com",
            "configGlossary:poweredBy": "Cofax",
            "configGlossary:poweredByIcon": "/images/cofax.gif",
            "configGlossary:staticPath": "/content/static",
            "templateProcessorClass": "org.cofax.WysiwygTemplate",
            "templateLoaderClass": "org.cofax.FilesTemplateLoader",
            "templatePath": "templates",
            "templateOverridePath": "",
            "defaultListTemplate": "listTemplate.htm",
            "defaultFileTemplate": "articleTemplate.htm",
            "useJSP": false,
            "jspListTemplate": "listTemplate.jsp",
            "jspFileTemplate": "articleTemplate.jsp",
            "cachePackageTagsTrack": 200,
            "cachePackageTagsStore": 200,
            "cachePackageTagsRefresh": 60,
            "cacheTemplatesTrack": 100,
            "cacheTemplatesStore": 50,
            "cacheTemplatesRefresh": 15,
            "cachePagesTrack": 200,
            "cachePagesStore": 100,
            "cachePagesRefresh": 10,
            "cachePagesDirtyRead": 10,
            "searchEngineListTemplate": "forSearchEnginesList.htm",
            "searchEngineFileTemplate": "forSearchEngines.htm",
            "searchEngineRobotsDb": "WEB-INF/robots.db",
            "useDataStore": true,
            "dataStoreClass": "org.cofax.SqlDataStore",
            "redirectionClass": "org.cofax.SqlRedirection",
            "dataStoreName": "cofax",
            "dataStoreDriver": "com.microsoft.jdbc.sqlserver.SQLServerDriver",
            "dataStoreUrl": "jdbc:microsoft:sqlserver://LOCALHOST:1433;DatabaseName=goon",
            "dataStoreUser": "sa",
            "dataStorePassword": "dataStoreTestQuery",
            "dataStoreTestQuery": "SET NOCOUNT ON;select test='test';",
            "dataStoreLogFile": "/usr/local/tomcat/logs/datastore.log",
            "dataStoreInitConns": 10,
            "dataStoreMaxConns": 100,
            "dataStoreConnUsageLimit": 100,
            "dataStoreLogLevel": "debug",
            "maxUrlLength": 500
        }
        },
        {
        "servlet-name": "cofaxEmail",
        "servlet-class": "org.cofax.cds.EmailServlet",
        "init-param": {
            "mailHost": "mail1",
            "mailHostOverride": "mail2"
        }
        },
        {
        "servlet-name": "cofaxAdmin",
        "servlet-class": "org.cofax.cds.AdminServlet"
        },
        {
        "servlet-name": "fileServlet",
        "servlet-class": "org.cofax.cds.FileServlet"
        },
        {
        "servlet-name": "cofaxTools",
        "servlet-class": "org.cofax.cms.CofaxToolsServlet",
        "init-param": {
            "templatePath": "toolstemplates/",
            "log": 1,
            "logLocation": "/usr/local/tomcat/logs/CofaxTools.log",
            "logMaxSize": "",
            "dataLog": 1,
            "dataLogLocation": "/usr/local/tomcat/logs/dataLog.log",
            "dataLogMaxSize": "",
            "removePageCache": "/content/admin/remove?cache=pages&id=",
            "removeTemplateCache": "/content/admin/remove?cache=templates&id=",
            "fileTransferFolder": "/usr/local/tomcat/webapps/content/fileTransferFolder",
            "lookInContext": 1,
            "adminGroupID": 4,
            "betaServer": true
        }
        }
    ],
    "servlet-mapping": {
        "cofaxCDS": "/",
        "cofaxEmail": "/cofaxutil/aemail/*",
        "cofaxAdmin": "/admin/*",
        "fileServlet": "/static/*",
        "cofaxTools": "/tools/*"
    },
    "taglib": {
        "taglib-uri": "cofax.tld",
        "taglib-location": "/WEB-INF/tlds/cofax.tld"
    }
    }
}"#;
    assert!(is_grammar_accept_string(&grammar, long_3k));
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_fill_next_token_bitmask() {
    let test_cases: &[(&str, &str, &[usize])] = &[
        (
            "meta-llama/Llama-2-7b-chat-hf",
            "{\"id\": 1,\"name\": \"Example\"}",
            &[
                31989, 31912, 270, 270, 270, 31973, 31846, 31846, 31948, 31915,
                270, 270, 270, 270, 270, 31973, 31846, 31846, 263, 263, 263,
                263, 263, 263, 263, 263, 31974, 31999,
            ],
        ),
        (
            "meta-llama/Llama-2-7b-chat-hf",
            "{\n\"id\": 1,\n\"na\": \"ex\",\n\"ac\": true,\n\"t\": [\"t1\", \"t2\"],\n\"ne\": {\"lv2\": {\"val\": \"dp\"}, \"arr\": [1, 2, 3]},\n\"res\": \"res\"\n}",
            &[
                31989, 31912, 31912, 270, 270, 270, 31973, 31846, 31846, 31948,
                31915, 31915, 270, 270, 270, 31973, 31846, 31846, 263, 263,
                263, 31974, 31915, 31915, 270, 270, 270, 31973, 31846, 31846,
                31997, 31997, 31998, 31974, 31915, 31915, 270, 270, 31973,
                31846, 31846, 31840, 262, 262, 262, 31969, 31846, 31846, 262,
                262, 262, 31969, 31974, 31915, 31915, 270, 270, 270, 31973,
                31846, 31846, 31908, 270, 270, 270, 270, 31973, 31846, 31846,
                31906, 270, 270, 270, 270, 31973, 31846, 31846, 262, 262, 262,
                31968, 31970, 31915, 31915, 270, 270, 270, 270, 31973, 31846,
                31846, 31840, 31943, 31846, 31846, 31943, 31846, 31846, 31943,
                31970, 31974, 31915, 31915, 270, 270, 270, 270, 31973, 31846,
                31846, 263, 263, 263, 263, 31974, 31974, 31999,
            ],
        ),
        // Note: Skipping meta-llama/Meta-Llama-3-8B-Instruct test case as it requires
        // additional authentication beyond HF_TOKEN
    ];

    for (tokenizer_path, input_str, expected_rejected_sizes) in test_cases {
        let tokenizer_info = make_hf_tokenizer_info(tokenizer_path);
        let mut grammar_compiler =
            GrammarCompiler::new(&tokenizer_info, 8, false, -1);
        let grammar = Grammar::from_ebnf(json_grammar_ebnf(), "root");
        let compiled_grammar = grammar_compiler.compile_grammar(&grammar);
        let mut matcher =
            GrammarMatcher::new(&compiled_grammar, None, false, -1);

        let vocab_size = tokenizer_info.vocab_size();
        let mut bitmask_data = allocate_token_bitmask(1, vocab_size);

        let input_bytes = input_str.as_bytes();

        for (i, &c) in input_bytes.iter().enumerate() {
            let (mut tensor, _shape, _strides) =
                create_bitmask_dltensor(&mut bitmask_data, 1, vocab_size);

            matcher.fill_next_token_bitmask(&mut tensor, 0, false);

            let rejected_token_ids = testing::get_masked_tokens_from_bitmask(
                &tensor,
                vocab_size as i32,
                0,
            );
            assert_eq!(
                rejected_token_ids.len(),
                expected_rejected_sizes[i],
                "Mismatch at byte index {} (char: {})",
                i,
                c as char
            );

            let byte_array = [c];
            let byte_str = std::str::from_utf8(&byte_array).unwrap_or("");
            assert!(matcher.accept_string(byte_str, false));

            // Reset bitmask for next iteration
            bitmask_data.fill(-1);
        }

        // Final correctness verification
        let (mut tensor, _shape, _strides) =
            create_bitmask_dltensor(&mut bitmask_data, 1, vocab_size);
        matcher.fill_next_token_bitmask(&mut tensor, 0, false);
        let rejected_token_ids = testing::get_masked_tokens_from_bitmask(
            &tensor,
            vocab_size as i32,
            0,
        );
        assert_eq!(
            rejected_token_ids.len(),
            expected_rejected_sizes[expected_rejected_sizes.len() - 1]
        );
    }
}

#[test]
#[serial]
fn test_nullable_grammar() {
    let grammar_str = r#"
    root ::= rule1 | (rule1 rule1 rule1 rule3)+
    rule1 ::= rule2
    rule2 ::= [0-9]*
    rule3 ::= [a-z]
"#;
    let grammar = Grammar::from_ebnf(grammar_str, "root");
    let test_strings = ["abc12312398014a", ""];
    for s in test_strings {
        assert!(is_grammar_accept_string(&grammar, s), "{}", s);
    }
}

#[test]
#[serial]
fn test_predict_complete() {
    // Test complex prediction and completion with EBNF grammar.
    let mixed_grammar_str = r#"root ::= rule1 [0-9]?
    rule1 ::= rule2 [0-9]? | rule4 [0-9]?
    rule2 ::= rule3 [0-9]? | rule2 [0-9]? | rule1 [0-9]?
    rule3 ::= rule4 [0-9]? | rule5 [0-9]?
    rule4 ::= rule5 [0-9]? | rule6 [0-9]?
    rule5 ::= rule6 [0-9]? | rule7 [0-9]? | rule8 [0-9]?
    rule6 ::= rule7 [0-9]? | rule1 [0-9]?
    rule7 ::= rule8 [0-9]? | rule9 [0-9]?
    rule8 ::= rule9 [0-9]? | rule7 [0-9]?
    rule9 ::= [0-9]?
    "#;

    let grammar = Grammar::from_ebnf(mixed_grammar_str, "root");
    let mut input = String::new();
    for _ in 0..10 {
        assert!(is_grammar_accept_string(&grammar, &input), "{}", input);
        input.push('0');
    }
    assert!(is_grammar_accept_string(&grammar, &input));

    // Test right recursion
    let right_recursion_grammar =
        Grammar::from_ebnf("root ::= [a-z] root | [a-z]", "root");

    let accept_strings = ["a", "ab", "abc", "abcd", "abcde"];
    let reject_strings = ["", "1", "a1", "ab1", "abc1"];
    for s in accept_strings {
        assert!(is_grammar_accept_string(&right_recursion_grammar, s), "{}", s);
    }
    for s in reject_strings {
        assert!(
            !is_grammar_accept_string(&right_recursion_grammar, s),
            "{}",
            s
        );
    }

    // Test the mixture of right recursion and other rules
    let mixed_grammar_str = r#"root ::= rule1
    rule1 ::= "{" rule2 | ""
    rule2 ::= root "}"
    "#;
    let grammar2 = Grammar::from_ebnf(mixed_grammar_str, "root");
    let test_strings = ["", "{}", "{{}}", "{{{}}}", "{{{{}}}}", "{{{{{}}}}}"];
    let rejected_strings = ["{", "{}{}", "{{{{}", "{{}}}", "{{{{{}}}}}}"];

    for s in test_strings {
        assert!(is_grammar_accept_string(&grammar2, s), "{}", s);
    }
    for s in rejected_strings {
        assert!(!is_grammar_accept_string(&grammar2, s), "{}", s);
    }
}

#[test]
#[serial]
fn test_advance() {
    // Test complex Advance and completion with EBNF grammar.
    let ebnf_grammar_str = r#"root ::= rule1
    rule1 ::= [a] | [a-b] | [a-c]* | "a" | "aaaaaaaaaaaaaaaaaaa"
    "#;
    let grammar = Grammar::from_ebnf(ebnf_grammar_str, "root");
    for i in 0..10 {
        let input = "a".repeat(i);
        assert!(is_grammar_accept_string(&grammar, &input), "{}", input);
    }
}

#[test]
#[serial]
fn test_character_class_star_utf8() {
    let ebnf_grammar_str = r#"root ::= [^0-9]*"#;
    let test_string = "worldせかい世界";
    let grammar = Grammar::from_ebnf(ebnf_grammar_str, "root");
    assert!(is_grammar_accept_string(&grammar, test_string));
}

#[test]
#[serial]
#[cfg(feature = "hf")]
fn test_not_neighbour_character_class() {
    let raw_grammar = "root ::= [a-cx-z]*";
    let tokenizer_path = "meta-llama/Llama-2-7b-chat-hf";
    let tokenizer_info = make_hf_tokenizer_info(tokenizer_path);
    let grammar = Grammar::from_ebnf(raw_grammar, "root");
    let mut grammar_compiler =
        GrammarCompiler::new(&tokenizer_info, 8, false, -1);
    let compiled_grammar = grammar_compiler.compile_grammar(&grammar);
    let mut matcher = GrammarMatcher::new(&compiled_grammar, None, false, -1);

    let vocab_size = tokenizer_info.vocab_size();
    let mut bitmask_data = allocate_token_bitmask(1, vocab_size);
    let (mut tensor, _shape, _strides) =
        create_bitmask_dltensor(&mut bitmask_data, 1, vocab_size);

    matcher.fill_next_token_bitmask(&mut tensor, 0, false);
    let rejected_token_ids =
        testing::get_masked_tokens_from_bitmask(&tensor, vocab_size as i32, 0);
    assert_eq!(rejected_token_ids.len(), 31933);
}

#[test]
#[serial]
fn test_nfa() {
    let grammar_str = r#"
root ::= rule1 | rule2 | rule3
rule1 ::= "abc" | ""
rule2 ::= "abd" | ""
rule3 ::= [a-n] [b-c] "x" | ""
"#;
    let grammar = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&grammar, "abc"));
    assert!(is_grammar_accept_string(&grammar, "abx"));
    assert!(is_grammar_accept_string(&grammar, "ccx"));
    assert!(!is_grammar_accept_string(&grammar, "abb"));
    assert!(!is_grammar_accept_string(&grammar, "ad"));
}
