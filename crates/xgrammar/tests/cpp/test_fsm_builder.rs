//! Port of `xgrammar/tests/cpp/test_fsm_builder.cc`.
//!
//! Covers the trie builder and the per-expression `GrammarFsmBuilder` (tag-dispatch,
//! token-tag-dispatch, byte-string, rule-ref, character-class, sequence, choices).

use xgrammar::{
    fsm::{CompactFsm, EdgeKind, TrieFsmBuilder},
    functor::GrammarFsmBuilder,
    grammar::{
        Grammar, GrammarExpr, GrammarExprType, TagDispatch, TokenTagDispatch,
    },
};

#[test]
fn test_trie_fsm_builder() {
    let patterns: [&[u8]; 6] =
        [b"hello", b"hi", "哈哈".as_bytes(), "哈".as_bytes(), b"hili", b"good"];
    let fsm = TrieFsmBuilder::build(&patterns, &[], None, true, false).unwrap();

    // A compact round-trip is constructible from the built trie.
    let _compact = CompactFsm::from_fsm(fsm.fsm());

    assert_eq!(fsm.start(), 0);

    let next = |state: i32, c: u8| {
        fsm.fsm().next_state(state, i32::from(c), EdgeKind::CharRange)
    };

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

/// Builds a `TagDispatch` from string tags.
fn tag_dispatch(
    pairs: &[(&str, i32)],
    loop_after: bool,
    excludes: &[&str],
) -> TagDispatch {
    TagDispatch {
        tag_rule_pairs: pairs
            .iter()
            .map(|&(t, r)| (t.as_bytes().to_vec(), r))
            .collect(),
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
    let td = tag_dispatch(
        &[("hel", 1), ("hi", 2), ("哈", 3)],
        true,
        &["hos", "eos"],
    );
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
    let fsm = GrammarFsmBuilder::token_tag_dispatch(&ttd).unwrap();
    let printed = fsm.to_string();
    assert!(printed.contains("Token"));
    assert!(printed.contains("ExcludeToken"));
}

#[test]
fn test_byte_string_fsm_builder1() {
    let data: [i32; 5] = [
        i32::from(b'h'),
        i32::from(b'e'),
        i32::from(b'l'),
        i32::from(b'l'),
        i32::from(b'o'),
    ];
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
    let data: [i32; 5] =
        [0, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
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
    let data: [i32; 5] =
        [0, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
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
    let data: [i32; 5] =
        [1, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClass,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=8, start=0, end=[1], edges=[
0: [[\0-@]->1, [[-`]->1, [{-\x7f]->1, [\xc0-\xdf]->2, [\xe0-\xef]->3, [\xf0-\xf7]->5]
1: []
2: [[\x80-\xbf]->1]
3: [[\x80-\xbf]->4]
4: [[\x80-\xbf]->1]
5: [[\x80-\xbf]->6]
6: [[\x80-\xbf]->7]
7: [[\x80-\xbf]->1]
])"#;
    assert_eq!(fsm.to_string(), expected);
}

#[test]
fn test_character_class_fsm_builder4() {
    let data: [i32; 5] =
        [1, i32::from(b'a'), i32::from(b'z'), i32::from(b'A'), i32::from(b'Z')];
    let expr = GrammarExpr {
        ty: GrammarExprType::CharacterClassStar,
        data: &data,
    };
    let fsm = GrammarFsmBuilder::character_class(&expr);
    let expected = r#"FSM(num_states=7, start=0, end=[0], edges=[
0: [[\0-@]->0, [[-`]->0, [{-\x7f]->0, [\xc0-\xdf]->1, [\xe0-\xef]->2, [\xf0-\xf7]->4]
1: [[\x80-\xbf]->0]
2: [[\x80-\xbf]->3]
3: [[\x80-\xbf]->0]
4: [[\x80-\xbf]->5]
5: [[\x80-\xbf]->6]
6: [[\x80-\xbf]->0]
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

    let root = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.root_rule().body_expr_id),
        &grammar,
    )
    .unwrap();
    assert_eq!(
        root.to_string(),
        r#"FSM(num_states=4, start=2, end=[3], edges=[
0: [Rule(2)->1]
1: [Rule(3)->3]
2: [Rule(1)->0]
3: []
])"#
    );

    let rule1 = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.rule(1).body_expr_id),
        &grammar,
    )
    .unwrap();
    assert_eq!(
        rule1.to_string(),
        r#"FSM(num_states=3, start=1, end=[2], edges=[
0: [Rule(3)->2, [a-z]->0]
1: ['a'->0]
2: []
])"#
    );

    let rule2 = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.rule(2).body_expr_id),
        &grammar,
    )
    .unwrap();
    assert_eq!(
        rule2.to_string(),
        r#"FSM(num_states=4, start=2, end=[3], edges=[
0: [[A-Z]->1]
1: [Rule(3)->3]
2: ['c'->0]
3: []
])"#
    );

    let rule3 = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.rule(3).body_expr_id),
        &grammar,
    )
    .unwrap();
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

    let root = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.root_rule().body_expr_id),
        &grammar,
    )
    .unwrap();
    assert_eq!(
        root.to_string(),
        r#"FSM(num_states=3, start=0, end=[1, 2], edges=[
0: [Rule(1)->1, Rule(2)->2]
1: []
2: []
])"#
    );

    let rule1 = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.rule(1).body_expr_id),
        &grammar,
    )
    .unwrap();
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

    let rule2 = GrammarFsmBuilder::choices(
        &grammar.expr(grammar.rule(2).body_expr_id),
        &grammar,
    )
    .unwrap();
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
