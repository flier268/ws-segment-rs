//! UTF-16 prefix tree of dictionary words. Hits are emitted shortest-first.

use crate::dict::DictEntry;
use std::collections::HashMap;

/// One dictionary match in the current span, as UTF-16 offsets into that span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DictHit {
    pub start: usize,
    pub len: usize,
    pub f: f64,
}

#[derive(Clone, Debug, Default)]
enum Children {
    #[default]
    None,
    One(u16, u32),
    Many(HashMap<u16, u32>),
}

impl Children {
    fn get(&self, unit: u16) -> Option<u32> {
        match self {
            Children::None => None,
            Children::One(k, v) if *k == unit => Some(*v),
            Children::One(_, _) => None,
            Children::Many(map) => map.get(&unit).copied(),
        }
    }

    fn insert(&mut self, unit: u16, node: u32) {
        match self {
            Children::None => *self = Children::One(unit, node),
            Children::One(k, old) => {
                if *k == unit {
                    *old = node;
                    return;
                }
                let mut map = HashMap::with_capacity(4);
                map.insert(*k, *old);
                map.insert(unit, node);
                *self = Children::Many(map);
            }
            Children::Many(map) => {
                map.insert(unit, node);
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct TrieNode {
    children: Children,
    end: Option<DictEntry>,
}

/// Prefix tree keyed by UTF-16 code units (JS `String` indexing).
#[derive(Clone, Debug)]
pub(crate) struct WordTrie {
    nodes: Vec<TrieNode>,
}

impl Default for WordTrie {
    fn default() -> Self {
        Self {
            nodes: vec![TrieNode::default()],
        }
    }
}

impl WordTrie {
    pub(crate) fn insert(&mut self, w: &str, entry: DictEntry) {
        let mut i = 0u32;
        for unit in w.encode_utf16() {
            let cur = i as usize;
            if let Some(next) = self.nodes[cur].children.get(unit) {
                i = next;
                continue;
            }
            let next = self.nodes.len() as u32;
            self.nodes[cur].children.insert(unit, next);
            self.nodes.push(TrieNode::default());
            i = next;
        }
        self.nodes[i as usize].end = Some(entry);
    }

    pub(crate) fn remove(&mut self, w: &str) {
        let mut i = 0u32;
        for unit in w.encode_utf16() {
            match self.nodes[i as usize].children.get(unit) {
                Some(next) => i = next,
                None => return,
            }
        }
        self.nodes[i as usize].end = None;
    }

    /// Dictionary hits starting at `start`, shortest first.
    pub(crate) fn matches_from<'a>(
        &'a self,
        units: &'a [u16],
        start: usize,
    ) -> MatchesFrom<'a> {
        MatchesFrom {
            trie: self,
            units,
            start,
            pos: start,
            node: 0,
        }
    }

    /// Longest word starting at `start` (greedy; last terminal on the walk).
    pub(crate) fn longest_from(&self, units: &[u16], start: usize) -> Option<usize> {
        let mut node = 0usize;
        let mut best = None;
        for pos in start..units.len() {
            match self.nodes[node].children.get(units[pos]) {
                Some(next) => node = next as usize,
                None => break,
            }
            if self.nodes[node].end.is_some() {
                best = Some(pos + 1 - start);
            }
        }
        best
    }
}

pub(crate) struct MatchesFrom<'a> {
    trie: &'a WordTrie,
    units: &'a [u16],
    start: usize,
    pos: usize,
    node: usize,
}

impl<'a> Iterator for MatchesFrom<'a> {
    type Item = (usize, DictEntry);

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.units.len() {
            let unit = self.units[self.pos];
            self.pos += 1;
            match self.trie.nodes[self.node].children.get(unit) {
                Some(next) => self.node = next as usize,
                None => {
                    self.pos = self.units.len();
                    return None;
                }
            }
            if let Some(entry) = self.trie.nodes[self.node].end {
                return Some((self.pos - self.start, entry));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(f: f64) -> DictEntry {
        DictEntry { p: 1, f, s: true }
    }

    #[test]
    fn matches_shortest_first() {
        let mut t = WordTrie::default();
        t.insert("中", entry(1.0));
        t.insert("中国", entry(2.0));
        t.insert("中国人", entry(3.0));
        let units: Vec<u16> = "中国人".encode_utf16().collect();
        let hits: Vec<(usize, f64)> = t
            .matches_from(&units, 0)
            .map(|(len, e)| (len, e.f))
            .collect();
        assert_eq!(hits, vec![(1, 1.0), (2, 2.0), (3, 3.0)]);
        assert_eq!(t.longest_from(&units, 0), Some(3));
    }

    #[test]
    fn miss_and_remove() {
        let mut t = WordTrie::default();
        t.insert("里面", entry(4.0));
        let units: Vec<u16> = "里头".encode_utf16().collect();
        assert_eq!(t.matches_from(&units, 0).count(), 0);
        let hit: Vec<u16> = "里面".encode_utf16().collect();
        assert_eq!(t.longest_from(&hit, 0), Some(2));
        t.remove("里面");
        assert_eq!(t.longest_from(&hit, 0), None);
    }

    #[test]
    fn overwrite_updates_frequency() {
        let mut t = WordTrie::default();
        t.insert("的", entry(1.0));
        t.insert("的", entry(9.0));
        let units: Vec<u16> = "的".encode_utf16().collect();
        let fs: Vec<f64> = t.matches_from(&units, 0).map(|(_, e)| e.f).collect();
        assert_eq!(fs, vec![9.0]);
    }
}
