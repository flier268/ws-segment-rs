//! Tokenizer / optimizer traits and default module names.

use crate::segment::Segment;
use crate::word::Word;

pub const URL_TOKENIZER: &str = "URLTokenizer";
pub const WILDCARD_TOKENIZER: &str = "WildcardTokenizer";
pub const PUNCTUATION_TOKENIZER: &str = "PunctuationTokenizer";
pub const FOREIGN_TOKENIZER: &str = "ForeignTokenizer";
pub const DICT_TOKENIZER: &str = "DictTokenizer";
pub const CHS_NAME_TOKENIZER: &str = "ChsNameTokenizer";
pub const JP_SIMPLE_TOKENIZER: &str = "JpSimpleTokenizer";
pub const ZHUYIN_TOKENIZER: &str = "ZhuyinTokenizer";
pub const EMAIL_OPTIMIZER: &str = "EmailOptimizer";
pub const CHS_NAME_OPTIMIZER: &str = "ChsNameOptimizer";
pub const DICT_OPTIMIZER: &str = "DictOptimizer";
pub const DATETIME_OPTIMIZER: &str = "DatetimeOptimizer";
pub const FOREIGN_OPTIMIZER: &str = "ForeignOptimizer";
pub const ZHT_SYNONYM_OPTIMIZER: &str = "ZhtSynonymOptimizer";
pub const ADJECTIVE_OPTIMIZER: &str = "AdjectiveOptimizer";

pub fn default_tokenizer_names(all_mod: bool) -> Vec<&'static str> {
    let _ = all_mod;
    vec![
        URL_TOKENIZER,
        WILDCARD_TOKENIZER,
        PUNCTUATION_TOKENIZER,
        FOREIGN_TOKENIZER,
        DICT_TOKENIZER,
        CHS_NAME_TOKENIZER,
        JP_SIMPLE_TOKENIZER,
        ZHUYIN_TOKENIZER,
    ]
}

pub fn default_optimizer_names(all_mod: bool) -> Vec<&'static str> {
    let mut v = vec![
        EMAIL_OPTIMIZER,
        CHS_NAME_OPTIMIZER,
        DICT_OPTIMIZER,
        DATETIME_OPTIMIZER,
        FOREIGN_OPTIMIZER,
        ADJECTIVE_OPTIMIZER,
    ];
    if all_mod {
        // Match ENUM_SUBMODS: Zht sits before Adjective.
        v.insert(v.len() - 1, ZHT_SYNONYM_OPTIMIZER);
    }
    v
}

pub trait Tokenizer: Send + Sync {
    fn name(&self) -> &'static str;
    fn split(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word>;
}

pub trait Optimizer: Send + Sync {
    fn name(&self) -> &'static str;
    fn do_optimize(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word>;
}

pub fn enabled(name: &str, disabled: &[String]) -> bool {
    !disabled.iter().any(|d| d == name)
}

/// Apply `fn` only to unrecognized words (`p` missing or 0). Returns `None` to keep original.
pub fn split_unknown(words: Vec<Word>, mut f: impl FnMut(&str) -> Option<Vec<Word>>) -> Vec<Word> {
    let mut ret = Vec::new();
    for word in words {
        if word.is_recognized() {
            ret.push(word);
            continue;
        }
        match f(&word.w) {
            Some(parts) => ret.extend(parts),
            None => ret.push(word),
        }
    }
    ret
}

/// Apply `fn` only when `p` is unset (`typeof p !== 'number'`).
pub fn split_unset(words: Vec<Word>, mut f: impl FnMut(&str) -> Option<Vec<Word>>) -> Vec<Word> {
    let mut ret = Vec::new();
    for word in words {
        if word.has_pos() {
            ret.push(word);
            continue;
        }
        match f(&word.w) {
            Some(parts) => ret.extend(parts),
            None => ret.push(word),
        }
    }
    ret
}

pub fn splice_token(words: &mut Vec<Word>, i: usize, len: usize, nw: Word) {
    let end = (i + len).min(words.len());
    words.splice(i..end, std::iter::once(nw));
}

/// JS `sliceToken` / `createToken`: set `auto_create` when the merged word is not in TABLE.
pub fn splice_created(words: &mut Vec<Word>, i: usize, len: usize, mut nw: Word, seg: &Segment) {
    if !seg.table.table.contains_key(&nw.w) {
        nw.auto_create = true;
    }
    let end = (i + len).min(words.len());
    if nw.m.is_none() && i < end {
        nw.m = Some(words[i..end].to_vec());
    }
    splice_token(words, i, len, nw);
}
