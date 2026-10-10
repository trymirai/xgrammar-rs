//! A trie over property-name spellings, for key patterns that exclude the declared names.

use std::collections::BTreeMap;

/// A trie node over key units (bytes or codepoints): whether a key ends here, and the children
/// ordered by their next unit.
#[derive(Debug)]
pub struct KeyTrie<T: Ord> {
    pub is_terminal: bool,
    pub children: BTreeMap<T, KeyTrie<T>>,
}

impl<T: Ord> KeyTrie<T> {
    /// Builds the trie of `keys`, each given as its sequence of units.
    pub fn new<K: IntoIterator<Item = T>>(keys: impl IntoIterator<Item = K>) -> Self {
        let mut root = Self::leaf();
        for key in keys {
            let mut node = &mut root;
            for unit in key {
                node = node.children.entry(unit).or_insert_with(Self::leaf);
            }
            node.is_terminal = true;
        }
        root
    }

    fn leaf() -> Self {
        Self {
            is_terminal: false,
            children: BTreeMap::new(),
        }
    }
}
