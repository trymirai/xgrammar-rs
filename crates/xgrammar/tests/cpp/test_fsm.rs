//! Port of `xgrammar/tests/cpp/test_fsm.cc`.
//!
//! Ported from `build` → `accept_string` acceptance through the full automata pipeline
//! (DFA conversion, minimization, epsilon simplification, state merging, intersection,
//! complement, and the compact representation).

use xgrammar::fsm::{
    CompactFsm, CompactFsmWithStartEnd, Fsm, FsmWithStartEnd, build_regex_fsm,
};

/// Whether some character-range edge out of state 0 covers `c` — mirrors the in-test lambda
/// the C++ uses to probe a single character class's first state.
fn state0_accepts_char(
    fsm_wse: &xgrammar::fsm::FsmWithStartEnd,
    c: u8,
) -> bool {
    fsm_wse
        .fsm()
        .state_edges(0)
        .iter()
        .any(|e| e.min <= i32::from(c) && e.max >= i32::from(c))
}

#[test]
fn basic_build_test() {
    // Test 1: literal string with an escape.
    let fsm_wse = build_regex_fsm("abcd\\n").unwrap();
    assert!(fsm_wse.accept_string("abcd\n"));

    // Test 2: a class mixing a leading dash, a range, and an escape.
    let fsm_wse = build_regex_fsm("[-a-z\\n]").unwrap();
    for c in "abcd-\n".bytes() {
        assert!(state0_accepts_char(&fsm_wse, c));
    }

    // Test 3: digit class.
    let fsm_wse = build_regex_fsm("[\\d]").unwrap();
    for c in "1234567890".bytes() {
        assert!(state0_accepts_char(&fsm_wse, c));
    }

    // Test 4: negated digit class.
    let fsm_wse = build_regex_fsm("[^\\d]").unwrap();
    for c in "1234567890".bytes() {
        assert!(!state0_accepts_char(&fsm_wse, c));
    }
    for c in "abz".bytes() {
        assert!(state0_accepts_char(&fsm_wse, c));
    }

    // Test 5: multibyte literal.
    let fsm_wse = build_regex_fsm("你好a").unwrap();
    assert!(fsm_wse.accept_string("你好a"));

    // Test 6: empty groups accept the empty string.
    let fsm_wse = build_regex_fsm("(())()()").unwrap();
    assert!(fsm_wse.accept_string(""));

    // Test 7: a class with duplicates collapses to merged ranges (two: a-d and x-z).
    let fsm_wse = build_regex_fsm("[abcdabcdxyzxyz]").unwrap();
    assert!(fsm_wse.accept_string("a"));
    assert!(!fsm_wse.accept_string("e"));
    assert_eq!(fsm_wse.fsm().state_edges(0).len(), 2);
}

#[test]
fn connection_test() {
    // Test 1: literal, class, and trailing literals concatenated.
    let fsm_wse = build_regex_fsm(" [a-zA-Z0-9]--").unwrap();
    assert!(fsm_wse.accept_string(" a--"));

    // Test 2: alternation of a literal and a class.
    let fsm_wse = build_regex_fsm("aaa|[\\d]").unwrap();
    assert!(fsm_wse.accept_string("aaa"));
    assert!(fsm_wse.accept_string("1"));

    // Test 3: nested alternation in groups.
    let fsm_wse = build_regex_fsm("(([\\d]|[\\w])|aaa)").unwrap();
    assert!(fsm_wse.accept_string("aaa"));
    assert!(fsm_wse.accept_string("1"));
    assert!(!fsm_wse.accept_string("1a"));
}

#[test]
fn symbol_test() {
    // Test 1: `+` quantifier.
    let fsm_wse = build_regex_fsm("1[\\d]+").unwrap();
    assert!(fsm_wse.accept_string("1111"));
    assert!(!fsm_wse.accept_string("1"));

    // Test 2: `*` quantifier.
    let fsm_wse = build_regex_fsm("1[1]*").unwrap();
    assert!(fsm_wse.accept_string("1111"));
    assert!(fsm_wse.accept_string("1"));

    // Test 3: `?` quantifier.
    let fsm_wse = build_regex_fsm("1[\\d]?").unwrap();
    assert!(!fsm_wse.accept_string("1111"));
    assert!(fsm_wse.accept_string("1"));
    assert!(fsm_wse.accept_string("11"));

    // Test 4: quantified literal spaces.
    let fsm_wse = build_regex_fsm(" * * + ? *").unwrap();
    assert!(fsm_wse.accept_string(" "));
    assert!(fsm_wse.accept_string("      "));
}

