//! Incremental body-FSM progress of a budgeted suffix/stop rule occurrence.

use std::collections::HashSet;

/// The body FSM states reached after consuming `[begin_byte, end_byte)` of the accepted input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetBodyMatchProgress {
    /// The first byte of the occurrence.
    pub begin_byte: i64,
    /// The byte up to which the body FSM has been advanced.
    pub end_byte: i64,
    /// The body FSM states after `end_byte`.
    pub states: HashSet<i32>,
    /// The tentative input the progress was computed against, when it extends past the
    /// accepted bytes.
    pub temporary_input_snapshot: Vec<u8>,
}
