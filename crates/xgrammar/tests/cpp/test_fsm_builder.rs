//! Port of `external/xgrammar/tests/cpp/test_fsm_builder.cc`.
//!
//! Covers the trie builder, the regex builder, and the per-expression `GrammarFsmBuilder`
//! (tag-dispatch, token-tag-dispatch, byte-string, rule-ref, character-class, sequence, choices).

use xgrammar::{
    fsm::{CompactFsm, EdgeKind, FsmWithStartEnd, NO_NEXT_STATE, RegexFsmBuilder, TrieFsmBuilder},
    functor::GrammarFsmBuilder,
    grammar::{Grammar, GrammarBuilder, GrammarExpr, GrammarExprType, TagDispatch, TokenTagDispatch},
};

#[test]
fn test_trie_fsm_builder() {
    let patterns: [&[u8]; 6] = [b"hello", b"hi", "哈哈".as_bytes(), "哈".as_bytes(), b"hili", b"good"];
    let fsm = TrieFsmBuilder::build(&patterns, &[], None, true, false).unwrap();

    // A compact round-trip is constructible from the built trie.
    let _compact = CompactFsm::from_fsm(fsm.fsm());

    assert_eq!(fsm.start(), 0);

    let next = |state: i32, c: u8| fsm.fsm().next_state(state, i32::from(c), EdgeKind::CharRange);

    // "hello".
    assert_eq!(next(fsm.start(), b'h'), 1);
    assert_eq!(next(1, b'e'), 2);
    assert_eq!(next(2, b'l'), 3);
    assert_eq!(next(3, b'l'), 4);
    assert_eq!(next(4, b'o'), 5);
    assert!(fsm.is_end_state(5));

    // "hil" (prefix of "hili", not itself a pattern).
    assert_eq!(next(fsm.start(), b'h'), 1);
    assert_eq!(next(1, b'i'), 6);
    assert_eq!(next(6, b'l'), 13);
    assert!(!fsm.is_end_state(13));

    // Walk failure: "goe" has no transition after "go".
    assert_eq!(next(fsm.start(), b'g'), 15);
    assert_eq!(next(15, b'o'), 16);
    assert_eq!(next(16, b'e'), -1);
}

#[test]
fn test_trie_excluded_suffixes() {
    // Following a longer pattern must not hide an excluded suffix at an explicit trie node,
    // including suffixes inherited through multiple failure links.
    let excluded_sets: [&[&[u8]]; 4] =
        [&[b"b", b"abx"], &[b"abx", b"b"], &[b"b", b"abx", b"xabxx"], &[b"ab", b"ab", b"abx"]];
    for excludes in excluded_sets {
        let fsm_wse = TrieFsmBuilder::build(&[], excludes, None, true, true).unwrap();
        let fsm = fsm_wse.fsm();
        // Compare the byte FSM against an independent substring predicate for every string up to
        // length five, rather than depending on generated state numbers.
        let mut inputs: Vec<Vec<u8>> = vec![Vec::new()];
        for length in 0..=5 {
            let mut next = Vec::new();
            for input in &inputs {
                let expected = excludes.iter().all(|excluded| !input.windows(excluded.len()).any(|w| w == *excluded));
                let mut state = fsm_wse.start();
                for &byte in input {
                    state = fsm.next_state(state, i32::from(byte), EdgeKind::CharRange);
                    if state == NO_NEXT_STATE {
                        break;
                    }
                }
                assert_eq!(state != NO_NEXT_STATE, expected, "{}", String::from_utf8_lossy(input));
                if length < 5 {
                    for &byte in b"abx" {
                        let mut extended = input.clone();
                        extended.push(byte);
                        next.push(extended);
                    }
                }
            }
            inputs = next;
        }
    }
}

/// Builds a `TagDispatch` from string tags.
fn tag_dispatch(
    pairs: &[(&str, i32)],
    loop_after: bool,
    excludes: &[&str],
) -> TagDispatch {
    TagDispatch {
        tag_rule_pairs: pairs.iter().map(|&(t, r)| (t.as_bytes().to_vec(), r)).collect(),
        loop_after_dispatch: loop_after,
        excludes: excludes.iter().map(|s| s.as_bytes().to_vec()).collect(),
    }
}

