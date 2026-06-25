//! A disjoint-set (union-find) structure — a port of `cpp/support/union_find_set.h`.
//!
//! Used by the FSM simplification passes to coalesce equivalent states. Only the elements
//! explicitly [`add`](UnionFindSet::add)ed participate; [`get_all_sets`](UnionFindSet::get_all_sets)
//! returns the partition in a deterministic order (each set ascending, sets ordered by their
//! smallest element) so downstream state renumbering is reproducible.

use std::{collections::HashMap, hash::Hash};

/// A union-find set over hashable, ordered elements with union-by-size and path compression.
#[derive(Debug, Clone, Default)]
pub struct UnionFindSet<T> {
    /// element → (parent, subtree size)
    parent_and_size: HashMap<T, (T, usize)>,
}

impl<T: Copy + Eq + Hash + Ord> UnionFindSet<T> {
    /// Creates an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent_and_size: HashMap::new(),
        }
    }

    /// Adds `element` as a new singleton set. Returns `false` if it already exists.
    pub fn add(
        &mut self,
        element: T,
    ) -> bool {
        if self.parent_and_size.contains_key(&element) {
            return false;
        }
        self.parent_and_size.insert(element, (element, 1));
        true
    }

    /// Removes all elements.
    pub fn clear(&mut self) {
        self.parent_and_size.clear();
    }

    /// The representative of the set containing `element` (with path compression).
    ///
    /// # Panics
    /// Panics if `element` was never added.
    pub fn find(
        &mut self,
        element: T,
    ) -> T {
        let parent = self
            .parent_and_size
            .get(&element)
            .expect("element not found in union-find set")
            .0;
        if parent != element {
            let root = self.find(parent);
            self.parent_and_size.get_mut(&element).expect("present").0 = root;
            root
        } else {
            element
        }
    }

    /// Merges the sets containing `a` and `b` (union by size).
    ///
    /// # Panics
    /// Panics if either element was never added.
    pub fn union(
        &mut self,
        a: T,
        b: T,
    ) {
        let mut root_a = self.find(a);
        let mut root_b = self.find(b);
        if root_a == root_b {
            return;
        }
        let size_a = self.parent_and_size[&root_a].1;
        let size_b = self.parent_and_size[&root_b].1;
        if size_a < size_b {
            std::mem::swap(&mut root_a, &mut root_b);
        }
        self.parent_and_size.get_mut(&root_b).expect("present").0 = root_a;
        self.parent_and_size.get_mut(&root_a).expect("present").1 =
            size_a + size_b;
    }

    /// Whether `element` has been added (`1`) or not (`0`) — mirrors the C++ `Count`.
    #[must_use]
    pub fn count(
        &self,
        element: T,
    ) -> bool {
        self.parent_and_size.contains_key(&element)
    }

    /// All sets, each sorted ascending, ordered by their smallest element.
    pub fn get_all_sets(&mut self) -> Vec<Vec<T>> {
        let elements: Vec<T> = self.parent_and_size.keys().copied().collect();
        let mut root_to_set: HashMap<T, usize> = HashMap::new();
        let mut result: Vec<Vec<T>> = Vec::new();
        for value in elements {
            let root = self.find(value);
            let idx = *root_to_set.entry(root).or_insert_with(|| {
                result.push(Vec::new());
                result.len() - 1
            });
            result[idx].push(value);
        }
        for vec in &mut result {
            vec.sort_unstable();
        }
        result.sort_unstable_by(|a, b| a[0].cmp(&b[0]));
        result
    }
}
