//! Port of `xgrammar/tests/python/test_grammar_matcher_basic.py`.
//!
//! The pure (non-HuggingFace) slice: string acceptance, token operations, rollback, reset,
//! termination, jump-forward, fork. The HF `fill_next_token_bitmask` cases and the batch
//! pressure tests land with the HF tokenizer.

use std::collections::BTreeSet;

use xgrammar::{
    grammar::Grammar,
    matcher::{
        BatchGrammarMatcher, GrammarMatcher, allocate_token_bitmask,
        get_masked_tokens_from_bitmask,
    },
    tokenizer::{TokenizerInfo, VocabType},
};

/// The small JSON vocabulary used by most cases (`"</s>"` at index 1 is the stop token).
const JSON_VOCAB: &[&str] = &[
    "<s>",
    "</s>",
    "a",
    "abc",
    "b\"",
    "\"",
    ":\"",
    "{",
    "}",
    ", ",
    "6",
    ":",
    "\n",
    " ",
    "\"a\":true",
];

fn string_matcher(grammar: &str) -> GrammarMatcher {
    let grammar = Grammar::from_ebnf(grammar, "root").unwrap();
    GrammarMatcher::from_grammar(&grammar, true)
}

fn json_matcher(vocab: &[&str]) -> GrammarMatcher {
    let v: Vec<String> = vocab.iter().map(|s| (*s).to_owned()).collect();
    let info = TokenizerInfo::new(&v, VocabType::Raw, None, None, false);
    GrammarMatcher::from_grammar_and_tokenizer(
        &Grammar::builtin_json_grammar(),
        info,
    )
}

fn idx(
    vocab: &[&str],
    token: &str,
) -> i32 {
    vocab.iter().position(|t| *t == token).expect("token in vocab") as i32
}

/// The rejected (masked) token ids in the current state.
fn rejected(
    m: &mut GrammarMatcher,
    vocab_size: i32,
) -> BTreeSet<i32> {
    let mut bm = allocate_token_bitmask(1, vocab_size);
    m.fill_next_token_bitmask(&mut bm, 0);
    get_masked_tokens_from_bitmask(&bm, vocab_size, 0).into_iter().collect()
}

/// The accepted token *names* in the current state.
fn accepted_names<'a>(
    m: &mut GrammarMatcher,
    vocab: &[&'a str],
) -> BTreeSet<&'a str> {
    let rej = rejected(m, vocab.len() as i32);
    (0..vocab.len() as i32)
        .filter(|t| !rej.contains(t))
        .map(|t| vocab[t as usize])
        .collect()
}

#[test]
fn test_accept_string() {
    let cases: &[(&[u8], bool)] = &[
        (b"bbb", true),
        (b"bba", false),
        ("©".as_bytes(), true),
        (b"\xe2\xa1\xa1", true),
        (b"\xe2\xa1\xa1\xa1", false),
        (b"\xe2\xa1\xe2\xa1", false),
    ];
    for &(input, accepted) in cases {
        let mut m = string_matcher("root ::= [^a]+");
        assert_eq!(m.accept_bytes(input), accepted, "input {input:?}");
    }
}