#[test]
fn integrated_test() {
    let fsm_wse = build_regex_fsm("((naive|bbb|[\\d]+)*[\\w])|  +").unwrap();
    for s in ["naive1", "bbbnaive114514W", "    ", "123", "_"] {
        assert!(fsm_wse.accept_string(s), "should accept {s:?}");
    }
    for s in ["naive", "bbbbbb", "naive   ", "123 ", "aaa"] {
        assert!(!fsm_wse.accept_string(s), "should reject {s:?}");
    }
}

#[test]
fn test_email() {
    let fsm_wse = build_regex_fsm(r"(\w+)(\.\w+)*@(\w+)(\.\w+)+").unwrap();
    for email in [
        "asnjdaj_19032910@google.com.test",
        "12393089340190@a.b.c.d.f.e.org.test",
        "as____________as@abc.me.test",
        "ooooohhhhh@123456.test",
        "ajidoa@a.test",
    ] {
        assert!(fsm_wse.accept_string(email), "should accept {email:?}");
    }
    for email in
        ["@google.test", "hello@", "hello@.test", "+++asd@b.test", "hello"]
    {
        assert!(!fsm_wse.accept_string(email), "should reject {email:?}");
    }
}

#[test]
fn test_time() {
    let fsm_wse = build_regex_fsm(r"(\d{1,2}):(\d{2})(:(\d{2}))?").unwrap();
    for time in ["1:34", "23:59", "00:00", "01:02:03", "23:59:59"] {
        assert!(fsm_wse.accept_string(time), "should accept {time:?}");
    }
    for time in [
        "19",
        "12:6",
        "12:34:",
        "12:34:5",
        "12:34:567",
        "12:123",
        "12:",
        ":34:23",
        "::",
    ] {
        assert!(!fsm_wse.accept_string(time), "should reject {time:?}");
    }
}

