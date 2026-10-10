//! Builds an NFA from a (byte-oriented) regex.
//!
//! The regex is parsed into an AST ([`RegexIr`]) and then lowered into an [`FsmWithStartEnd`].
//! This is the FSM-level regex used to compile grammars, distinct from the regex→EBNF
//! converter.

use super::{fsm::Fsm, fsm_with_start_end::FsmWithStartEnd, regex_ir::RegexIr};
use crate::grammar::GrammarBuilder;

/// Converts regexes into FSMs.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegexFsmBuilder;

impl RegexFsmBuilder {
    /// Builds an FSM accepting the language of `regex`. A leading `(?i)` makes the match ASCII
    /// case-insensitive.
    ///
    /// With a `builder`, bounded repetitions whose upper bound exceeds the unroll threshold
    /// (128) are compiled into a repeat-ref edge referencing a new rule (with a regex body holding
    /// the repeated sub-pattern, named after `rule_hint`) added through `builder`, instead of
    /// being unrolled.
    ///
    /// # Errors
    /// Returns a message if the regex is malformed or uses an unsupported feature, or if a
    /// repetition that has to be unrolled is too large.
    pub fn build(
        regex: &[u8],
        builder: Option<&mut GrammarBuilder>,
        rule_hint: &str,
    ) -> Result<FsmWithStartEnd, String> {
        RegexIr::parse(regex, builder, rule_hint)?.build()
    }

    /// Builds the FSM of `regex` (see [`Self::build`]), then removes the forbidden bytes from
    /// every character edge. The result accepts the strings of the regex language that contain
    /// no forbidden byte; the language may be empty.
    ///
    /// # Errors
    /// See [`Self::build`].
    pub fn build_with_forbidden_chars(
        regex: &[u8],
        forbidden_chars: &[bool; 256],
        builder: Option<&mut GrammarBuilder>,
        rule_hint: &str,
    ) -> Result<FsmWithStartEnd, String> {
        let fsm_wse = Self::build(regex, builder, rule_hint)?;
        if !forbidden_chars.contains(&true) {
            return Ok(fsm_wse);
        }
        let fsm = fsm_wse.fsm();
        let mut new_fsm = Fsm::new(fsm_wse.num_states() as usize);
        for state in 0..fsm_wse.num_states() {
            for edge in fsm.state_edges(state) {
                if !edge.is_char_range() {
                    new_fsm.add_edge(state, edge.target, edge.min, edge.max);
                    continue;
                }
                // Split the character range into the maximal sub-ranges of allowed characters.
                let mut range_start = None;
                for c in edge.min..=edge.max + 1 {
                    if c <= edge.max && !forbidden_chars[c as usize] {
                        range_start.get_or_insert(c);
                    } else if let Some(start) = range_start.take() {
                        new_fsm.add_edge(state, edge.target, start, c - 1);
                    }
                }
            }
        }
        new_fsm.set_edge_aux_data(fsm.edge_aux_data().to_vec());
        Ok(FsmWithStartEnd::new(new_fsm, fsm_wse.start(), fsm_wse.ends().to_vec(), false))
    }

    /// Whether `regex` matches the empty string. Only parses the regex; no FSM is built, so this
    /// is cheap even for huge bounded repetitions.
    ///
    /// # Errors
    /// Returns a message if the regex is malformed or uses an unsupported feature.
    pub fn matches_empty(regex: &[u8]) -> Result<bool, String> {
        Ok(RegexIr::parse(regex, None, "")?.matches_empty())
    }
}
