use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use xgrammar::{TokenizerInfo, VocabType};

#[test]
fn compile_builtin_json_with_minimal_vocab() {
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    struct Person {
        name: String,
        age: i32,
    }

    // Minimal synthetic vocab sufficient for basic JSON tokens
    let vocab = vec![
        "{",
        "}",
        "[",
        "]",
        ":",
        ",",
        "\"",
        " ",
        "x",
        "g",
        "r",
        "a",
        "m",
        "e",
        "n",
        "0",
        "1",
        "2",
        "<|end_of_text|>",
    ];

    // Mark the last token as stop token
    let stop_id = (vocab.len() - 1) as i32;

    let tok_info = TokenizerInfo::new_from_strings(
        &vocab,
        VocabType::RAW,
        Some(vocab.len()),
        Some(vec![stop_id]),
        false,
    );

    // TODO: Add GrammarCompiler/Matcher once port is complete.
    assert_eq!(tok_info.vocab_size(), autocxx::c_int(vocab.len() as i32));

    // Generate JSON Schema from Person and print it
    let schema = schema_for!(Person);
    let schema_json = to_string_pretty(&schema).unwrap();
    println!("{}", schema_json);
}
