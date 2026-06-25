//! Port of `xgrammar/tests/cpp/test_fsm_builder.cc`.
//!
//! The `TrieFsmBuilder` cases are ported here. The `GrammarFSMBuilder` cases (tag-dispatch,
//! byte-string, rule-ref, character-class, sequence, choices) depend on the grammar→FSM
//! compiler bridge and are filled in with the compiler milestone.

use xgrammar::fsm::{CompactFsm, EdgeKind, TrieFsmBuilder};

#[test]
fn test_trie_fsm_builder() {
    let patterns = ["hello", "hi", "哈哈", "哈", "hili", "good"];
    let fsm = TrieFsmBuilder::build(&patterns, &[], None, true, false).unwrap();

    // A compact round-trip is constructible from the built trie.
    let _compact = CompactFsm::from_fsm(fsm.fsm());

    // The start state is 0.
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