#[test]
fn function_test() {
    // Test 1: compact round-trip.
    let fsm_wse = build_regex_fsm("[\\d\\d\\d]+123").unwrap();
    assert!(fsm_wse.accept_string("123456123"));
    let compact = CompactFsm::from_fsm(fsm_wse.fsm());
    let compact_wse = CompactFsmWithStartEnd::new(
        compact.clone(),
        fsm_wse.start(),
        fsm_wse.ends().to_vec(),
        false,
    );
    assert!(compact_wse.accept_string("123456123"));
    let roundtrip = FsmWithStartEnd::new(
        compact.to_fsm(),
        fsm_wse.start(),
        fsm_wse.ends().to_vec(),
        false,
    );
    assert!(roundtrip.accept_string("123456123"));

    // Test 2: DFA conversion removes epsilons and de-duplicates transitions.
    let fsm_wse = build_regex_fsm("([abc]|[\\d])+").unwrap();
    assert!(fsm_wse.accept_string("abc3"));
    let dfa = fsm_wse.to_dfa().unwrap();
    assert!(dfa.accept_string("abc3"));
    assert!(
        dfa.fsm().edges().iter().all(|es| es.iter().all(|e| !e.is_epsilon()))
    );
    for edges in dfa.fsm().edges() {
        let mut rules = std::collections::HashSet::new();
        let mut chars = std::collections::HashSet::new();
        for e in edges {
            if e.is_rule_ref() {
                assert!(rules.insert(e.ref_rule_id()), "duplicate rule edge");
                continue;
            }
            for c in e.min..=e.max {
                assert!(chars.insert(c), "duplicate char transition");
            }
        }
    }

    // Test 3: minimization.
    let minimized = dfa.minimize_dfa().unwrap();
    assert!(minimized.accept_string("abc3"));
    assert_eq!(minimized.fsm().edges().len(), 2);

    // Test 4: complement.
    let complement = minimized.not().unwrap();
    assert!(!complement.accept_string("abc3"));
    assert!(complement.accept_string("abcd"));

    // Test 5: bounded and unbounded repetition.
    let fsm_wse = build_regex_fsm("[\\d]{1,5}").unwrap();
    assert!(fsm_wse.accept_string("123"));
    assert!(fsm_wse.accept_string("12345"));
    assert!(!fsm_wse.accept_string("123456"));
    assert!(!fsm_wse.accept_string("1234567"));
    let fsm_wse = build_regex_fsm("[\\d]{6}").unwrap();
    assert!(fsm_wse.accept_string("123456"));
    assert!(!fsm_wse.accept_string("1234567"));
    let fsm_wse = build_regex_fsm("[\\d]{6, }").unwrap();
    assert!(fsm_wse.accept_string("123456"));
    assert!(fsm_wse.accept_string("1234567"));

    // Test 6: epsilon simplification preserves acceptance and shrinks states.
    let fsm_wse = build_regex_fsm("[a][b][c][d]").unwrap();
    assert!(fsm_wse.accept_string("abcd"));
    let simplified = fsm_wse.simplify_epsilon();
    assert_eq!(simplified.fsm().num_states(), 5);
    assert!(simplified.accept_string("abcd"));

    // Test 7: shared-prefix merge.
    let fsm_wse = build_regex_fsm("abc|abd").unwrap();
    assert!(fsm_wse.accept_string("abc"));
    let merged = fsm_wse.simplify_epsilon().merge_equivalent_states();
    assert!(merged.accept_string("abc"));
    assert!(!merged.accept_string("abcd"));
    assert_eq!(merged.fsm().num_states(), 4);

    // Test 8: shared-suffix merge.
    let fsm_wse = build_regex_fsm("acd|bcd").unwrap();
    assert!(fsm_wse.accept_string("acd"));
    let merged = fsm_wse.simplify_epsilon().merge_equivalent_states();
    assert!(merged.accept_string("acd"));
    assert!(!merged.accept_string("abcd"));
    assert_eq!(merged.fsm().num_states(), 4);

    // Test 9: star simplification.
    let fsm_wse = build_regex_fsm("ab*").unwrap();
    assert!(fsm_wse.accept_string("abbb"));
    let simplified = fsm_wse.simplify_epsilon();
    assert!(simplified.accept_string("abbb"));
    assert_eq!(simplified.fsm().num_states(), 2);

    // Test 10: intersection.
    let left = build_regex_fsm("[c-f]+").unwrap();
    let right = build_regex_fsm("[d-h]*").unwrap();
    let intersection = FsmWithStartEnd::intersect(&left, &right).unwrap();
    assert!(intersection.accept_string("de"));
    assert!(intersection.accept_string("def"));
    assert!(!intersection.accept_string(""));
    assert!(!intersection.accept_string("cd"));
}

/// Builds a 10-state FSM by hand for the merge/epsilon string oracles.
fn manual_fsm(num_states: i32) -> FsmWithStartEnd {
    FsmWithStartEnd::new(
        Fsm::new(num_states as usize),
        0,
        vec![false; num_states as usize],
        false,
    )
}

#[test]
fn merging_nodes_test() {
    let mut fsm_wse = manual_fsm(10);
    fsm_wse.set_start_state(0);
    fsm_wse.add_end_state(9);
    let edges: &[(i32, i32, u8)] = &[
        (0, 1, b'a'),
        (0, 2, b'a'),
        (1, 3, b'b'),
        (1, 3, b'c'),
        (1, 4, b'b'),
        (1, 4, b'c'),
        (2, 5, b'b'),
        (2, 5, b'c'),
        (2, 6, b'b'),
        (2, 6, b'c'),
        (3, 7, b'd'),
        (4, 7, b'd'),
        (5, 8, b'd'),
        (6, 8, b'd'),
        (7, 9, b'e'),
        (8, 9, b'e'),
    ];
    for &(from, to, c) in edges {
        fsm_wse.fsm_mut().add_edge(from, to, i32::from(c), i32::from(c));
    }
    let merged = fsm_wse.merge_equivalent_states();
    let expected = "FSM(num_states=5, start=3, end=[4], edges=[\n\
        0: ['d'->2]\n\
        1: ['b'->0, 'c'->0]\n\
        2: ['e'->4]\n\
        3: ['a'->1]\n\
        4: []\n\
        ])";
    assert_eq!(merged.to_string(), expected);
    assert_eq!(merged.fsm().num_states(), 5);
}

