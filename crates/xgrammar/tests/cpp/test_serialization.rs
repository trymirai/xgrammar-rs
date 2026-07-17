//! Port of selected cases from `xgrammar/tests/cpp/test_serialization.cc`
//! covering compact FSM JSON roundtrip.

use xgrammar::fsm::{
    CompactFsm, CompactFsmWithStartEnd, Fsm, FsmWithStartEnd,
};

#[test]
fn compact_fsm_with_start_end_json_roundtrip() {
    let mut ends = vec![false; 2];
    ends[1] = true;
    let mut fsm_wse = FsmWithStartEnd::new(Fsm::new(2), 0, ends, false);
    fsm_wse
        .fsm_mut()
        .add_edge(0, 1, i32::from(b'a'), i32::from(b'a'));

    let compact = CompactFsm::from_fsm(fsm_wse.fsm());
    let compact_wse = CompactFsmWithStartEnd::new(
        compact,
        fsm_wse.start(),
        fsm_wse.ends().to_vec(),
        false,
    );
    let json = compact_wse.serialize_json_value();
    let restored =
        CompactFsmWithStartEnd::deserialize_json_value(&json).unwrap();

    assert_eq!(restored.num_states(), compact_wse.num_states());
    assert_eq!(restored.start(), compact_wse.start());
    assert_eq!(restored.ends(), compact_wse.ends());
}
