//! Port of `xgrammar/tests/cpp/test_fsm.cc`.
//!
//! The string-acceptance subset (build → `accept_string`) is ported here. Tests that
//! additionally exercise DFA conversion, minimization, epsilon simplification, state
//! merging, intersection, complement, or the compact representation are filled in as those
//! FSM algorithms land.

use xgrammar::fsm::build_regex_fsm;

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
