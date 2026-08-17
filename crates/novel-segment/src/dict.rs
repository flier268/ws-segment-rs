//! Dictionary tables.

use crate::word::Word;
use std::collections::{BTreeMap, HashMap, HashSet};

/// One dictionary row.
#[derive(Clone, Debug, Default, PartialEq)]
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

/// Main word table + length buckets (`TABLE` / `TABLE2`).
#[derive(Clone, Debug, Default)]
pub struct TableDict {
    pub table: HashMap<String, DictEntry>,
    /// Length → word → entry. `BTreeMap` so iteration matches JS integer-key order.
    pub table2: BTreeMap<usize, HashMap<String, DictEntry>>,
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
        let len = crate::text::char_len(&w);
        let entry = DictEntry { p, f, s };
        self.table.insert(w.clone(), entry.clone());
        self.table2.entry(len).or_default().insert(w, entry);
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
            let len = crate::text::char_len(w);
            if let Some(bucket) = self.table2.get_mut(&len) {
                bucket.remove(w);
                if bucket.is_empty() {
                    self.table2.remove(&len);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        self.table.len()
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
