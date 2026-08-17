//! Segmenter.

use crate::dict::{SetDict, SynonymDict, TableDict};
use crate::error::{Error, Result};
use crate::loader::{
    load_dict_file, load_line_file, load_synonym_file, parse_dict_line, resolve_named,
};
use crate::optimizers::builtin_optimizers;
use crate::options::{DoSegmentOptions, SegmentOptions};
use crate::pipeline::{
    default_optimizer_names, default_tokenizer_names, enabled, Optimizer, Tokenizer,
};
use crate::postag::POSTAG;
use crate::text::{is_newline_only, split_sections};
use crate::tokenizers::{
    builtin_tokenizers, DEFAULT_MAX_CHUNK_COUNT, DEFAULT_MAX_CHUNK_COUNT_MIN,
};
use crate::word::{stringify_list, Word};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Chinese word segmenter.
pub struct Segment {
    pub(crate) options: SegmentOptions,
    pub(crate) table: TableDict,
    pub(crate) wildcard: TableDict,
    pub(crate) synonym: SynonymDict,
    pub(crate) stopword: SetDict,
    pub(crate) blacklist: SetDict,
    pub(crate) blacklist_optimizer: SetDict,
    pub(crate) blacklist_synonym: SetDict,
    pub(crate) tokenizers: Vec<Box<dyn Tokenizer>>,
    pub(crate) optimizers: Vec<Box<dyn Optimizer>>,
    pub(crate) max_chunk_count: usize,
    pub(crate) min_chunk_count: usize,
    pub(crate) inited: bool,
}

impl Default for Segment {
    fn default() -> Self {
        Self::new(SegmentOptions::default())
    }
}

impl Segment {
    pub fn new(options: SegmentOptions) -> Self {
        let max_chunk_count = options
            .max_chunk_count
            .filter(|n| *n > DEFAULT_MAX_CHUNK_COUNT_MIN)
            .unwrap_or(DEFAULT_MAX_CHUNK_COUNT);
        let min_chunk_count = options
            .min_chunk_count
            .filter(|n| *n > DEFAULT_MAX_CHUNK_COUNT_MIN)
            .unwrap_or(DEFAULT_MAX_CHUNK_COUNT_MIN);
        Self {
            options,
            table: TableDict::new(),
            wildcard: TableDict::new(),
            synonym: SynonymDict::default(),
            stopword: SetDict::default(),
            blacklist: SetDict::default(),
            blacklist_optimizer: SetDict::default(),
            blacklist_synonym: SetDict::default(),
            tokenizers: Vec::new(),
            optimizers: Vec::new(),
            max_chunk_count,
            min_chunk_count,
            inited: false,
        }
    }

    /// `new Segment(); useDefault()` — `auto_cjk` stays false.
    pub fn with_default() -> Result<Self> {
        let mut seg = Self::new(SegmentOptions::default());
        seg.use_default()?;
        Ok(seg)
    }

    /// Test/CLI profile: `autoCjk`, `all_mod`, `convertSynonym`.
    pub fn with_novel_default() -> Result<Self> {
        let mut seg = Self::new(SegmentOptions {
            auto_cjk: true,
            all_mod: true,
            options_do_segment: DoSegmentOptions::convert_synonym(),
            ..Default::default()
        });
        seg.use_default()?;
        Ok(seg)
    }

    /// `word.novel.test.ts`: createSegment then wipe SYNONYM and reload with `nodeNovelMode`.
    pub fn with_node_novel_default() -> Result<Self> {
        let mut seg = Self::new(SegmentOptions {
            auto_cjk: true,
            all_mod: true,
            node_novel_mode: true,
            options_do_segment: DoSegmentOptions::convert_synonym(),
            ..Default::default()
        });
        seg.use_default()?;
        seg.clear_synonym_dict();
        seg.use_default_synonym_dict(true)?;
        Ok(seg)
    }