#[test]
fn test_tag_dispatch_fsm_builder1() {
    let td = tag_dispatch(&[("hel", 1), ("hi", 2), ("哈", 3)], true, &[]);
    let fsm = GrammarFsmBuilder::tag_dispatch(&td).unwrap();
    let expected = r#"FSM(num_states=8, start=0, end=[0, 1, 2, 5, 6], edges=[
0: [[\0-g]->0, 'h'->1, [i-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
1: [[\0-d]->0, 'e'->2, [f-g]->0, 'h'->1, 'i'->4, [j-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
2: [[\0-g]->0, 'h'->1, [i-k]->0, 'l'->3, [m-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
3: [Rule(1)->0]
4: [Rule(2)->0]
5: [[\0-g]->0, 'h'->1, [i-\x92]->0, '\x93'->6, [\x94-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
6: [[\0-g]->0, 'h'->1, [i-\x87]->0, '\x88'->7, [\x89-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
7: [Rule(3)->0]
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_tag_dispatch_fsm_builder2() {
    let td = tag_dispatch(&[("hel", 1), ("hi", 2), ("哈", 3)], false, &[]);
    let fsm = GrammarFsmBuilder::tag_dispatch(&td).unwrap();
    let expected = r#"FSM(num_states=11, start=0, end=[0, 1, 2, 5, 6, 8, 9, 10], edges=[
0: [[\0-g]->0, 'h'->1, [i-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
1: [[\0-d]->0, 'e'->2, [f-g]->0, 'h'->1, 'i'->4, [j-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
2: [[\0-g]->0, 'h'->1, [i-k]->0, 'l'->3, [m-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
3: [Rule(1)->8]
4: [Rule(2)->9]
5: [[\0-g]->0, 'h'->1, [i-\x92]->0, '\x93'->6, [\x94-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
6: [[\0-g]->0, 'h'->1, [i-\x87]->0, '\x88'->7, [\x89-\xe4]->0, '\xe5'->5, [\xe6-\xff]->0]
7: [Rule(3)->10]
8: []
9: []
10: []
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_tag_dispatch_fsm_builder3() {
    let td = tag_dispatch(&[("hel", 1), ("hi", 2), ("哈", 3)], true, &["hos", "eos"]);
    let fsm = GrammarFsmBuilder::tag_dispatch(&td).unwrap();
    let printed = fsm.to_string();
    assert!(printed.contains("Rule(1)->0"));
    assert!(printed.contains("Rule(2)->0"));
    assert!(printed.contains("Rule(3)->0"));
}

#[test]
fn test_token_tag_dispatch_fsm_builder() {
    let ttd = TokenTagDispatch {
        trigger_rule_pairs: vec![(3, 1), (5, 2)],
        loop_after_dispatch: false,
        excludes: vec![7],
    };
    let fsm = GrammarFsmBuilder::token_tag_dispatch(&ttd);
    let printed = fsm.to_string();
    assert!(printed.contains("Token"));
    assert!(printed.contains("ExcludeToken"));
}

#[test]
fn test_byte_string_fsm_builder1() {
    let data: [i32; 5] = [i32::from(b'h'), i32::from(b'e'), i32::from(b'l'), i32::from(b'l'), i32::from(b'o')];
    let expr = GrammarExpr {
        ty: GrammarExprType::ByteString,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::byte_string(&expr);
    let expected = r#"FSM(num_states=6, start=0, end=[5], edges=[
0: ['h'->1]
1: ['e'->2]
2: ['l'->3]
3: ['l'->4]
4: ['o'->5]
5: []
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_byte_string_fsm_builder2() {
    let data: Vec<i32> = "你好".bytes().map(i32::from).collect();
    let expr = GrammarExpr {
        ty: GrammarExprType::ByteString,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::byte_string(&expr);
    let expected = r#"FSM(num_states=7, start=0, end=[6], edges=[
0: ['\xe4'->1]
1: ['\xbd'->2]
2: ['\xa0'->3]
3: ['\xe5'->4]
4: ['\xa5'->5]
5: ['\xbd'->6]
6: []
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_rule_ref_fsm_builder() {
    let data: [i32; 1] = [1];
    let expr = GrammarExpr {
        ty: GrammarExprType::RuleRef,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::rule_ref(&expr);
    let expected = r#"FSM(num_states=2, start=0, end=[1], edges=[
0: [Rule(1)->1]
1: []
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_character_class_fsm_builder1() {
    let data: [i32; 5] = [0, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClass,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=2, start=0, end=[1], edges=[
0: [[a-z]->1, [A-Z]->1]
1: []
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_character_class_fsm_builder2() {
    let data: [i32; 5] = [0, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClassStar,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=1, start=0, end=[0], edges=[
0: [[a-z]->0, [A-Z]->0]
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_character_class_fsm_builder3() {
    let data: [i32; 5] = [1, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClass,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=16, start=0, end=[1], edges=[
0: [[\0-@]->1, [[-`]->1, [{-\x7f]->1, [\xc2-\xdf]->2, '\xe0'->3, [\xe1-\xef]->5, '\xf0'->7, '\xf4'->10, [\xf1-\xf3]->13]
1: []
2: [[\x80-\xbf]->1]
3: [[\xa0-\xbf]->4]
4: [[\x80-\xbf]->1]
5: [[\x80-\xbf]->6]
6: [[\x80-\xbf]->1]
7: [[\x90-\xbf]->8]
8: [[\x80-\xbf]->9]
9: [[\x80-\xbf]->1]
10: [[\x80-\x8f]->11]
11: [[\x80-\xbf]->12]
12: [[\x80-\xbf]->1]
13: [[\x80-\xbf]->14]
14: [[\x80-\xbf]->15]
15: [[\x80-\xbf]->1]
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_character_class_fsm_builder4() {
    let data: [i32; 5] = [1, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClassStar,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=15, start=0, end=[0], edges=[
0: [[\0-@]->0, [[-`]->0, [{-\x7f]->0, [\xc2-\xdf]->1, '\xe0'->2, [\xe1-\xef]->4, '\xf0'->6, '\xf4'->9, [\xf1-\xf3]->12]
1: [[\x80-\xbf]->0]
2: [[\xa0-\xbf]->3]
3: [[\x80-\xbf]->0]
4: [[\x80-\xbf]->5]
5: [[\x80-\xbf]->0]
6: [[\x90-\xbf]->7]
7: [[\x80-\xbf]->8]
8: [[\x80-\xbf]->0]
9: [[\x80-\x8f]->10]
10: [[\x80-\xbf]->11]
11: [[\x80-\xbf]->0]
12: [[\x80-\xbf]->13]
13: [[\x80-\xbf]->14]
14: [[\x80-\xbf]->0]
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_sequence_fsm_builder() {
    let grammar = Grammar::from_ebnf(
        "root ::= rule1 rule2 rule3\n\
         rule1 ::= \"a\" [a-z]* rule3\n\
         rule2 ::= \"c\" [A-Z] rule3\n\
         rule3 ::= \"a\" rule3\n",
        "root",
    )
    .unwrap();

    let root = GrammarFsmBuilder::choices(&grammar.expr(grammar.root_rule().body_expr_id), &grammar).unwrap();
    assert_eq!(
        root.to_string(),
        r#"FSM(num_states=4, start=2, end=[3], edges=[
0: [Rule(2)->1]
1: [Rule(3)->3]
2: [Rule(1)->0]
3: []
])"#
    );

    let rule1 = GrammarFsmBuilder::choices(&grammar.expr(grammar.rule(1).body_expr_id), &grammar).unwrap();
    assert_eq!(
        rule1.to_string(),
        r#"FSM(num_states=3, start=1, end=[2], edges=[
0: [Rule(3)->2, [a-z]->0]
1: ['a'->0]
2: []
])"#
    );

    let rule2 = GrammarFsmBuilder::choices(&grammar.expr(grammar.rule(2).body_expr_id), &grammar).unwrap();
    assert_eq!(
        rule2.to_string(),
        r#"FSM(num_states=4, start=2, end=[3], edges=[
0: [[A-Z]->1]
1: [Rule(3)->3]
2: ['c'->0]
3: []
])"#
    );

    let rule3 = GrammarFsmBuilder::choices(&grammar.expr(grammar.rule(3).body_expr_id), &grammar).unwrap();
    assert_eq!(
        rule3.to_string(),
        r#"FSM(num_states=3, start=1, end=[2], edges=[
0: [Rule(3)->2]
1: ['a'->0]
2: []
])"#
    );
}

#[test]
fn test_choices_fsm_builder() {
    let grammar = Grammar::from_ebnf(
        "root ::= rule1 | rule2\n\
         rule1 ::= \"\" | \"hello\" rule2\n\
         rule2 ::= [a-z]* \"A\" | \"B\" rule2\n",
        "root",
    )
    .unwrap();

    let root = GrammarFsmBuilder::choices(&grammar.expr(grammar.root_rule().body_expr_id), &grammar).unwrap();
    assert_eq!(
        root.to_string(),
        r#"FSM(num_states=3, start=0, end=[1, 2], edges=[
0: [Rule(1)->1, Rule(2)->2]
1: []
2: []
])"#
    );

    let rule1 = GrammarFsmBuilder::choices(&grammar.expr(grammar.rule(1).body_expr_id), &grammar).unwrap();
    assert_eq!(
        rule1.to_string(),
        r#"FSM(num_states=7, start=0, end=[0, 6], edges=[
0: ['h'->2]
1: [Rule(2)->6]
2: ['e'->3]
3: ['l'->4]
4: ['l'->5]
5: ['o'->1]
6: []
])"#
    );

    let rule2 = GrammarFsmBuilder::choices(&grammar.expr(grammar.rule(2).body_expr_id), &grammar).unwrap();
    assert_eq!(
        rule2.to_string(),
        r#"FSM(num_states=4, start=1, end=[0], edges=[
0: []
1: [Eps->2, 'B'->3]
2: ['A'->0, [a-z]->2]
3: [Rule(2)->0]
])"#
    );
}

/// Builds `regex` without a grammar builder.
fn build(regex: &str) -> Result<FsmWithStartEnd, String> {
    RegexFsmBuilder::build(regex.as_bytes(), None, "")
}

#[test]
fn test_regex_build_with_forbidden_chars() {
    // \S matches any non-whitespace byte. With the JSON forbidden characters removed, the
    // quote and the backslash must be rejected while other printable characters stay.
    let forbidden = GrammarFsmBuilder::json_string_forbidden_chars();
    let build_forbidden =
        |regex: &str| RegexFsmBuilder::build_with_forbidden_chars(regex.as_bytes(), forbidden, None, "").unwrap();
    let fsm_wse = build_forbidden(r"\S+");
    assert!(fsm_wse.accept_string("abc"));
    assert!(fsm_wse.accept_string("a!~z"));
    assert!(!fsm_wse.accept_string("a\"b"));
    assert!(!fsm_wse.accept_string("a\\b"));
    assert!(!fsm_wse.accept_string("a b"));

    // A positive class spanning the quote and the backslash is split around them.
    let fsm_wse = build_forbidden("[ -~]+");
    assert!(fsm_wse.accept_string("A z!"));
    assert!(!fsm_wse.accept_string("\""));
    assert!(!fsm_wse.accept_string("\\"));

    // . matches any byte; the control characters must be rejected as well.
    let fsm_wse = build_forbidden(".");
    assert!(fsm_wse.accept_string("a"));
    assert!(!fsm_wse.accept_string("\t"));
    assert!(!fsm_wse.accept_string("\n"));
    assert!(!fsm_wse.accept_string("\""));

    // Epsilon transitions are preserved: the pattern still accepts the empty string.
    let fsm_wse = build_forbidden("(ab)*");
    assert!(fsm_wse.accept_string(""));
    assert!(fsm_wse.accept_string("abab"));
    assert!(!fsm_wse.accept_string("a"));

    // A pattern that requires a forbidden character becomes the empty language.
    let fsm_wse = build_forbidden("a\"b");
    assert!(!fsm_wse.accept_string("a\"b"));
    assert!(!fsm_wse.accept_string("ab"));
    assert!(!fsm_wse.accept_string(""));

    // Multi-byte UTF-8 characters (bytes >= 0x80) are not affected by the JSON exclusion.
    let fsm_wse = build_forbidden(".+");
    assert!(fsm_wse.accept_string("你好"));
}

#[test]
fn test_regex_build_with_forbidden_chars_preserves_repeat_aux_data() {
    let forbidden = GrammarFsmBuilder::json_string_forbidden_chars();
    let mut builder = GrammarBuilder::new();

    let fsm_wse =
        RegexFsmBuilder::build_with_forbidden_chars(br"[^\n\r]{1,129}", forbidden, Some(&mut builder), "").unwrap();
    let fsm = fsm_wse.fsm();
    let aux_data = fsm.edge_aux_data();
    assert!(!aux_data.is_empty());

    let mut found_repeat_edge = false;
    for state in 0..fsm.num_states() {
        for edge in fsm.state_edges(state) {
            if !edge.is_repeat_ref() {
                continue;
            }
            found_repeat_edge = true;
            assert!(edge.aux_index() >= 0);
            assert!(edge.aux_index() + 2 < aux_data.len() as i32);

            let repeat_info = fsm.repeat_edge_info(edge.aux_index());
            assert_eq!(repeat_info.lower(), 1);
            assert_eq!(repeat_info.upper(), 129);
        }
    }
    assert!(found_repeat_edge);
}

#[test]
fn test_grammar_fsm_builder_regex() {
    // The compiled regex automaton must preserve the language after simplification.
    let fsm_wse = GrammarFsmBuilder::regex(b"(ab)+", false).unwrap();
    assert!(!fsm_wse.accept_string(""));
    assert!(!fsm_wse.accept_string("a"));
    assert!(fsm_wse.accept_string("ab"));
    assert!(fsm_wse.accept_string("abab"));

    let fsm_wse = GrammarFsmBuilder::regex(b"[0-9]{5}$", false).unwrap();
    assert!(fsm_wse.accept_string("12345"));
    assert!(!fsm_wse.accept_string("1234"));
    assert!(!fsm_wse.accept_string("123456"));

    // json_string excludes the JSON forbidden characters.
    let fsm_wse = GrammarFsmBuilder::regex(br"\S+", true).unwrap();
    assert!(fsm_wse.accept_string("abc"));
    assert!(!fsm_wse.accept_string("a\"b"));
    assert!(!fsm_wse.accept_string("a\\b"));

    // Without the flag, the quote is accepted.
    let fsm_wse = GrammarFsmBuilder::regex(br"\S+", false).unwrap();
    assert!(fsm_wse.accept_string("a\"b"));

    // Invalid patterns report an error.
    assert!(GrammarFsmBuilder::regex(b"+a", false).is_err());
}

#[test]
fn test_regex_repeat_zero() {
    // {0} / {0,0} matches exactly the empty string (ECMA-262 semantics, and consistent
    // with the EBNF parser and the CFG fallback path).
    let fsm_wse = build("a{0}").unwrap();
    assert!(fsm_wse.accept_string(""));
    assert!(!fsm_wse.accept_string("a"));

    let fsm_wse = build("a{0,0}").unwrap();
    assert!(fsm_wse.accept_string(""));
    assert!(!fsm_wse.accept_string("a"));

    let fsm_wse = build("(ab){0}").unwrap();
    assert!(fsm_wse.accept_string(""));
    assert!(!fsm_wse.accept_string("ab"));

    // In a concatenation the {0} element contributes nothing.
    let fsm_wse = build("ba{0}c").unwrap();
    assert!(fsm_wse.accept_string("bc"));
    assert!(!fsm_wse.accept_string("bac"));

    // The empty-string FSM survives the simplification passes (those of the C++
    // `GrammarFSMBuilder::Regex`).
    let fsm_wse = build("ba{0}c").unwrap().simplify_epsilon().merge_equivalent_states();
    assert!(fsm_wse.accept_string("bc"));
    assert!(!fsm_wse.accept_string("bac"));

    // A lower bound larger than the upper bound is an error in every regex dialect.
    assert!(build("a{3,1}").is_err());
    assert!(build("a{1,0}").is_err());
}

#[test]
fn test_regex_case_insensitive_flag() {
    // The (?i) prefix folds ASCII letters in literals, alternations and repeats.
    let fsm_wse = build("(?i)abc").unwrap();
    assert!(fsm_wse.accept_string("abc"));
    assert!(fsm_wse.accept_string("ABC"));
    assert!(fsm_wse.accept_string("aBc"));
    assert!(!fsm_wse.accept_string("abd"));

    let fsm_wse = build("(?i)(ab|cd)+").unwrap();
    assert!(fsm_wse.accept_string("aBcD"));
    assert!(!fsm_wse.accept_string("ac"));

    // Positive classes fold both the explicit letters and letter ranges.
    let fsm_wse = build("(?i)[a-dx]").unwrap();
    assert!(fsm_wse.accept_string("b"));
    assert!(fsm_wse.accept_string("B"));
    assert!(fsm_wse.accept_string("X"));
    assert!(!fsm_wse.accept_string("e"));
    assert!(!fsm_wse.accept_string("E"));

    // Negated classes exclude both cases of the folded letters.
    let fsm_wse = build("(?i)[^k]").unwrap();
    assert!(fsm_wse.accept_string("a"));
    assert!(!fsm_wse.accept_string("k"));
    assert!(!fsm_wse.accept_string("K"));

    // Only ASCII letters are folded; non-ASCII characters match literally.
    let fsm_wse = build("(?i)Σ").unwrap();
    assert!(fsm_wse.accept_string("Σ"));
    assert!(!fsm_wse.accept_string("σ"));

    // Without the prefix the match stays case-sensitive.
    let fsm_wse = build("abc").unwrap();
    assert!(!fsm_wse.accept_string("ABC"));
}

#[test]
fn test_regex_escapes() {
    // \xHH, \uHHHH and \u{...} escapes, both standalone and inside classes.
    let fsm_wse = build(r"\x41B\u{43}").unwrap();
    assert!(fsm_wse.accept_string("ABC"));

    let fsm_wse = build(r"\u{1F600}").unwrap();
    assert!(fsm_wse.accept_string("😀"));
    assert!(!fsm_wse.accept_string("😁"));

    let fsm_wse = build(r"[A-C]+").unwrap();
    assert!(fsm_wse.accept_string("ABC"));
    assert!(!fsm_wse.accept_string("D"));

    // A quantifier after a multi-byte character applies to the whole codepoint.
    let fsm_wse = build("好*").unwrap();
    assert!(fsm_wse.accept_string(""));
    assert!(fsm_wse.accept_string("好好"));
    assert!(!fsm_wse.accept_string(b"\xbd"));

    // \s matches exactly the standard whitespace characters [ \t\n\r\f\v].
    let fsm_wse = build(r"\s+").unwrap();
    assert!(fsm_wse.accept_string(" \t\n\r\x0c\x0b"));
    assert!(!fsm_wse.accept_string("\0"));
    assert!(!fsm_wse.accept_string("\x01"));

    // \S is the codepoint-domain complement, so non-ASCII characters are accepted.
    let fsm_wse = build(r"\S").unwrap();
    assert!(fsm_wse.accept_string("好"));
    assert!(fsm_wse.accept_string("\0"));
    assert!(!fsm_wse.accept_string(" "));
}

#[test]
fn test_regex_unsupported_features() {
    // Word boundaries, Unicode properties and backreferences raise errors.
    assert!(build(r"a\b").is_err());
    assert!(build(r"a\B").is_err());
    assert!(build(r"\p{L}").is_err());
    assert!(build(r"(a)\1").is_err());
    assert!(build(r"(?<name>a)\k<name>").is_err());
    assert!(build("(?<=a)b").is_err());
    assert!(build("[]").is_err());
    assert!(build("[^]").is_err());

    // Lookahead assertions are ignored with a warning (treated as the empty string).
    let fsm_wse = build("a(?=b)c").unwrap();
    assert!(fsm_wse.accept_string("ac"));
    let fsm_wse = build("a(?!b)c").unwrap();
    assert!(fsm_wse.accept_string("ac"));

    // Named groups compile like plain groups; the name is ignored.
    let fsm_wse = build("(?<name>ab)+").unwrap();
    assert!(fsm_wse.accept_string("abab"));
    let fsm_wse = build("(?P<name>ab)c").unwrap();
    assert!(fsm_wse.accept_string("abc"));

    // Mid-pattern anchors are ignored with a warning.
    let fsm_wse = build("a^b$c").unwrap();
    assert!(fsm_wse.accept_string("abc"));

    // Non-greedy quantifiers behave like their greedy counterparts.
    let fsm_wse = build("a*?b+?c??(de){1,2}?").unwrap();
    assert!(fsm_wse.accept_string("aabbcde"));
    assert!(fsm_wse.accept_string("bdede"));

    // Empty alternatives match the empty string.
    let fsm_wse = build("(a|)b").unwrap();
    assert!(fsm_wse.accept_string("ab"));
    assert!(fsm_wse.accept_string("b"));
    let fsm_wse = build("a|").unwrap();
    assert!(fsm_wse.accept_string("a"));
    assert!(fsm_wse.accept_string(""));
}

#[test]
fn test_regex_error_messages() {
    // Parse errors mirror the RegexConverter format and carry the 1-based position.
    let error_message = |regex: &str| build(regex).unwrap_err();
    assert_eq!(error_message("+a"), "Regex parsing error at position 1: There is nothing to repeat before '+'");
    assert_eq!(error_message("ab)"), "Regex parsing error at position 3: Unmatched ')'");
    assert_eq!(error_message("a]"), "Regex parsing error at position 2: Unmatched ']'");
    assert_eq!(error_message("a(bc"), "Regex parsing error at position 2: The parenthesis is not closed");
    assert_eq!(error_message("a[bc"), "Regex parsing error at position 2: Unclosed '['");
    assert_eq!(
        error_message("a[]"),
        "Regex parsing error at position 2: Empty character class is not allowed in regex"
    );
    assert_eq!(
        error_message("a{,2}"),
        "Regex parsing error at position 2: Invalid repetition count: expected a number after '{'"
    );
    assert_eq!(
        error_message("a{3,1}"),
        "Regex parsing error at position 2: Invalid repetition count: the lower bound 3 is larger than the upper \
         bound 1"
    );
    assert_eq!(
        error_message("a{1,9999999999}"),
        "Regex parsing error at position 2: Invalid repetition count: the count 9999999999 is too large"
    );
    assert_eq!(
        error_message(r"a\b"),
        r"Regex parsing error at position 2: Word boundary assertion \b is not supported in regex"
    );
    assert_eq!(error_message("(?<name)a"), "Regex parsing error at position 1: Invalid named capturing group");
    // The position includes the "(?i)" prefix when present.
    assert_eq!(error_message("(?i)+a"), "Regex parsing error at position 5: There is nothing to repeat before '+'");
}

#[test]
fn test_regex_matches_empty() {
    let matches_empty = |regex: &str| RegexFsmBuilder::matches_empty(regex.as_bytes());
    assert!(matches_empty("a*").unwrap());
    assert!(matches_empty("(a|)").unwrap());
    assert!(matches_empty("a{0,10}").unwrap());
    assert!(matches_empty("(ab)?").unwrap());
    assert!(!matches_empty("a+").unwrap());
    assert!(!matches_empty("a{1,10}").unwrap());
    assert!(!matches_empty("ab|cd").unwrap());
    // No FSM is built, so huge bounded repetitions are cheap to check.
    assert!(!matches_empty("(abc){2,1000000}").unwrap());
    assert!(matches_empty("(abc){0,1000000}").unwrap());
    assert!(matches_empty("(?i)A*").unwrap());
    assert!(matches_empty(r"\b").is_err());
}

#[test]
fn test_regex_large_repeat_without_builder() {
    // Without a GrammarBuilder, moderate repetitions are unrolled physically.
    let fsm_wse = build("a{2,300}").unwrap();
    assert!(!fsm_wse.accept_string("a"));
    assert!(fsm_wse.accept_string("aa"));
    assert!(fsm_wse.accept_string("a".repeat(300)));
    assert!(!fsm_wse.accept_string("a".repeat(301)));

    // Repetitions whose estimated state count is too large report an error instead of hanging.
    assert!(build("(abcdefghij){1,100000}").is_err());
}