fn json_accepts(input: &str) -> bool {
    let grammar = Grammar::builtin_json_grammar();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_grammar_accept() {
    for input in [r#"{"name": "John"}"#, r#"{ "name" : "John" }"#] {
        assert!(json_accepts(input), "should accept {input:?}");
    }
}

#[test]
fn test_grammar_refuse() {
    for input in [r#"{ name: "John" }"#, r#"{ "name": "John" } "#] {
        assert!(!json_accepts(input), "should refuse {input:?}");
    }
}

#[test]
fn test_token_operations() {
    let input_tokens =
        ["{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\":true", "}"];
    let input_ids: Vec<i32> =
        input_tokens.iter().map(|t| idx(JSON_VOCAB, t)).collect();
    let expected: &[&[&str]] = &[
        &["{"],
        &["\"", "}", "\n", " ", "\"a\":true"],
        &["<s>", "a", "abc", "b\"", "\"", ":\"", "{", "}", ", ", "6", ":", " "],
        &["<s>", "a", "abc", "b\"", "\"", ":\"", "{", "}", ", ", "6", ":", " "],
        &[":", "\n", " ", ":\""],
        &["\"", "{", "6", "\n", " "],
        &["}", ", ", "6", "\n", " "],
        &[" ", "\n", "\"", "\"a\":true"],
        &[" ", "\n", "\"", "\"a\":true"],
        &["}", ", ", "\n", " "],
        &["</s>"],
    ];
    let mut m = json_matcher(JSON_VOCAB);
    let mut result: Vec<BTreeSet<&str>> = Vec::new();
    for &id in &input_ids {
        result.push(accepted_names(&mut m, JSON_VOCAB));
        assert!(m.accept_token(id));
    }
    result.push(accepted_names(&mut m, JSON_VOCAB));
    let expected: Vec<BTreeSet<&str>> =
        expected.iter().map(|step| step.iter().copied().collect()).collect();
    assert_eq!(result, expected);
}

#[test]
fn test_rollback() {
    let input_tokens =
        ["{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\":true", "}"];
    let input_ids: Vec<i32> =
        input_tokens.iter().map(|t| idx(JSON_VOCAB, t)).collect();
    let n = JSON_VOCAB.len() as i32;
    let mut m = json_matcher(JSON_VOCAB);
    assert_eq!(m.max_rollback_tokens(), -1);

    for pair in input_ids.chunks(2) {
        let (i1, i2) = (pair[0], pair[1]);
        let orig1 = rejected(&mut m, n);
        assert!(m.accept_token(i1));
        let orig2 = rejected(&mut m, n);
        assert!(m.accept_token(i2));

        m.rollback(2);
        let after1 = rejected(&mut m, n);
        assert!(m.accept_token(i1));
        let after2 = rejected(&mut m, n);
        assert!(m.accept_token(i2));
        assert_eq!(orig1, after1);
        assert_eq!(orig2, after2);
    }
}

#[test]
fn test_graceful_rollback_failure() {
    let vocab: &[&str] = &[
        "<s>",
        "</s>",
        "a",
        "abc",
        "b\"",
        "\"",
        ":\"",
        "{",
        "}",
        ", ",
        "6",
        "6:",
        ":",
        "\n",
        " ",
        "\"a\":true",
    ];
    let mut m = json_matcher(vocab);
    for t in ["{", "\"", "abc", "\"", ":"] {
        assert!(m.accept_token(idx(vocab, t)));
    }
    // "6:" matches '6' then fails on ':'; the partial advance must be gracefully reverted.
    assert!(!m.accept_token(idx(vocab, "6:")));
    for t in ["\"", "abc", "\"", " ", "}"] {
        assert!(m.accept_token(idx(vocab, t)));
    }
}

#[test]
fn test_reset() {
    let input_tokens =
        ["{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\":true", "}"];
    let input_ids: Vec<i32> =
        input_tokens.iter().map(|t| idx(JSON_VOCAB, t)).collect();
    let n = JSON_VOCAB.len() as i32;
    let mut m = json_matcher(JSON_VOCAB);

    let mut orig = Vec::new();
    for &i in &input_ids {
        orig.push(rejected(&mut m, n));
        assert!(m.accept_token(i));
    }
    m.reset();
    let mut after = Vec::new();
    for &i in &input_ids {
        after.push(rejected(&mut m, n));
        assert!(m.accept_token(i));
    }
    assert_eq!(orig, after);
}

#[test]
fn test_termination() {
    let vocab: &[&str] = &[
        "<s>", "</s>", "a", "abc", "b\"", "\"", ":\"", "{", " }", ", ", "6",
        ":", "\n", " ", "\"a\"", ":true",
    ];
    let input_tokens = [
        "{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\"", ":true", " }",
        "</s>",
    ];
    let input_ids: Vec<i32> =
        input_tokens.iter().map(|t| idx(vocab, t)).collect();
    let mut m = json_matcher(vocab);
    for &i in &input_ids {
        assert!(m.accept_token(i));
    }
    assert!(m.is_terminated());
    assert!(!m.accept_token(0));

    m.rollback(2);
    assert!(!m.is_terminated());
    assert!(m.accept_token(input_ids[input_ids.len() - 2]));
}

#[test]
fn test_is_completed() {
    let vocab: &[&str] = &[
        "<s>", "</s>", "a", "abc", "b\"", "\"", ":\"", "{", " }", ", ", "6",
        ":", "\n", " ", "\"a\"", ":true",
    ];
    let without_stop =
        ["{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\"", ":true", " }"];
    let ids: Vec<i32> = without_stop.iter().map(|t| idx(vocab, t)).collect();
    let stop = idx(vocab, "</s>");

    let mut m = json_matcher(vocab);
    assert!(!m.is_completed());
    assert!(!m.is_terminated());
    for &i in &ids {
        assert!(m.accept_token(i));
    }
    assert!(m.is_completed());
    assert!(!m.is_terminated());
    assert!(m.accept_token(stop));
    assert!(m.is_completed());
    assert!(m.is_terminated());
    m.rollback(1);
    assert!(m.is_completed());
    assert!(!m.is_terminated());
    m.rollback(2);
    assert!(!m.is_completed());
    assert!(!m.is_terminated());
}

#[test]
fn test_get_jump_forward_string() {
    let grammar = Grammar::from_ebnf(
        "root ::= \"abb\" | \"abbd\" | other_rule\n\
         other_rule ::= \"a\" sub_rule \"b\"\n\
         sub_rule ::= \"b\"\n",
        "root",
    )
    .unwrap();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    assert!(m.accept_string("a"));
    assert_eq!(m.find_jump_forward_string(), b"bb");
}

#[test]
fn test_debug_print_internal_state() {
    let mut m = json_matcher(JSON_VOCAB);
    for c in "{\"name\": \"John\"}".chars() {
        assert!(m.accept_string(&c.to_string()));
        assert!(!m.debug_print_internal_state().is_empty());
    }
}

#[test]
fn test_vocab_size() {
    let v: Vec<String> = JSON_VOCAB.iter().map(|s| (*s).to_owned()).collect();
    let info = TokenizerInfo::new(&v, VocabType::Raw, Some(64), None, false);
    let m = GrammarMatcher::from_grammar_and_tokenizer(
        &Grammar::builtin_json_grammar(),
        info,
    );
    assert_eq!(m.tokenizer_info().vocab_size(), 64);
}

#[test]
fn test_fork_initial_state() {
    let vocab = ["<s>", "</s>", "a", "b"];
    let grammar = Grammar::from_ebnf("root ::= \"a\" \"b\"", "root").unwrap();
    let v: Vec<String> = vocab.iter().map(|s| (*s).to_owned()).collect();
    let info = TokenizerInfo::new(&v, VocabType::Raw, None, None, false);
    let mut original =
        GrammarMatcher::from_grammar_and_tokenizer(&grammar, info);
    let mut forked = original.fork();
    assert_eq!(rejected(&mut original, 4), rejected(&mut forked, 4));
    assert!(!original.is_terminated() && !forked.is_terminated());
    assert_eq!(original.stop_token_ids(), forked.stop_token_ids());
}

#[test]
#[allow(clippy::type_complexity)]
fn test_batch_accept_string() {
    let cases: &[(&[&str], &[&[u8]], &[bool])] = &[
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[b"a", b"123", b"ab"],
            &[true, true, true],
        ),
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[b"b", b"123a", b"d"],
            &[false, false, false],
        ),
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[b"a", b"123a", b"ab"],
            &[true, false, true],
        ),
        (&["root ::= \"a\""], &[b"a"], &[true]),
        (&["root ::= \"a\""], &[b"b"], &[false]),
        (
            &[
                "root ::= \"你好\"",
                "root ::= \"こんにちは\"",
                "root ::= \"안녕하세요\"",
            ],
            &[
                "你好".as_bytes(),
                "こんにちは".as_bytes(),
                "안녕하세요".as_bytes(),
            ],
            &[true, true, true],
        ),
    ];
    for (grammars, inputs, expecteds) in cases {
        let mut matchers: Vec<GrammarMatcher> =
            grammars.iter().map(|g| string_matcher(g)).collect();
        let results =
            BatchGrammarMatcher::batch_accept_string(&mut matchers, inputs);
        assert_eq!(results, *expecteds, "grammars {grammars:?}");
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn test_batch_accept_token() {
    let vocab = ["<s>", "</s>", "a", "b", "c", "1", "2", "3", "123a", "ab"];
    let cases: &[(&[&str], &[i32], &[bool])] = &[
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[2, 5, 2],
            &[true, true, true],
        ),
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[3, 2, 4],
            &[false, false, false],
        ),
        (
            &["root ::= \"a\"", "root ::= [0-9]+", "root ::= \"ab\""],
            &[2, 8, 9],
            &[true, false, true],
        ),
        (&["root ::= \"a\""], &[2], &[true]),
        (&["root ::= \"a\""], &[3], &[false]),
    ];
    for (grammars, inputs, expecteds) in cases {
        let v: Vec<String> = vocab.iter().map(|s| (*s).to_owned()).collect();
        let mut matchers: Vec<GrammarMatcher> = grammars
            .iter()
            .map(|g| {
                let grammar = Grammar::from_ebnf(g, "root").unwrap();
                let info =
                    TokenizerInfo::new(&v, VocabType::Raw, None, None, false);
                GrammarMatcher::from_grammar_and_tokenizer(&grammar, info)
            })
            .collect();
        let results =
            BatchGrammarMatcher::batch_accept_token(&mut matchers, inputs);
        assert_eq!(results, *expecteds, "grammars {grammars:?}");
    }
}