    pub fn use_default(&mut self) -> Result<&mut Self> {
        if !self.options.nomod {
            self.use_default_modules();
        }
        if !self.options.nodict {
            self.use_default_dicts()?;
        }
        self.inited = true;
        Ok(self)
    }

    pub fn use_default_modules(&mut self) -> &mut Self {
        let tok_names = default_tokenizer_names(self.options.all_mod);
        let opt_names = default_optimizer_names(self.options.all_mod);
        let disabled = &self.options.disable_modules;
        self.tokenizers = builtin_tokenizers()
            .into_iter()
            .filter(|t| tok_names.contains(&t.name()) && enabled(t.name(), disabled))
            .collect();
        self.optimizers = builtin_optimizers()
            .into_iter()
            .filter(|t| opt_names.contains(&t.name()) && enabled(t.name(), disabled))
            .collect();
        self
    }

    pub fn use_tokenizer(&mut self, tok: Box<dyn Tokenizer>) -> &mut Self {
        self.tokenizers.push(tok);
        self.inited = true;
        self
    }

    pub fn use_optimizer(&mut self, opt: Box<dyn Optimizer>) -> &mut Self {
        self.optimizers.push(opt);
        self.inited = true;
        self
    }

    #[cfg(feature = "default-dict")]
    fn segment_root(&self) -> PathBuf {
        novel_segment_dict::segment_dict_root()
    }

    #[cfg(not(feature = "default-dict"))]
    fn segment_root(&self) -> PathBuf {
        PathBuf::from("dict/segment")
    }

    pub fn use_default_dicts(&mut self) -> Result<&mut Self> {
        #[cfg(feature = "default-dict")]
        {
            if let Some(cached) = lookup_cache(&self.options) {
                self.apply_cached_dicts(&cached);
                return Ok(self);
            }
        }
        self.load_dict("char")?;
        self.load_dict("pangu/phrases")?;
        self.load_dict("pangu/phrases2")?;
        self.load_dict("phrases/*")?;
        self.load_dict("dict")?;
        self.load_dict("dict2")?;
        self.load_dict("dict3")?;
        self.load_dict("dict4")?;
        self.load_dict("pangu/dict005")?;
        self.load_dict("pangu/dict006")?;
        self.load_dict("dict_synonym/*")?;
        self.load_stopword_dict("stopword")?;
        self.load_dict("lazy/dict_synonym")?;
        self.load_dict("names/*")?;
        self.load_dict("lazy/*")?;
        self.load_dict("pangu/num")?;
        self.load_dict("lazy/badword")?;
        self.load_dict_as("pangu/wildcard", DictKind::Wildcard, true)?;
        self.load_synonym_dict("synonym", true)?;
        self.load_synonym_dict("zht.synonym", false)?;
        if self.options.node_novel_mode {
            self.load_synonym_dict("badword.synonym", false)?;
            self.load_synonym_dict("zht.common.synonym", false)?;
        }
        self.load_blacklist_dict("blacklist")?;
        self.load_blacklist_optimizer_dict("blacklist.name")?;
        self.load_blacklist_synonym_dict("blacklist.synonym")?;
        self.do_blacklist();
        #[cfg(feature = "default-dict")]
        {
            store_cache(self);
        }
        Ok(self)
    }

    pub fn load_dict(&mut self, name: &str) -> Result<&mut Self> {
        self.load_dict_as(name, DictKind::Table, false)
    }

    pub fn load_dict_file(&mut self, path: impl AsRef<Path>) -> Result<&mut Self> {
        let path = path.as_ref();
        let rows = load_dict_file(path)?;
        for row in rows {
            self.table.add_with_cjk(&row.w, row.p, row.f, self.options.auto_cjk);
        }
        self.inited = true;
        Ok(self)
    }

