//! Sorted integer-set operations — a port of `cpp/support/int_set.h`.
//!
//! All inputs are assumed to be **sorted and duplicate-free**. Intersection and
//! difference mutate the left operand in place (reusing its allocation); union rebuilds
//! it via an O(n+m) merge.
//!
//! These power the adaptive token-mask computation, so correctness is paramount — the
//! Rust port computes true set results (the C++ `IntsetUnion` back-merge mishandles some
//! inputs, e.g. `[3] ∪ [1, 3]`).

use std::cmp::Ordering;

/// Replaces `lhs` with the sorted union `lhs ∪ rhs`.
pub fn intset_union(lhs: &mut Vec<i32>, rhs: &[i32]) {
    let mut merged = Vec::with_capacity(lhs.len() + rhs.len());
    let (mut i, mut j) = (0, 0);
    while i < lhs.len() && j < rhs.len() {
        match lhs[i].cmp(&rhs[j]) {
            Ordering::Less => {
                merged.push(lhs[i]);
                i += 1;
            }
            Ordering::Greater => {
                merged.push(rhs[j]);
                j += 1;
            }
            Ordering::Equal => {
                merged.push(lhs[i]);
                i += 1;
                j += 1;
            }
        }
    }
    merged.extend_from_slice(&lhs[i..]);
    merged.extend_from_slice(&rhs[j..]);
    *lhs = merged;
}

/// Replaces `lhs` with the sorted intersection `lhs ∩ rhs`, in place.
///
/// As a special case, `lhs == [-1]` denotes the universal set, so the result is `rhs`.
pub fn intset_intersection(lhs: &mut Vec<i32>, rhs: &[i32]) {
    if lhs.as_slice() == [-1] {
        lhs.clear();
        lhs.extend_from_slice(rhs);
        return;
    }
    let mut write = 0;
    let (mut i, mut j) = (0, 0);
    while i < lhs.len() && j < rhs.len() {
        match lhs[i].cmp(&rhs[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                lhs[write] = lhs[i];
                write += 1;
                i += 1;
                j += 1;
            }
        }
    }
    lhs.truncate(write);
}

/// Replaces `lhs` with the sorted difference `lhs − rhs`, in place.
pub fn intset_difference(lhs: &mut Vec<i32>, rhs: &[i32]) {
    let mut write = 0;
    let (mut i, mut j) = (0, 0);
    while i < lhs.len() && j < rhs.len() {
        match lhs[i].cmp(&rhs[j]) {
            Ordering::Less => {
                lhs[write] = lhs[i];
                write += 1;
                i += 1;
            }
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    while i < lhs.len() {
        lhs[write] = lhs[i];
        write += 1;
        i += 1;
    }
    lhs.truncate(write);
}

/// Returns `[0, n) − excluded`. `excluded` must be sorted with values in `[0, n)`.
#[must_use]
pub fn intset_complement(n: i32, excluded: &[i32]) -> Vec<i32> {
    let mut result = Vec::with_capacity((n as usize).saturating_sub(excluded.len()));
    let mut it = excluded.iter().peekable();
    for i in 0..n {
        if it.peek() == Some(&&i) {
            it.next();
        } else {
            result.push(i);
        }
    }
    result
}
