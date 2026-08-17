//! Public options.

/// Segmenter construction options.
///
/// `auto_cjk` defaults to `false`, matching JS `new Segment()`.
/// Tests and the novel CLI pass `autoCjk: true`.
#[derive(Clone, Debug, Default)]
pub struct SegmentOptions {
    /// Expand CJK variants when adding dictionary words.
    pub auto_cjk: bool,
    /// Also enable `ZhtSynonymOptimizer` (JS `all_mod`).
    pub all_mod: bool,
    /// Load extra node-novel synonym files.
    pub node_novel_mode: bool,
    /// Skip built-in tokenizers/optimizers.
    pub nomod: bool,
    /// Skip built-in dictionaries.
    pub nodict: bool,
    /// Default `do_segment` options.
    pub options_do_segment: DoSegmentOptions,
    /// DictTokenizer max chunk count (JS default 40).
    pub max_chunk_count: Option<usize>,
    /// DictTokenizer min chunk count (JS default 30).
    pub min_chunk_count: Option<usize>,
    /// Module names to disable.
    pub disable_modules: Vec<String>,
}

/// Per-call segmentation options.
///
/// `None` means inherit from `SegmentOptions.options_do_segment` then default `false`
/// (JS `Object.assign({}, defaults, optionsDoSegment, options)`).
#[derive(Clone, Debug, Default)]
pub struct DoSegmentOptions {
    pub simple: Option<bool>,
    pub strip_punctuation: Option<bool>,
    pub convert_synonym: Option<bool>,
    pub strip_stopword: Option<bool>,
    pub strip_space: Option<bool>,
    pub disable_modules: Vec<String>,
}

impl DoSegmentOptions {
    pub fn convert_synonym() -> Self {
        Self {
            convert_synonym: Some(true),
            ..Default::default()
        }
    }

    pub fn merge(&self, over: &Self) -> Self {
        Self {
            simple: over.simple.or(self.simple),
            strip_punctuation: over.strip_punctuation.or(self.strip_punctuation),
            convert_synonym: over.convert_synonym.or(self.convert_synonym),
            strip_stopword: over.strip_stopword.or(self.strip_stopword),
            strip_space: over.strip_space.or(self.strip_space),
            disable_modules: if over.disable_modules.is_empty() {
                self.disable_modules.clone()
            } else {
                over.disable_modules.clone()
            },
        }
    }

    pub fn simple_flag(&self) -> bool {
        self.simple.unwrap_or(false)
    }
    pub fn strip_punctuation_flag(&self) -> bool {
        self.strip_punctuation.unwrap_or(false)
    }
    pub fn convert_synonym_flag(&self) -> bool {
        self.convert_synonym.unwrap_or(false)
    }
    pub fn strip_stopword_flag(&self) -> bool {
        self.strip_stopword.unwrap_or(false)
    }
    pub fn strip_space_flag(&self) -> bool {
        self.strip_space.unwrap_or(false)
    }
}