    fn load_dict_as(&mut self, name: &str, kind: DictKind, lower: bool) -> Result<&mut Self> {
        let files = resolve_named(&self.segment_root(), name)?;
        for file in files {
            let rows = load_dict_file(&file)?;
            for mut row in rows {
                if lower {
                    row.w = row.w.to_lowercase();
                }
                match kind {
                    DictKind::Table => {
                        self.table.add_with_cjk(&row.w, row.p, row.f, self.options.auto_cjk);
                    }
                    DictKind::Wildcard => {
                        self.wildcard.add(row.w, row.p, row.f, true);
                    }
                }
            }
        }
        self.inited = true;
        Ok(self)
    }

    pub fn load_synonym_dict(&mut self, name: &str, skip_exists: bool) -> Result<&mut Self> {
        #[cfg(feature = "default-dict")]
        let root = novel_segment_dict::synonym_dict_root();
        #[cfg(not(feature = "default-dict"))]
        let root = PathBuf::from("dict/synonym");
        let files = match resolve_named(&root, name) {
            Ok(f) => f,
            Err(_) => resolve_named(&self.segment_root(), name)?,
        };
        for file in files {
            for (canon, vars) in load_synonym_file(&file)? {
                self.synonym.add(&canon, &vars, skip_exists);
                // Phrase synonyms must be tokenizable (word.novel: 恐怖分子 → 恐怖份子).
                if let Some(e) = self.table.table.get(&canon).cloned() {
                    for v in &vars {
                        if !self.table.table.contains_key(v) {
                            self.table.add(v, e.p, e.f, false);
                        }
                    }
                }
            }
        }
        self.inited = true;
        Ok(self)
    }

    pub fn clear_synonym_dict(&mut self) -> &mut Self {
        self.synonym = SynonymDict::default();
        self
    }

    /// JS `useDefaultSynonymDict`.
    pub fn use_default_synonym_dict(&mut self, node_novel_mode: bool) -> Result<&mut Self> {
        self.load_synonym_dict("synonym", true)?;
        self.load_synonym_dict("zht.synonym", false)?;
        if node_novel_mode {
            self.load_synonym_dict("badword.synonym", false)?;
            self.load_synonym_dict("zht.common.synonym", false)?;
        }
        Ok(self)
    }

    pub fn load_stopword_dict(&mut self, name: &str) -> Result<&mut Self> {
        #[cfg(feature = "default-dict")]
        let root = novel_segment_dict::stopword_dict_root();
        #[cfg(not(feature = "default-dict"))]
        let root = PathBuf::from("dict/stopword");
        let files = match resolve_named(&root, name) {
            Ok(f) => f,
            Err(_) => resolve_named(&self.segment_root(), name)?,
        };
        for file in files {
            for line in load_line_file(&file)? {
                self.stopword.add(line);
            }
        }
        Ok(self)
    }

    pub fn load_blacklist_dict(&mut self, name: &str) -> Result<&mut Self> {
        self.load_set_dict(name, BlackKind::Main)
    }

    pub fn load_blacklist_optimizer_dict(&mut self, name: &str) -> Result<&mut Self> {
        self.load_set_dict(name, BlackKind::Optimizer)
    }

    pub fn load_blacklist_synonym_dict(&mut self, name: &str) -> Result<&mut Self> {
        self.load_set_dict(name, BlackKind::Synonym)
    }

    fn load_set_dict(&mut self, name: &str, kind: BlackKind) -> Result<&mut Self> {
        #[cfg(feature = "default-dict")]
        let root = novel_segment_dict::blacklist_dict_root();
        #[cfg(not(feature = "default-dict"))]
        let root = PathBuf::from("dict/blacklist");
        let files = match resolve_named(&root, name) {
            Ok(f) => f,
            Err(_) => resolve_named(&self.segment_root(), name)?,
        };
        for file in files {
            for line in load_line_file(&file)? {
                match kind {
                    BlackKind::Main => self.blacklist.add(line),
                    BlackKind::Optimizer => self.blacklist_optimizer.add(line),
                    BlackKind::Synonym => self.blacklist_synonym.add(line),
                }
            }
        }
        Ok(self)
    }

