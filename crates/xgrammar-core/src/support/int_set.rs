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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_merges_and_dedups() {
        let mut a = vec![1, 3, 5];
        intset_union(&mut a, &[2, 3, 6]);
        assert_eq!(a, vec![1, 2, 3, 5, 6]);
    }

    #[test]
    fn union_handles_smaller_rhs_minimum() {
        // The C++ back-merge gets this wrong; the correct union is [1, 3].
        let mut a = vec![3];
        intset_union(&mut a, &[1, 3]);
        assert_eq!(a, vec![1, 3]);
    }

    #[test]
    fn union_with_empty() {
        let mut a = vec![1, 2];
        intset_union(&mut a, &[]);
        assert_eq!(a, vec![1, 2]);
        let mut b = vec![];
        intset_union(&mut b, &[4, 5]);
        assert_eq!(b, vec![4, 5]);
    }

    #[test]
    fn intersection_basic() {
        let mut a = vec![1, 2, 3, 4];
        intset_intersection(&mut a, &[2, 4, 6]);
        assert_eq!(a, vec![2, 4]);
    }

    #[test]
    fn intersection_universal_is_rhs() {
        let mut a = vec![-1];
        intset_intersection(&mut a, &[3, 7, 9]);
        assert_eq!(a, vec![3, 7, 9]);
    }

    #[test]
    fn intersection_disjoint_is_empty() {
        let mut a = vec![1, 3, 5];
        intset_intersection(&mut a, &[2, 4, 6]);
        assert!(a.is_empty());
    }

    #[test]
    fn difference_basic() {
        let mut a = vec![1, 2, 3, 4, 5];
        intset_difference(&mut a, &[2, 4]);
        assert_eq!(a, vec![1, 3, 5]);
    }

    #[test]
    fn difference_removes_all() {
        let mut a = vec![1, 2, 3];
        intset_difference(&mut a, &[1, 2, 3, 4]);
        assert!(a.is_empty());
    }

    #[test]
    fn complement_excludes() {
        assert_eq!(intset_complement(5, &[1, 3]), vec![0, 2, 4]);
        assert_eq!(intset_complement(4, &[]), vec![0, 1, 2, 3]);
        assert_eq!(intset_complement(3, &[0, 1, 2]), Vec::<i32>::new());
    }
}
