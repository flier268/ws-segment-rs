//! Built-in tokenizers.

use crate::dict::DictEntry;
use crate::names::{DOUBLE_NAME_1, DOUBLE_NAME_2, FAMILY_NAME_1, FAMILY_NAME_2, SINGLE_NAME};
use crate::pipeline::{
    split_unknown, split_unset, Tokenizer, CHS_NAME_TOKENIZER, DICT_TOKENIZER, FOREIGN_TOKENIZER,
    JP_SIMPLE_TOKENIZER, PUNCTUATION_TOKENIZER, URL_TOKENIZER, WILDCARD_TOKENIZER,
    ZHUYIN_TOKENIZER,
};
use crate::postag::POSTAG;
use crate::segment::Segment;
use crate::stopword::STOPWORD2;
use crate::text::{
    char_at, char_len, char_slice, char_substr_from, leading_repeat_run, slice_chars, slice_utf16,
};
use crate::word::Word;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

pub fn builtin_tokenizers() -> Vec<Box<dyn Tokenizer>> {
    vec![
        Box::new(UrlTokenizer),
        Box::new(WildcardTokenizer),
        Box::new(PunctuationTokenizer),
        Box::new(ForeignTokenizer),
        Box::new(DictTokenizer),
        Box::new(ChsNameTokenizer),
        Box::new(JpSimpleTokenizer),
        Box::new(ZhuyinTokenizer),
    ]
}

// ---------------------------------------------------------------------------
// URL
// ---------------------------------------------------------------------------

const PROTOCOLS: &[&str] = &["http://", "https://", "ftp://", "news://", "telnet://"];

static PROTOCOL_UNITS: Lazy<Vec<Vec<char>>> =
    Lazy::new(|| PROTOCOLS.iter().map(|p| p.chars().collect()).collect());