    pub fn do_blacklist(&mut self) -> &mut Self {
        let keys: Vec<String> = self.blacklist.table.iter().cloned().collect();
        for k in keys {
            self.table.remove(&k);
        }
        self
    }

    /// Remove a word from TABLE (JS `addBlacklist`).
    pub fn add_blacklist(&mut self, word: &str) -> &mut Self {
        if !word.is_empty() {
            self.blacklist.add(word.to_string());
            self.table.remove(word);
        }
        self
    }

    /// Add a dictionary entry (`詞|詞性|詞權值` or just the word).
    pub fn add_word(&mut self, spec: &str, p: Option<u32>, f: Option<f64>) -> Result<&mut Self> {
        if let Some(row) = parse_dict_line(spec) {
            self.table
                .add_with_cjk(&row.w, row.p, row.f, self.options.auto_cjk);
            return Ok(self);
        }
        if spec.trim().is_empty() {
            return Err(Error::InvalidInput(spec.to_string()));
        }
        self.table.add_with_cjk(
            spec.trim(),
            p.unwrap_or(0),
            f.unwrap_or(0.0),
            self.options.auto_cjk,
        );
        Ok(self)
    }

    pub fn add_synonym(&mut self, canonical: &str, variants: &[&str]) -> &mut Self {
        let vars: Vec<String> = variants.iter().map(|s| (*s).to_string()).collect();
        self.synonym.add(canonical, &vars, false);
        if let Some(e) = self.table.table.get(canonical).cloned() {
            for v in &vars {
                if !self.table.table.contains_key(v) {
                    self.table.add(v, e.p, e.f, false);
                }
            }
        }
        self
    }

    pub fn do_segment(&self, text: &str, options: DoSegmentOptions) -> Vec<Word> {
        let options = self.options.options_do_segment.merge(&options);
        let disabled = if options.disable_modules.is_empty() {
            &self.options.disable_modules
        } else {
            &options.disable_modules
        };
        let toks: Vec<&dyn Tokenizer> = self
            .tokenizers
            .iter()
            .filter(|t| enabled(t.name(), disabled))
            .map(|t| t.as_ref())
            .collect();
        let opts: Vec<&dyn Optimizer> = self
            .optimizers
            .iter()
            .filter(|t| enabled(t.name(), disabled))
            .map(|t| t.as_ref())
            .collect();

        let mut ret = Vec::new();
        for section in split_sections(text) {
            if is_newline_only(&section) {
                ret.push(Word::new(section));
                continue;
            }
            if section.is_empty() {
                continue;
            }
            let mut words = vec![Word::new(section)];
            for t in &toks {
                words = t.split(words, self);
            }
            for o in &opts {
                words = o.do_optimize(words, self);
            }
            ret.extend(words);
        }

        if options.strip_punctuation_flag() {
            ret.retain(|w| w.pos() != POSTAG::D_W);
        }
        if options.convert_synonym_flag() {
            ret = convert_synonym(ret, self);
        }
        if options.strip_stopword_flag() {
            ret.retain(|w| !self.stopword.contains(&w.w));
        }
        if options.strip_space_flag() {
            ret.retain(|w| !w.w.chars().all(char::is_whitespace));
        }
        ret
    }

    pub fn do_segment_simple(&self, text: &str, mut options: DoSegmentOptions) -> Vec<String> {
        options.simple = Some(true);
        stringify_list(&self.do_segment(text, options))
    }

    fn apply_cached_dicts(&mut self, cached: &CachedDicts) {
        self.table = cached.table.clone();
        self.wildcard = cached.wildcard.clone();
        self.synonym = cached.synonym.clone();
        self.stopword = cached.stopword.clone();
        self.blacklist = cached.blacklist.clone();
        self.blacklist_optimizer = cached.blacklist_optimizer.clone();
        self.blacklist_synonym = cached.blacklist_synonym.clone();
        self.inited = true;
    }
}

