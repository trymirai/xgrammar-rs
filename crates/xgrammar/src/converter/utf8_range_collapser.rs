//! Collapses the UTF-8 byte paths of a byte automaton into codepoint transitions.

use std::collections::HashMap;

use crate::fsm::FsmWithStartEnd;

/// Turns the byte transitions of a leaf automaton into codepoint intervals, so that a string
/// automaton can be emitted as character classes (EBNF string literals denote codepoints, not raw
/// bytes). Only productive states (those that can still reach an accepting state) are kept, and
/// multi-byte suffixes are memoized so wide Unicode classes are not enumerated per codepoint.
pub struct Utf8RangeCollapser<'a> {
    fsm: &'a FsmWithStartEnd,
    productive: Vec<bool>,
    suffix_cache: HashMap<(i32, i32), Vec<[i32; 3]>>,
}

impl<'a> Utf8RangeCollapser<'a> {
    const LEADING_MIN: [i32; 4] = [0, 0xc2, 0xe0, 0xf0];
    const LEADING_MAX: [i32; 4] = [0, 0xdf, 0xef, 0xf4];
    const CODEPOINT_MIN: [i32; 4] = [0, 0x80, 0x800, 0x10000];
    const CODEPOINT_MAX: [i32; 4] = [0, 0x7ff, 0xffff, 0x10ffff];

    /// Analyzes `fsm`, whose edges must all be byte ranges.
    #[must_use]
    pub fn new(fsm: &'a FsmWithStartEnd) -> Self {
        let num_states = fsm.num_states() as usize;
        let mut predecessors = vec![Vec::new(); num_states];
        for state in 0..fsm.num_states() {
            for edge in fsm.fsm().state_edges(state) {
                predecessors[edge.target as usize].push(state);
            }
        }
        let mut productive = vec![false; num_states];
        let mut worklist: Vec<i32> = (0..fsm.num_states()).filter(|&state| fsm.is_end_state(state)).collect();
        for &state in &worklist {
            productive[state as usize] = true;
        }
        let mut index = 0;
        while index < worklist.len() {
            for &state in &predecessors[worklist[index] as usize] {
                if !productive[state as usize] {
                    productive[state as usize] = true;
                    worklist.push(state);
                }
            }
            index += 1;
        }
        Self {
            fsm,
            productive,
            suffix_cache: HashMap::new(),
        }
    }

    /// Whether an accepting state is reachable from `state`.
    #[must_use]
    pub fn is_productive(
        &self,
        state: i32,
    ) -> bool {
        self.productive[state as usize]
    }

    /// The merged `(target, min, max)` codepoint transitions out of `state`, sorted by target.
    pub fn state_ranges(
        &mut self,
        state: i32,
    ) -> Vec<[i32; 3]> {
        let mut ranges = Vec::new();
        let fsm = self.fsm;
        for edge in fsm.fsm().state_edges(state) {
            if !self.productive[edge.target as usize] {
                continue;
            }
            if edge.min < 128 {
                ranges.push([edge.target, edge.min, edge.max.min(127)]);
            }
            for remaining in 1..=3 {
                let min = edge.min.max(Self::LEADING_MIN[remaining]);
                let max = edge.max.min(Self::LEADING_MAX[remaining]);
                if min > max {
                    continue;
                }
                let mask = (1 << (6 - remaining)) - 1;
                let mut unicode_ranges = Vec::new();
                for suffix in self.suffix_ranges(edge.target, remaining as i32) {
                    Self::append(&mut unicode_ranges, min & mask, max & mask, 6 * remaining as i32, suffix);
                }
                // The byte automaton can contain non-canonical UTF-8 paths. Never turn an overlong
                // encoding into a second transition for an ASCII character (bypassing exclusions).
                for [target, min, max] in unicode_ranges {
                    let min = min.max(Self::CODEPOINT_MIN[remaining]);
                    let max = max.min(Self::CODEPOINT_MAX[remaining]);
                    if min <= max {
                        ranges.push([target, min, max]);
                    }
                }
            }
        }
        Self::merge(ranges)
    }

    /// The codepoint suffixes of `remaining` continuation bytes starting at `state`.
    fn suffix_ranges(
        &mut self,
        state: i32,
        remaining: i32,
    ) -> Vec<[i32; 3]> {
        if let Some(cached) = self.suffix_cache.get(&(state, remaining)) {
            return cached.clone();
        }
        let mut ranges = Vec::new();
        if remaining == 0 {
            ranges.push([state, 0, 0]);
        } else {
            let fsm = self.fsm;
            for edge in fsm.fsm().state_edges(state) {
                if !self.productive[edge.target as usize] {
                    continue;
                }
                let min = edge.min.max(0x80);
                let max = edge.max.min(0xbf);
                if min > max {
                    continue;
                }
                for suffix in self.suffix_ranges(edge.target, remaining - 1) {
                    Self::append(&mut ranges, min & 0x3f, max & 0x3f, 6 * (remaining - 1), suffix);
                }
            }
        }
        let merged = Self::merge(ranges);
        self.suffix_cache.insert((state, remaining), merged.clone());
        merged
    }

    /// Sorts the ranges and merges adjacent or overlapping ranges of the same target.
    fn merge(mut ranges: Vec<[i32; 3]>) -> Vec<[i32; 3]> {
        ranges.sort_unstable();
        let mut merged: Vec<[i32; 3]> = Vec::with_capacity(ranges.len());
        for [target, min, max] in ranges {
            match merged.last_mut() {
                Some(last) if last[0] == target && min <= last[2] + 1 => last[2] = last[2].max(max),
                _ => merged.push([target, min, max]),
            }
        }
        merged
    }

    /// Prefixes the suffix range with every leading value in `[min, max]`, shifted by `shift`.
    fn append(
        ranges: &mut Vec<[i32; 3]>,
        min: i32,
        max: i32,
        shift: i32,
        [target, suffix_min, suffix_max]: [i32; 3],
    ) {
        if suffix_min == 0 && suffix_max == (1 << shift) - 1 {
            ranges.push([target, min << shift, (max << shift) | suffix_max]);
        } else {
            for prefix in min..=max {
                ranges.push([target, (prefix << shift) | suffix_min, (prefix << shift) | suffix_max]);
            }
        }
    }
}