#[test]
fn merge_equivalent_states_no_cross_rule_chaining() {
    let mut fsm_wse = manual_fsm(7);
    fsm_wse.set_start_state(0);
    fsm_wse.add_end_state(6);
    // 2 and 3 are equivalent successors of 0 under 'x' (Case 1).
    fsm_wse.fsm_mut().add_edge(0, 2, i32::from(b'x'), i32::from(b'x'));
    fsm_wse.fsm_mut().add_edge(0, 3, i32::from(b'x'), i32::from(b'x'));
    // 1 is another predecessor of 4 under 'a' (Case 2 candidate with 2).
    fsm_wse.fsm_mut().add_edge(0, 1, i32::from(b'y'), i32::from(b'y'));
    fsm_wse.fsm_mut().add_edge(1, 4, i32::from(b'a'), i32::from(b'a'));
    fsm_wse.fsm_mut().add_edge(2, 4, i32::from(b'a'), i32::from(b'a'));
    fsm_wse.fsm_mut().add_edge(3, 5, i32::from(b'b'), i32::from(b'b'));
    fsm_wse.fsm_mut().add_edge(4, 6, i32::from(b'm'), i32::from(b'm'));
    fsm_wse.fsm_mut().add_edge(5, 6, i32::from(b'n'), i32::from(b'n'));
    let merged = fsm_wse.merge_equivalent_states();
    assert!(merged.accept_string("xam"));
    assert!(merged.accept_string("xbn"));
    assert!(merged.accept_string("yam"));
    // Should not over-merge and introduce this path.
    assert!(!merged.accept_string("ybn"));
}

#[test]
fn epsilon_simplification_test() {
    let mut fsm_wse = manual_fsm(10);
    fsm_wse.set_start_state(0);
    fsm_wse.add_end_state(9);
    fsm_wse.fsm_mut().add_epsilon_edge(0, 1);
    fsm_wse.fsm_mut().add_epsilon_edge(0, 2);
    fsm_wse.fsm_mut().add_edge(1, 3, i32::from(b'b'), i32::from(b'b'));
    fsm_wse.fsm_mut().add_epsilon_edge(1, 3);
    fsm_wse.fsm_mut().add_edge(1, 4, i32::from(b'b'), i32::from(b'b'));
    fsm_wse.fsm_mut().add_edge(3, 3, i32::from(b'c'), i32::from(b'c'));
    fsm_wse.fsm_mut().add_epsilon_edge(2, 5);
    fsm_wse.fsm_mut().add_edge(2, 5, i32::from(b'c'), i32::from(b'c'));
    fsm_wse.fsm_mut().add_edge(2, 6, i32::from(b'b'), i32::from(b'b'));
    fsm_wse.fsm_mut().add_edge(2, 6, i32::from(b'c'), i32::from(b'c'));
    fsm_wse.fsm_mut().add_epsilon_edge(3, 7);
    fsm_wse.fsm_mut().add_epsilon_edge(4, 7);
    fsm_wse.fsm_mut().add_epsilon_edge(5, 8);
    fsm_wse.fsm_mut().add_epsilon_edge(6, 8);
    fsm_wse.fsm_mut().add_epsilon_edge(7, 9);
    fsm_wse.fsm_mut().add_epsilon_edge(8, 9);
    let simplified = fsm_wse.simplify_epsilon();
    let expected = "FSM(num_states=3, start=0, end=[1], edges=[\n\
        0: [Eps->1, Eps->2, 'b'->1, 'b'->2, 'c'->1]\n\
        1: []\n\
        2: [Eps->1, 'c'->2]\n\
        ])";
    assert_eq!(simplified.to_string(), expected);
    assert_eq!(simplified.fsm().num_states(), 3);
}

#[test]
fn efficiency_test() {
    // ([a-z]0123456789){10}, written as 10 concatenated 52-way alternations, exercises the
    // full simplify → merge → DFA → minimize pipeline. The minimal DFA has 111 states.
    let group = (0..52)
        .map(|i| {
            let letter = (b'a' + (i / 2) as u8) as char;
            format!("{letter}0123456789")
        })
        .collect::<Vec<_>>()
        .join("|");
    let regex = (0..10).map(|_| format!("({group})")).collect::<String>();
    let mut fsm_wse = build_regex_fsm(&regex).unwrap();
    fsm_wse = fsm_wse.simplify_epsilon();
    fsm_wse = fsm_wse.merge_equivalent_states();
    fsm_wse = fsm_wse.to_dfa().unwrap();
    fsm_wse = fsm_wse.minimize_dfa().unwrap();
    assert_eq!(fsm_wse.fsm().num_states(), 111);
}