#[derive(Clone, Copy)]
enum DictKind {
    Table,
    Wildcard,
}

enum BlackKind {
    Main,
    Optimizer,
    Synonym,
}

fn convert_synonym(mut words: Vec<Word>, seg: &Segment) -> Vec<Word> {
    loop {
        let mut count = 0;
        let mut next = Vec::with_capacity(words.len());
        for item in words {
            let w = item.w.clone();
            let mut nw: Option<String> = None;
            if let Some(s) = seg.synonym.get(&w) {
                nw = Some(s.to_string());
            } else if item.auto_create && item.ow.is_none() {
                if let Some(parts) = &item.m {
                    if !parts.is_empty() {
                        let mut joined = String::new();
                        let mut hit = false;
                        for b in parts {
                            if let Some(s) = seg.synonym.get(&b.w) {
                                joined.push_str(s);
                                hit = true;
                            } else {
                                joined.push_str(&b.w);
                            }
                        }
                        if hit {
                            nw = Some(joined);
                        }
                    }
                }
            }
            if let Some(nw) = nw {
                count += 1;
                let mut p = item.pos();
                if let Some(e) = seg.table.table.get(&w) {
                    if e.p != 0 {
                        p = e.p;
                    }
                }
                if p & POSTAG::BAD != 0 {
                    p ^= POSTAG::BAD;
                }
                next.push(Word {
                    ow: Some(w),
                    op: item.p,
                    w: nw,
                    p: Some(p),
                    ..item
                });
            } else {
                next.push(item);
            }
        }
        words = next;
        if count == 0 {
            break;
        }
    }
    words
}

#[derive(Clone)]
struct CachedDicts {
    table: TableDict,
    wildcard: TableDict,
    synonym: SynonymDict,
    stopword: SetDict,
    blacklist: SetDict,
    blacklist_optimizer: SetDict,
    blacklist_synonym: SetDict,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    auto_cjk: bool,
    node_novel_mode: bool,
    dict_root: String,
}

static DICT_CACHES: OnceLock<Mutex<HashMap<CacheKey, CachedDicts>>> = OnceLock::new();

fn cache_key(options: &SegmentOptions) -> CacheKey {
    let dict_root = {
        #[cfg(feature = "default-dict")]
        {
            novel_segment_dict::dict_root().display().to_string()
        }
        #[cfg(not(feature = "default-dict"))]
        {
            String::from("dict")
        }
    };
    CacheKey {
        auto_cjk: options.auto_cjk,
        node_novel_mode: options.node_novel_mode,
        dict_root,
    }
}

#[cfg(feature = "default-dict")]
fn lookup_cache(options: &SegmentOptions) -> Option<CachedDicts> {
    let map = DICT_CACHES.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = map.lock().ok()?;
    guard.get(&cache_key(options)).cloned()
}

#[cfg(feature = "default-dict")]
fn store_cache(seg: &Segment) {
    let map = DICT_CACHES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(
            cache_key(&seg.options),
            CachedDicts {
                table: seg.table.clone(),
                wildcard: seg.wildcard.clone(),
                synonym: seg.synonym.clone(),
                stopword: seg.stopword.clone(),
                blacklist: seg.blacklist.clone(),
                blacklist_optimizer: seg.blacklist_optimizer.clone(),
                blacklist_synonym: seg.blacklist_synonym.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_compound_parts() {
        let mut seg = Segment::new(SegmentOptions::default());
        seg.add_synonym("標準", &["錯字"]);
        let mut item = Word::new("錯字甲");
        item.auto_create = true;
        item.m = Some(vec![Word::new("錯字"), Word::new("甲")]);
        let out = convert_synonym(vec![item], &seg);
        assert_eq!(out[0].w, "標準甲");
        assert_eq!(out[0].ow.as_deref(), Some("錯字甲"));
    }
}
