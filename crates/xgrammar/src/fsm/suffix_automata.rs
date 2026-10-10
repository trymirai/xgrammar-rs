//! Suffix automaton construction for substring expressions.

use std::collections::BTreeMap;

use super::{fsm::Fsm, fsm_with_start_end::FsmWithStartEnd};

/// Builds the automaton of a substring expression via a chunk-level suffix automaton.
#[derive(Debug, Clone, Copy, Default)]
pub struct SuffixAutomata;

impl SuffixAutomata {
    /// Builds an FSM that accepts exactly the contiguous subsequences of `chunks`, including the
    /// empty one.
    ///
    /// A suffix automaton is built over the chunk sequence (each chunk is one symbol), so the
    /// number of automaton states grows linearly with the number of chunks, and every automaton
    /// state is accepting. Each chunk-labeled transition is then expanded into a chain of byte
    /// transitions; an empty chunk becomes an epsilon transition. Chunks may be empty or
    /// repeated.
    #[must_use]
    pub fn build<C: AsRef<[u8]>>(chunks: &[C]) -> FsmWithStartEnd {
        // Step 1. The standard online construction over the chunk sequence. State `i` has the
        // longest-string length `lengths[i]`, the suffix link `links[i]`, and the chunk-labeled
        // `transitions[i]`.
        let mut lengths: Vec<i32> = vec![0];
        let mut links: Vec<i32> = vec![-1];
        let mut transitions: Vec<BTreeMap<&[u8], i32>> = vec![BTreeMap::new()];
        let mut last = 0usize;
        for chunk in chunks {
            let chunk = chunk.as_ref();
            let current = lengths.len();
            lengths.push(lengths[last] + 1);
            links.push(-1);
            transitions.push(BTreeMap::new());

            let mut parent = last as i32;
            while parent != -1 && !transitions[parent as usize].contains_key(chunk) {
                transitions[parent as usize].insert(chunk, current as i32);
                parent = links[parent as usize];
            }
            if parent == -1 {
                links[current] = 0;
            } else {
                let target = transitions[parent as usize][chunk];
                if lengths[parent as usize] + 1 == lengths[target as usize] {
                    links[current] = target;
                } else {
                    let clone = lengths.len() as i32;
                    lengths.push(lengths[parent as usize] + 1);
                    links.push(links[target as usize]);
                    transitions.push(transitions[target as usize].clone());
                    while parent != -1 && transitions[parent as usize].get(chunk) == Some(&target) {
                        transitions[parent as usize].insert(chunk, clone);
                        parent = links[parent as usize];
                    }
                    links[target as usize] = clone;
                    links[current] = clone;
                }
            }
            last = current;
        }

        // Step 2. Expand into a byte-level FSM: automaton state `i` is FSM state `i`, and a
        // chunk-labeled transition becomes a chain of byte edges through fresh states.
        let num_states = lengths.len();
        let mut fsm = Fsm::new(num_states);
        for (index, state_transitions) in transitions.iter().enumerate() {
            for (&chunk, &target) in state_transitions {
                let Some((&last_byte, prefix)) = chunk.split_last() else {
                    fsm.add_epsilon_edge(index as i32, target);
                    continue;
                };
                let mut current_state = index as i32;
                for &byte in prefix {
                    let next_state = fsm.add_state();
                    fsm.add_edge(current_state, next_state, i32::from(byte), i32::from(byte));
                    current_state = next_state;
                }
                fsm.add_edge(current_state, target, i32::from(last_byte), i32::from(last_byte));
            }
        }
        FsmWithStartEnd::new(fsm, 0, (0..num_states as i32).collect(), false)
    }
}
