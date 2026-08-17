//! Word token (`IWord` equivalent).

/// A segmented word. Field names match the JavaScript `IWord` object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Word {
    /// Word text.
    pub w: String,
    /// POS bit flags (`POSTAG`).
    pub p: Option<u32>,
    /// Frequency / weight.
    pub f: Option<f64>,
    /// Start index in the current section (scalar characters).
    pub c: Option<usize>,
    /// Native dictionary entry.
    pub s: Option<bool>,
    /// Original word before synonym conversion.
    pub ow: Option<String>,
    /// Original POS before conversion / retag.
    pub op: Option<u32>,
    /// Merged source tokens (JS `m`).
    pub m: Option<Vec<Word>>,
    /// Created by an optimizer and not in TABLE (JS debug `autoCreate`).
    pub auto_create: bool,
    /// Previous native-dict flag.
    pub os: Option<bool>,
}

impl Word {
    pub fn new(w: impl Into<String>) -> Self {
        Self {
            w: w.into(),
            ..Default::default()
        }
    }

    pub fn with_p(mut self, p: u32) -> Self {
        self.p = Some(p);
        self
    }

    pub fn with_f(mut self, f: f64) -> Self {
        self.f = Some(f);
        self
    }

    pub fn with_c(mut self, c: usize) -> Self {
        self.c = Some(c);
        self
    }

    /// JS `word.p > 0`.
    pub fn is_recognized(&self) -> bool {
        self.p.unwrap_or(0) > 0
    }

    /// JS `typeof word.p === 'number'`.
    pub fn has_pos(&self) -> bool {
        self.p.is_some()
    }

    pub fn pos(&self) -> u32 {
        self.p.unwrap_or(0)
    }

    pub fn freq(&self) -> f64 {
        self.f.unwrap_or(0.0)
    }

    /// JS `!word.p` — missing or zero POS.
    pub fn pos_falsy(&self) -> bool {
        self.p.unwrap_or(0) == 0
    }
}

/// Join words back into the original text.
pub fn stringify(words: &[Word]) -> String {
    words.iter().map(|w| w.w.as_str()).collect()
}

/// Join words or strings (for simple mode results stored as words).
pub fn stringify_list(words: &[Word]) -> Vec<String> {
    words.iter().map(|w| w.w.clone()).collect()
}