static URL_CHARS: Lazy<HashSet<char>> = Lazy::new(|| {
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&‘()*+,-./:;=?@[\\]^_`|~"
        .chars()
        .collect()
});

pub struct UrlTokenizer;

impl Tokenizer for UrlTokenizer {
    fn name(&self) -> &'static str {
        URL_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        split_unknown(words, |text| {
            let hits = match_url(text);
            if hits.is_empty() {
                return None;
            }
            Some(interleave_hits(text, &hits, |w| {
                Word::new(w).with_p(POSTAG::URL)
            }))
        })
    }
}

fn match_url(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let min_proto = PROTOCOL_UNITS.iter().map(|p| p.len()).min().unwrap_or(0);
    let mut ret = Vec::new();
    let mut s: Option<usize> = None;
    let mut cur = 0usize;
    while cur < n {
        // JS: `cur < text.length - MIN_PROTOTAL_LEN` (remaining must exceed shortest protocol).
        if s.is_none() && cur + min_proto < n {
            for prot in PROTOCOL_UNITS.iter() {
                let end = cur + prot.len();
                if end <= n && chars[cur..end] == prot[..] {
                    s = Some(cur);
                    // JS loop increments after the last protocol char.
                    cur = end.saturating_sub(1);
                    break;
                }
            }
        } else if let Some(start) = s {
            if !URL_CHARS.contains(&chars[cur]) {
                ret.push((start, chars[start..cur].iter().collect()));
                s = None;
            }
        }
        cur += 1;
    }
    if let Some(start) = s {
        ret.push((start, chars[start..].iter().collect()));
    }
    ret
}

fn interleave_hits(
    text: &str,
    hits: &[(usize, String)],
    mut tagged: impl FnMut(&str) -> Word,
) -> Vec<Word> {
    let mut ret = Vec::new();
    let mut lastc = 0usize;
    for (c, w) in hits {
        if *c > lastc {
            ret.push(Word::new(char_slice(text, lastc, c - lastc)));
        }
        ret.push(tagged(w));
        lastc = c + char_len(w);
    }
    if let Some((c, w)) = hits.last() {
        let end = c + char_len(w);
        if end < char_len(text) {
            ret.push(Word::new(char_substr_from(text, end)));
        }
    }
    ret
}

// ---------------------------------------------------------------------------
// Wildcard
// ---------------------------------------------------------------------------

pub struct WildcardTokenizer;

impl Tokenizer for WildcardTokenizer {
    fn name(&self) -> &'static str {
        WILDCARD_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        split_unknown(words, |text| split_wildcard(text, seg))
    }
}

fn split_wildcard(text: &str, seg: &Segment) -> Option<Vec<Word>> {
    let table = &seg.wildcard;
    if table.table.is_empty() {
        return None;
    }
    let lower = text.to_lowercase();
    let n = char_len(text);
    let mut hits = Vec::new();
    let mut cur = 0usize;
    while cur < n {
        let mut best: Option<(usize, String)> = None;
        for (len, bucket) in &table.table2 {
            let piece = char_slice(&lower, cur, *len);
            if bucket.contains_key(&piece) {
                best = Some((cur, char_slice(text, cur, *len)));
            }
        }
        if let Some((c, w)) = best {
            let step = char_len(&w);
            hits.push((c, w));
            cur += step;
        } else {
            cur += 1;
        }
    }
    if hits.is_empty() {
        return None;
    }
    Some(interleave_hits(text, &hits, |w| {
        let p = table.table.get(&w.to_lowercase()).map(|e| e.p).unwrap_or(0);
        Word::new(w).with_p(p)
    }))
}

// ---------------------------------------------------------------------------
// Punctuation
// ---------------------------------------------------------------------------

pub struct PunctuationTokenizer;

impl Tokenizer for PunctuationTokenizer {
    fn name(&self) -> &'static str {
        PUNCTUATION_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        split_unknown(words, |text| {
            let hits = match_stopword(text);
            if hits.is_empty() {
                return None;
            }
            Some(interleave_hits(text, &hits, |w| {
                Word::new(w).with_p(POSTAG::D_W)
            }))
        })
    }
}

fn match_stopword(text: &str) -> Vec<(usize, String)> {
    let n = char_len(text);
    let mut ret = Vec::new();
    let mut cur = 0usize;
    if crate::text::is_bmp(text) {
        let chars: Vec<char> = text.chars().collect();
        while cur < n {
            let matched = STOPWORD2.iter().find_map(|(len, bucket)| {
                let w = slice_chars(&chars, cur, *len)?;
                bucket.contains_key(&w).then_some(w)
            });
            if let Some(w) = matched {
                let step = char_len(&w);
                ret.push((cur, w));
                cur += step;
            } else {
                cur += 1;
            }
        }
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        while cur < n {
            let matched = STOPWORD2.iter().find_map(|(len, bucket)| {
                let w = slice_utf16(&units, cur, *len)?;
                bucket.contains_key(&w).then_some(w)
            });
            if let Some(w) = matched {
                let step = char_len(&w);
                ret.push((cur, w));
                cur += step;
            } else {
                cur += 1;
            }
        }
    }
    ret
}

// ---------------------------------------------------------------------------
// Foreign
// ---------------------------------------------------------------------------

static RE_SPLIT_1: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r"(",
        r"[\u{4E00}-\u{9FFF}]+",
        r"|",
        r"[\d０-９]+(?:,[\d０-９]+)?(?:\.[\d０-９]+)?",
        r"|",
        r"[A-Za-z0-9_０-９Ａ-Ｚａ-ｚ\u{0100}-\u{017F}\u{00A1}-\u{00FF}]+",
        r"|",
        r"[\u{0600}-\u{06FF}\u{0750}-\u{077F}]+",
        r"|",
        r"[\u{0400}-\u{04FF}]+",
        r"|",
        r"[\u{0370}-\u{03FF}]+",
        r")"
    ))
    .unwrap()
});

static RE_SPLIT_2: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r"(",
        r"[\d０-９]+(?:,[\d０-９]+)?(?:\.[\d０-９]+)?",
        r"|",
        r"[A-Za-z0-9_０-９Ａ-Ｚａ-ｚ\u{0100}-\u{017F}\u{00A1}-\u{00FF}]+",
        r"|",
        r"[\u{0600}-\u{06FF}\u{0750}-\u{077F}]+",
        r"|",
        r"[\u{0400}-\u{04FF}]+",
        r"|",
        r"[\u{0370}-\u{03FF}]+",
        r")"
    ))
    .unwrap()
});

static RE_SPLIT_NUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"([\d+０-９]+)").unwrap());

pub struct ForeignTokenizer;

impl Tokenizer for ForeignTokenizer {
    fn name(&self) -> &'static str {
        FOREIGN_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        split_unknown(words, |text| split_foreign2(text, seg))
    }
}

fn split_foreign2(text: &str, seg: &Segment) -> Option<Vec<Word>> {
    let mut ret = Vec::new();
    for w in split_keep_re(text, &RE_SPLIT_1) {
        if w.is_empty() {
            continue;
        }
        if RE_SPLIT_2.is_match(&w) {
            if let Some(cw) = seg.table.table.get(&w) {
                ret.push(raw_from_entry(&w, cw));
                continue;
            }
            for part in split_keep_re(&w, &RE_SPLIT_NUM) {
                if part.is_empty() {
                    continue;
                }
                let mut c = part.chars().next().map(|ch| ch as u32).unwrap_or(0);
                if (65296..=65370).contains(&c) {
                    c -= 65248;
                }
                let lasttype = if (48..=57).contains(&c) {
                    POSTAG::A_M
                } else if (65..=90).contains(&c) || (97..=122).contains(&c) {
                    POSTAG::A_NX
                } else {
                    POSTAG::UNK
                };
                if lasttype == POSTAG::A_NX {
                    if let Some(cw) = seg.table.table.get(&part) {
                        ret.push(raw_from_entry(&part, cw));
                        continue;
                    }
                }
                let mut tok = Word::new(part);
                if lasttype != POSTAG::UNK {
                    tok.p = Some(lasttype);
                }
                ret.push(tok);
            }
        } else {
            ret.push(Word::new(w));
        }
    }
    if ret.is_empty() {
        None
    } else {
        Some(ret)
    }
}

fn split_keep_re(text: &str, re: &Regex) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0usize;
    for m in re.find_iter(text) {
        if m.start() > last {
            out.push(text[last..m.start()].to_string());
        }
        out.push(m.as_str().to_string());
        last = m.end();
    }
    if last < text.len() {
        out.push(text[last..].to_string());
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
}

fn raw_from_entry(w: &str, e: &DictEntry) -> Word {
    Word {
        w: w.to_string(),
        p: Some(e.p),
        f: Some(e.f),
        s: Some(e.s),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Dict (MMSG)
// ---------------------------------------------------------------------------

pub const DEFAULT_MAX_CHUNK_COUNT: usize = 40;
pub const DEFAULT_MAX_CHUNK_COUNT_MIN: usize = 30;
/// Spans longer than this (UTF-16 units) use DAG beam search instead of `get_chunks`.
pub(crate) const DICT_ENUM_MAX_SPAN: usize = 24;

/// Dictionary tokenizer (MMSEG-style scoring).
///
/// Spans longer than [`DICT_ENUM_MAX_SPAN`] use `dict_path` DAG search instead of
/// enumerating `get_chunks`. See the crate README.
pub struct DictTokenizer;

impl Tokenizer for DictTokenizer {
    fn name(&self) -> &'static str {
        DICT_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        let mut ret = Vec::new();
        for (i, word) in words.iter().enumerate() {
            if word.is_recognized() {
                ret.push(word.clone());
                continue;
            }
            let pre = if i > 0 { Some(&words[i - 1]) } else { None };
            let wordinfo = match_word(&word.w, 0, pre, seg);
            if wordinfo.is_empty() {
                ret.push(word.clone());
                continue;
            }
            let mut lastc = 0usize;
            for bw in &wordinfo {
                let c = bw.c.unwrap_or(0);
                if c > lastc {
                    ret.push(Word::new(char_slice(&word.w, lastc, c - lastc)));
                }
                if let Some(e) = seg.table.table.get(&bw.w) {
                    ret.push(Word {
                        w: bw.w.clone(),
                        p: Some(e.p),
                        f: bw.f.or(Some(e.f)),
                        s: Some(e.s),
                        ..Default::default()
                    });
                } else {
                    ret.push(Word {
                        w: bw.w.clone(),
                        f: bw.f,
                        ..Default::default()
                    });
                }
                lastc = c + char_len(&bw.w);
            }
            if let Some(last) = wordinfo.last() {
                let end = last.c.unwrap_or(0) + char_len(&last.w);
                if end < char_len(&word.w) {
                    ret.push(Word::new(char_substr_from(&word.w, end)));
                }
            }
        }
        ret
    }
}

fn match_word(text: &str, mut cur: usize, preword: Option<&Word>, seg: &Segment) -> Vec<Word> {
    let n = char_len(text);
    let mut ret = Vec::new();
    if crate::text::is_bmp(text) {
        let chars: Vec<char> = text.chars().collect();
        while cur < n {
            for (len, bucket) in &seg.table.table2 {
                if let Some(w) = slice_chars(&chars, cur, *len) {
                    if let Some(e) = bucket.get(&w) {
                        ret.push(Word {
                            w,
                            c: Some(cur),
                            f: Some(e.f),
                            ..Default::default()
                        });
                    }
                }
            }
            cur += 1;
        }
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        while cur < n {
            for (len, bucket) in &seg.table.table2 {
                if let Some(w) = slice_utf16(&units, cur, *len) {
                    if let Some(e) = bucket.get(&w) {
                        ret.push(Word {
                            w,
                            c: Some(cur),
                            f: Some(e.f),
                            ..Default::default()
                        });
                    }
                }
            }
            cur += 1;
        }
    }
    filter_word(ret, preword, text, seg)
}

fn filter_word(words: Vec<Word>, preword: Option<&Word>, text: &str, seg: &Segment) -> Vec<Word> {
    if words.is_empty() && text.is_empty() {
        return words;
    }
    let wordpos = get_pos_info(&words, text);
    // Enumerating get_chunks is exponential past this span length.
    if char_len(text) > DICT_ENUM_MAX_SPAN {
        let mut curr = crate::dict_path::best_path(&wordpos, text, preword, seg);
        curr.retain(|w| seg.table.table.contains_key(&w.w));
        return curr;
    }
    let chunks = get_chunks(&wordpos, 0, text, 0, None, seg);
    if chunks.is_empty() {
        return Vec::new();
    }
    let assess: Vec<_> = chunks
        .iter()
        .map(|chunk| crate::dict_path::assess_chunk(chunk, preword, text, seg))
        .collect();
    let mut curr = chunks[crate::dict_path::get_tops(&assess)].clone();
    curr.retain(|w| seg.table.table.contains_key(&w.w));
    curr
}

fn get_pos_info(words: &[Word], text: &str) -> Vec<Vec<Word>> {
    let n = char_len(text);
    let mut wordpos: Vec<Vec<Word>> = vec![Vec::new(); n + 1];
    for word in words {
        if let Some(c) = word.c {
            if c < wordpos.len() {
                wordpos[c].push(word.clone());
            }
        }
    }
    for i in 0..n {
        if wordpos[i].is_empty() {
            if let Some(ch) = char_at(text, i) {
                wordpos[i].push(Word::new(ch.to_string()).with_c(i).with_f(0.0));
            } else {
                // Surrogate / non-decodable UTF-16 unit: still expose a 1-unit edge.
                let unit = char_slice(text, i, 1);
                if !unit.is_empty() {
                    wordpos[i].push(Word::new(unit).with_c(i).with_f(0.0));
                }
            }
        }
    }
    wordpos
}

fn get_chunks(
    wordpos: &[Vec<Word>],
    pos: usize,
    text: &str,
    total_count: usize,
    max_chunk: Option<usize>,
    seg: &Segment,
) -> Vec<Vec<Word>> {
    let max_chunk_count = if total_count == 0 {
        let mut m = seg.max_chunk_count;
        if char_len(text) < m {
            m += 1;
        }
        m
    } else if max_chunk.unwrap_or(0) <= seg.max_chunk_count {
        max_chunk
            .unwrap_or(seg.max_chunk_count)
            .saturating_sub(1)
            .max(seg.min_chunk_count)
            .max(DEFAULT_MAX_CHUNK_COUNT_MIN)
    } else {
        max_chunk.unwrap_or(seg.max_chunk_count)
    };

    if let Some(run) = leading_repeat_run(text) {
        let s1 = char_slice(text, 0, run);
        let s2 = char_substr_from(text, run);
        let word = Word::new(s1).with_c(pos).with_f(0.0);
        if s2.is_empty() {
            return vec![vec![word]];
        }
        let chunks = get_chunks(
            wordpos,
            pos + run,
            &s2,
            total_count,
            Some(max_chunk_count),
            seg,
        );
        return chunks
            .into_iter()
            .map(|ws| {
                let mut v = vec![word.clone()];
                v.extend(ws);
                v
            })
            .collect();
    }

    let total_count = total_count + 1;
    let words = wordpos.get(pos).cloned().unwrap_or_default();
    let mut ret = Vec::new();
    for word in words {
        let nextcur = word.c.unwrap_or(pos) + char_len(&word.w);
        let next_exists = wordpos.get(nextcur).map(|v| !v.is_empty()).unwrap_or(false);
        if !next_exists {
            ret.push(vec![word]);
        } else if total_count > max_chunk_count {
            let mut w1 = vec![word];
            let mut j = nextcur;
            while let Some(bucket) = wordpos.get(j) {
                if let Some(w2) = bucket.first() {
                    w1.push(w2.clone());
                    j += char_len(&w2.w);
                } else {
                    break;
                }
            }
            ret.push(w1);
        } else {
            let t = char_substr_from(text, char_len(&word.w));
            let chunks = get_chunks(
                wordpos,
                nextcur,
                &t,
                total_count,
                Some(max_chunk_count),
                seg,
            );
            for ws in chunks {
                let mut v = vec![word.clone()];
                v.extend(ws);
                ret.push(v);
            }
        }
    }
    let _ = max_chunk_count;
    ret
}

// ---------------------------------------------------------------------------
// ChsName
// ---------------------------------------------------------------------------

pub struct ChsNameTokenizer;

impl Tokenizer for ChsNameTokenizer {
    fn name(&self) -> &'static str {
        CHS_NAME_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        let mut ret = Vec::new();
        for word in words {
            if word.is_recognized() {
                ret.push(word);
                continue;
            }
            let hits = match_name(&word.w);
            if hits.is_empty() {
                ret.push(word);
                continue;
            }
            ret.extend(interleave_hits(&word.w, &hits, |w| {
                Word::new(w).with_p(POSTAG::A_NR)
            }));
        }
        ret
    }
}

fn match_name(text: &str) -> Vec<(usize, String)> {
    let n = char_len(text);
    let mut ret = Vec::new();
    let mut cur = 0usize;
    while cur < n {
        let mut name: Option<String> = None;
        let f2 = char_slice(text, cur, 2);
        if FAMILY_NAME_2.contains(&f2) {
            let n1 = char_at(text, cur + 2)
                .map(|c| c.to_string())
                .unwrap_or_default();
            let n2 = char_at(text, cur + 3)
                .map(|c| c.to_string())
                .unwrap_or_default();
            if DOUBLE_NAME_1.contains(&n1) && DOUBLE_NAME_2.contains(&n2) {
                name = Some(format!("{f2}{n1}{n2}"));
            } else if SINGLE_NAME.contains(&n1) {
                let extra = if n1 == n2 { n2 } else { String::new() };
                name = Some(format!("{f2}{n1}{extra}"));
            }
        }
        let f1 = char_at(text, cur)
            .map(|c| c.to_string())
            .unwrap_or_default();
        if name.is_none() && FAMILY_NAME_1.contains(&f1) {
            let n1 = char_at(text, cur + 1)
                .map(|c| c.to_string())
                .unwrap_or_default();
            let n2 = char_at(text, cur + 2)
                .map(|c| c.to_string())
                .unwrap_or_default();
            if DOUBLE_NAME_1.contains(&n1) && DOUBLE_NAME_2.contains(&n2) {
                name = Some(format!("{f1}{n1}{n2}"));
            } else if SINGLE_NAME.contains(&n1) {
                let extra = if n1 == n2 { n2 } else { String::new() };
                name = Some(format!("{f1}{n1}{extra}"));
            }
        }
        if let Some(name) = name {
            let step = char_len(&name);
            ret.push((cur, name));
            cur += step;
        } else {
            cur += 1;
        }
    }
    ret
}

// ---------------------------------------------------------------------------
// Japanese
// ---------------------------------------------------------------------------

pub struct JpSimpleTokenizer;

impl Tokenizer for JpSimpleTokenizer {
    fn name(&self) -> &'static str {
        JP_SIMPLE_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        split_unset(words, split_jp)
    }
}

fn is_hira(c: char) -> bool {
    ('\u{3041}'..='\u{3093}').contains(&c)
}

fn is_kata(c: char) -> bool {
    matches!(c, '\u{30A1}'..='\u{30F4}' | 'ー' | '\u{FF71}'..='\u{FF9D}' | 'ﾞ' | 'ｰ')
}

fn kana_kind(c: char) -> Option<bool> {
    if is_hira(c) {
        Some(true)
    } else if is_kata(c) {
        Some(false)
    } else {
        None
    }
}

fn split_jp(text: &str) -> Option<Vec<Word>> {
    let b1 = text.chars().any(is_hira);
    let b2 = text.chars().any(is_kata);
    if !b1 || !b2 {
        if (b1 && text.chars().all(is_hira)) || (b2 && text.chars().all(is_kata)) {
            return Some(vec![Word::new(text)]);
        }
        return None;
    }
    // rust `regex` has no lookahead, so scan JS `(?:[^other]+)?run(?=other)`.
    Some(split_jp_mixed(text))
}

fn split_jp_mixed(text: &str) -> Vec<Word> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let n = chars.len();
    let mut ret = Vec::new();
    let mut from = 0;
    while from < n {
        let mut switch = None;
        for i in from + 1..n {
            if let (Some(prev), Some(cur)) = (kana_kind(chars[i - 1].1), kana_kind(chars[i].1)) {
                if prev != cur {
                    switch = Some((i, cur));
                    break;
                }
            }
        }
        let Some((i, other)) = switch else {
            ret.push(Word::new(&text[chars[from].0..]));
            break;
        };
        let mut j = i;
        while j > from && kana_kind(chars[j - 1].1) == Some(!other) {
            j -= 1;
        }
        while j > from && kana_kind(chars[j - 1].1) != Some(other) {
            j -= 1;
        }
        if j > from {
            ret.push(Word::new(&text[chars[from].0..chars[j].0]));
        }
        ret.push(Word::new(&text[chars[j].0..chars[i].0]));
        from = i;
    }
    ret
}

// ---------------------------------------------------------------------------
// Zhuyin
// ---------------------------------------------------------------------------

pub struct ZhuyinTokenizer;

impl Tokenizer for ZhuyinTokenizer {
    fn name(&self) -> &'static str {
        ZHUYIN_TOKENIZER
    }

    fn split(&self, words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        split_unset(words, split_zhuyin)
    }
}

fn is_zhuyin(c: char) -> bool {
    matches!(c, '\u{31A0}'..='\u{31BA}' | '\u{3105}'..='\u{312E}')
}

fn split_zhuyin(text: &str) -> Option<Vec<Word>> {
    if !text.chars().any(is_zhuyin) {
        return None;
    }
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"([\u{31A0}-\u{31BA}\u{3105}-\u{312E}]+)").unwrap());
    let mut ret = Vec::new();
    for part in split_keep_re(text, &RE) {
        if !part.is_empty() {
            ret.push(Word::new(part));
        }
    }
    if ret.is_empty() {
        None
    } else {
        Some(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_requires_remaining_longer_than_shortest_protocol() {
        // `ftp://` is 6 units; JS only searches when remaining length > 6.
        assert!(match_url("ftp://").is_empty());
        assert!(match_url("xftp://").is_empty());
        assert!(match_url("见ftp://").is_empty());
        assert_eq!(match_url("ftp://x"), vec![(0, "ftp://x".into())]);
        assert_eq!(match_url("http://"), vec![(0, "http://".into())]);
        assert_eq!(match_url("见ftp://x"), vec![(1, "ftp://x".into())]);
        assert_eq!(
            match_url("主页http://ucdok.com娃"),
            vec![(2, "http://ucdok.com".into())]
        );
    }

    fn jp_parts(text: &str) -> Option<Vec<String>> {
        split_jp(text).map(|words| words.into_iter().map(|word| word.w).collect())
    }

    #[test]
    fn split_jp_pure_scripts_and_non_jp() {
        assert_eq!(jp_parts("こんにちは"), Some(vec!["こんにちは".into()]));
        assert_eq!(jp_parts("カタカナ"), Some(vec!["カタカナ".into()]));
        assert!(jp_parts("hello").is_none());
        assert!(jp_parts("制度很好").is_none());
    }

    #[test]
    fn split_jp_mixed_kana_matches_js_lookahead_split() {
        let cases: &[(&str, &[&str])] = &[
            ("こんにちはカタカナ", &["こんにちは", "カタカナ"]),
            ("abcひらカタ", &["abcひら", "カタ"]),
            ("カタひらabc", &["カタ", "ひらabc"]),
            ("ひらカナひら", &["ひら", "カナ", "ひら"]),
            ("した「ドラゴン」", &["した「ドラゴン」"]),
            ("ひら漢字カタ", &["ひら漢字カタ"]),
            ("ひらxyzカタひら", &["ひら", "xyzカタ", "ひら"]),
            ("abcひらxyzカタ", &["abcひらxyzカタ"]),
            ("カタabcひらカタ", &["カタ", "abcひら", "カタ"]),
            ("ひら「カタひら", &["ひら", "「カタ", "ひら"]),
            ("ひカひ", &["ひ", "カ", "ひ"]),
            ("漢字カタひら", &["漢字カタ", "ひら"]),
            ("xxひらyyカタzzひら", &["xxひらyyカタzzひら"]),
            ("した「ドラゴン」です", &["した「ドラゴン」です"]),
            ("ｱｲｳこんにちは", &["ｱｲｳ", "こんにちは"]),
            ("ーカタひら", &["ーカタ", "ひら"]),
            ("ひらｰカタ", &["ひら", "ｰカタ"]),
        ];
        for &(text, parts) in cases {
            let expected: Vec<String> = parts.iter().map(|s| (*s).to_string()).collect();
            assert_eq!(jp_parts(text), Some(expected), "{text}");
        }
    }
}
