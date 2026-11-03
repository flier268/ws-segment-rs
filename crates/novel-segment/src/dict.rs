//! Dictionary tables.

use crate::trie::{DictHit, WordTrie};
use crate::word::Word;
use std::collections::{HashMap, HashSet};

/// One dictionary row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DictEntry {
    pub p: u32,
    pub f: f64,
    pub s: bool,
}

impl From<&DictEntry> for Word {
    fn from(e: &DictEntry) -> Self {
        Word {
            p: Some(e.p),
            f: Some(e.f),
            s: Some(e.s),
            ..Default::default()
        }
    }
}

/// Main word table (`TABLE`) plus a UTF-16 trie for span matching.
#[derive(Clone, Debug, Default)]
pub struct TableDict {
    pub table: HashMap<String, DictEntry>,
    pub(crate) trie: WordTrie,
}

impl TableDict {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn exists(&self, w: &str) -> Option<&DictEntry> {
        self.table.get(w)
    }

    pub fn add(&mut self, w: impl Into<String>, p: u32, f: f64, s: bool) {
        let w = w.into();
        if w.is_empty() {
            return;
        }
        let entry = DictEntry { p, f, s };
        self.trie.insert(&w, entry);
        self.table.insert(w, entry);
    }

    pub fn add_with_cjk(&mut self, w: &str, p: u32, f: f64, auto_cjk: bool) {
        self.add(w, p, f, true);
        if !auto_cjk {
            return;
        }
        #[cfg(feature = "default-dict")]
        {
            for v in novel_segment_dict::expand_cjk_variants(w) {
                if v != w && !self.table.contains_key(&v) {
                    self.add(v, p, f, false);
                }
            }
        }
        let _ = auto_cjk;
    }

    pub fn remove(&mut self, w: &str) {
        if self.table.remove(w).is_some() {
            self.trie.remove(w);
        }
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.table.len()
    }

    pub(crate) fn hits_from(&self, units: &[u16], start: usize) -> Vec<DictHit> {
        self.trie
            .matches_from(units, start)
            .map(|(len, e)| DictHit {
                start,
                len,
                f: e.f,
            })
            .collect()
    }

    pub(crate) fn longest_from(&self, units: &[u16], start: usize) -> Option<usize> {
        self.trie.longest_from(units, start)
    }
}

/// One-to-many synonym map: variant → canonical.
#[derive(Clone, Debug, Default)]
pub struct SynonymDict {
    /// variant → canonical
    pub table: HashMap<String, String>,
    /// canonical → variants
    pub table2: HashMap<String, Vec<String>>,
}

impl SynonymDict {
    pub fn add(&mut self, canonical: &str, variants: &[String], skip_exists: bool) {
        let canonical = canonical.trim();
        if canonical.is_empty() {
            return;
        }
        self.table2.entry(canonical.to_string()).or_default();
        for v in variants {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            // JS: (!forceOverwrite) && (skipExists && exists(bw) || bw in TABLE2)
            if (skip_exists && self.table.contains_key(v)) || self.table2.contains_key(v) {
                continue;
            }
            self.table2
                .entry(canonical.to_string())
                .or_default()
                .push(v.to_string());
            self.table.insert(v.to_string(), canonical.to_string());
            if self.table.get(canonical).map(|s| s == v).unwrap_or(false) {
                self.table.remove(canonical);
            }
        }
    }

    pub fn get(&self, w: &str) -> Option<&str> {
        self.table.get(w).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synonym_skip_canonical_in_table2() {
        let mut d = SynonymDict::default();
        d.add("正", &["甲".into()], false);
        d.add("另", &["正".into()], false);
        assert_eq!(d.get("正"), None);
        assert_eq!(d.get("甲"), Some("正"));
    }

    #[test]
    fn table_trie_hits_match_table() {
        let mut t = TableDict::new();
        t.add("中", 1, 1.0, true);
        t.add("中国", 1, 2.0, true);
        let units: Vec<u16> = "中国人".encode_utf16().collect();
        let hits: Vec<(usize, usize, f64)> = t
            .hits_from(&units, 0)
            .into_iter()
            .map(|h| (h.start, h.len, h.f))
            .collect();
        assert_eq!(hits, vec![(0, 1, 1.0), (0, 2, 2.0)]);
        t.remove("中国");
        let hits: Vec<usize> = t.hits_from(&units, 0).into_iter().map(|h| h.len).collect();
        assert_eq!(hits, vec![1]);
    }
}

/// Set-like dictionary (stopword / blacklist).
#[derive(Clone, Debug, Default)]
pub struct SetDict {
    pub table: HashSet<String>,
}

impl SetDict {
    pub fn add(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !w.is_empty() {
            self.table.insert(w);
        }
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, w: &str) {
        self.table.remove(w);
    }

    pub fn contains(&self, w: &str) -> bool {
        self.table.contains(w)
    }
}
