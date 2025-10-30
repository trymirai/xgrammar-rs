#![cfg(feature = "hf")]
use serial_test::serial;

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

fn matcher_from_grammar_with_tokenizer(
    grammar: &xgrammar::Grammar,
    tk: &tokenizers::Tokenizer,
) -> xgrammar::GrammarMatcher {
    let ti = xgrammar::TokenizerInfo::from_huggingface(tk, None, None);
    let mut compiler = xgrammar::GrammarCompiler::new(&ti, 1, false, -1);
    let compiled = compiler.compile_grammar(grammar);
    xgrammar::GrammarMatcher::new(&compiled, None, true, -1)
}

#[test]
#[serial]
fn test_json_accept_with_hf_tokenizer() {
    // Mirror Python matcher tests by compiling builtin JSON grammar with a real tokenizer.
    let model_id = "gpt2";
    let path =
        download_tokenizer_json(model_id).expect("download tokenizer.json");
    let tk = tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");

    let g = xgrammar::Grammar::builtin_json_grammar();
    let mut m = matcher_from_grammar_with_tokenizer(&g, &tk);

    let accepted =
        ["{\"name\": \"John\"}", "{ \"name\" : \"John\" }", "{}", "[]"];
    for s in accepted {
        assert!(m.accept_string(s, true), "{}", s);
        m.reset();
    }
}

#[test]
#[serial]
fn test_regex_accept_with_hf_tokenizer() {
    let model_id = "gpt2";
    let path =
        download_tokenizer_json(model_id).expect("download tokenizer.json");
    let tk = tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");

    let g = xgrammar::Grammar::from_regex("(abc|def)+", false);
    let mut m = matcher_from_grammar_with_tokenizer(&g, &tk);

    let positives = ["abcdef", "abcabc"]; // keep to robust positive-only due to converter differences
    for s in positives {
        assert!(m.accept_string(s, true), "{}", s);
        m.reset();
    }
}