#[test]
fn test_batch_rollback() {
    let input_tokens =
        ["{", "\"", "abc", "b\"", ":", "6", ", ", " ", "\"a\":true", "}"];
    let input_ids: Vec<i32> =
        input_tokens.iter().map(|t| idx(JSON_VOCAB, t)).collect();
    let n = JSON_VOCAB.len() as i32;
    let mut matchers: Vec<GrammarMatcher> =
        (0..3).map(|_| json_matcher(JSON_VOCAB)).collect();
    let rollback_lengths = [0, 1, 2];

    for pair in input_ids.chunks(2) {
        let (first, second) = (pair[0], pair[1]);
        // Per matcher: rejected sets before-first, before-second, after-second.
        let mut orig: Vec<[BTreeSet<i32>; 3]> = Vec::new();
        for m in &mut matchers {
            let b0 = rejected(m, n);
            assert!(m.accept_token(first));
            let b1 = rejected(m, n);
            assert!(m.accept_token(second));
            let b2 = rejected(m, n);
            orig.push([b0, b1, b2]);
        }

        BatchGrammarMatcher::batch_rollback(&mut matchers, &rollback_lengths);

        for (mi, m) in matchers.iter_mut().enumerate() {
            match rollback_lengths[mi] {
                0 => assert_eq!(rejected(m, n), orig[mi][2]),
                1 => {
                    assert_eq!(rejected(m, n), orig[mi][1]);
                    assert!(m.accept_token(second));
                    assert_eq!(rejected(m, n), orig[mi][2]);
                },
                _ => {
                    assert_eq!(rejected(m, n), orig[mi][0]);
                    assert!(m.accept_token(first));
                    assert_eq!(rejected(m, n), orig[mi][1]);
                    assert!(m.accept_token(second));
                    assert_eq!(rejected(m, n), orig[mi][2]);
                },
            }
        }
    }
}

#[test]
fn test_fork_after_accept_tokens() {
    let vocab: &[&str] =
        &["<s>", "</s>", "a", "abc", "b\"", "\"", "{", "}", " ", ":"];
    let input = ["{", "\"", "abc", "b\""];
    let mut original = json_matcher(vocab);
    for t in input {
        assert!(original.accept_token(idx(vocab, t)));
    }
    let mut forked = original.fork();
    assert_eq!(
        rejected(&mut original, vocab.len() as i32),
        rejected(&mut forked, vocab.len() as i32)
    );
    let next = idx(vocab, ":");
    assert!(original.accept_token(next));
    assert!(forked.accept_token(next));
    original.rollback(1);
    forked.rollback(1);
    assert_eq!(
        rejected(&mut original, vocab.len() as i32),
        rejected(&mut forked, vocab.len() as i32)
    );
}
