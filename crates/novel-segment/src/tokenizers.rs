//! Built-in tokenizers.

use crate::datetime::DATETIME;
use crate::dict::DictEntry;
use crate::names::{DOUBLE_NAME_1, DOUBLE_NAME_2, FAMILY_NAME_1, FAMILY_NAME_2, SINGLE_NAME};
use crate::pipeline::{
    split_unknown, split_unset, Tokenizer, CHS_NAME_TOKENIZER, DICT_TOKENIZER, FOREIGN_TOKENIZER,
    JP_SIMPLE_TOKENIZER, PUNCTUATION_TOKENIZER, URL_TOKENIZER, WILDCARD_TOKENIZER, ZHUYIN_TOKENIZER,
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
            Some(interleave_hits(text, &hits, |w| Word::new(w).with_p(POSTAG::URL)))
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

fn interleave_hits(text: &str, hits: &[(usize, String)], mut tagged: impl FnMut(&str) -> Word) -> Vec<Word> {
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
        let p = table
            .table
            .get(&w.to_lowercase())
            .map(|e| e.p)
            .unwrap_or(0);
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

#[derive(Clone, Debug)]
struct Assess {
    x: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

fn filter_word(words: Vec<Word>, preword: Option<&Word>, text: &str, seg: &Segment) -> Vec<Word> {
    if words.is_empty() && text.is_empty() {
        return words;
    }
    let wordpos = get_pos_info(&words, text);
    let chunks = get_chunks(&wordpos, 0, text, 0, None, seg);
    if chunks.is_empty() {
        return Vec::new();
    }
    let mut assess = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let mut row = Assess {
            x: chunk.len() as f64,
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
        };
        let sp = char_len(text) as f64 / chunk.len() as f64;
        let mut has_d_v = false;
        let mut prew: Option<Word> = preword.cloned();
        for (j, w) in chunk.iter().enumerate() {
            if let Some(e) = seg.table.table.get(&w.w) {
                let wp = e.p;
                row.a += w.f.unwrap_or(e.f);
                if j == 0 && preword.is_none() && wp & POSTAG::D_V != 0 {
                    has_d_v = true;
                }
                if let Some(ref pw) = prew {
                    if pw.w == "年" && matches!(w.w.as_str(), "历史" | "歷史" | "歴史") {
                        row.d += 0.5;
                    }
                    if pw.pos() & POSTAG::A_M != 0
                        && (wp & POSTAG::A_Q != 0 || DATETIME.contains(&w.w))
                    {
                        row.d += 1.0;
                    }
                    if wp & POSTAG::D_V != 0 {
                        has_d_v = true;
                        if pw.pos() & POSTAG::D_D != 0 {
                            row.d += 1.0;
                        }
                    }
                    if (pw.pos() & POSTAG::A_NS != 0
                        || pw.pos() & POSTAG::A_NT != 0
                        || pw.pos() & POSTAG::D_A != 0)
                        && (wp & POSTAG::D_N != 0
                            || wp & POSTAG::A_NR != 0
                            || wp & POSTAG::A_NS != 0
                            || wp & POSTAG::A_NZ != 0
                            || wp & POSTAG::A_NT != 0)
                    {
                        row.d += 1.0;
                    }
                    if pw.pos() & POSTAG::D_F != 0 && (wp & POSTAG::A_M != 0 || wp & POSTAG::D_MQ != 0)
                    {
                        row.d += 1.0;
                    }
                    if (FAMILY_NAME_1.contains(&pw.w) || FAMILY_NAME_2.contains(&pw.w))
                        && (wp & POSTAG::D_N != 0 || wp & POSTAG::A_NZ != 0)
                    {
                        row.d += 1.0;
                    }
                    if POSTAG::any(pw.pos(), &[POSTAG::D_S, POSTAG::A_NS])
                        && POSTAG::any(wp, &[POSTAG::D_F])
                    {
                        row.d += 0.5;
                    }
                    if let Some(nextw) = chunk.get(j + 1) {
                        let np = seg
                            .table
                            .table
                            .get(&nextw.w)
                            .map(|e| e.p)
                            .unwrap_or(nextw.pos());
                        let mut temp_ok = true;
                        if (w.w == "的" || w.w == "之")
                            && np != 0
                            && (np & POSTAG::D_N != 0
                                || np & POSTAG::D_V != 0
                                || np & POSTAG::A_NR != 0
                                || np & POSTAG::A_NS != 0
                                || np & POSTAG::A_NZ != 0
                                || np & POSTAG::A_NT != 0)
                        {
                            row.d += 1.5;
                            temp_ok = false;
                        } else if pw.pos() != 0 && wp & POSTAG::D_C != 0 {
                            let p = pw.pos() & np;
                            if pw.pos() == np {
                                row.d += 1.0;
                                temp_ok = false;
                            } else if p != 0 {
                                row.d += 0.25;
                                temp_ok = false;
                                if p & POSTAG::D_N != 0 {
                                    row.d += 0.75;
                                }
                            }
                        }
                        if temp_ok && wp & POSTAG::D_R != 0 && np & POSTAG::D_P != 0 {
                            row.d += 1.0;
                            temp_ok = false;
                        }
                        if temp_ok && np != 0 && wp & POSTAG::D_P != 0 {
                            if np & POSTAG::A_NR != 0 && char_len(&nextw.w) > 1 {
                                row.d += 1.0;
                                if pw.w == "的" {
                                    row.d += 1.0;
                                    temp_ok = false;
                                }
                            }
                        }
                        if temp_ok && wp & POSTAG::D_P != 0 {
                            if POSTAG::any(pw.pos(), &[POSTAG::D_N])
                                && POSTAG::any(np, &[POSTAG::D_N, POSTAG::D_V])
                            {
                                row.d += 1.0;
                                temp_ok = false;
                            } else if POSTAG::any(pw.pos(), &[POSTAG::D_R])
                                && POSTAG::any(np, &[POSTAG::D_R])
                            {
                                row.d += 0.5;
                                temp_ok = false;
                            }
                        }
                        let _ = temp_ok;
                        if nextw.w == "后"
                            && wp & POSTAG::D_T != 0
                            && POSTAG::any(pw.pos(), &[POSTAG::D_MQ, POSTAG::A_M])
                        {
                            row.d += 1.0;
                        } else if (nextw.w == "后" || nextw.w == "後") && POSTAG::any(wp, &[POSTAG::D_F])
                        {
                            row.d += 1.0;
                        } else if (w.w == "后" || w.w == "後")
                            && POSTAG::any(pw.pos(), &[POSTAG::D_F])
                            && POSTAG::any(np, &[POSTAG::D_N])
                        {
                            row.d += 1.0;
                        }
                    } else if wp & POSTAG::D_F != 0 && POSTAG::any(pw.pos(), &[POSTAG::D_N]) {
                        row.d += 1.0;
                    }
                }
                let mut scored = w.clone();
                scored.p = Some(wp);
                prew = Some(scored);
            } else {
                row.c += 1.0;
                prew = Some(w.clone());
            }
            row.b += (sp - char_len(&w.w) as f64).powi(2);
        }
        if !has_d_v {
            row.d -= 0.5;
        }
        row.a /= chunk.len() as f64;
        row.b /= chunk.len() as f64;
        assess.push(row);
    }
    let top = get_tops(&assess);
    let mut curr = chunks[top].clone();
    curr.retain(|w| seg.table.table.contains_key(&w.w));
    curr
}

fn get_tops(assess: &[Assess]) -> usize {
    if assess.is_empty() {
        return 0;
    }
    let mut top = assess[0].clone();
    for ass in assess.iter().skip(1) {
        if ass.a > top.a {
            top.a = ass.a;
        }
        if ass.b < top.b {
            top.b = ass.b;
        }
        if ass.c > top.c {
            top.c = ass.c;
        }
        if ass.d < top.d {
            top.d = ass.d;
        }
        if ass.x > top.x {
            top.x = ass.x;
        }
    }
    let mut tops = Vec::with_capacity(assess.len());
    for ass in assess {
        let mut s = (top.x - ass.x) * 1.5;
        if ass.a >= top.a {
            s += 1.0;
        }
        if ass.b <= top.b {
            s += 1.0;
        }
        s += top.c - ass.c;
        s += if ass.d < 0.0 {
            top.d + ass.d
        } else {
            ass.d - top.d
        };
        tops.push(s);
    }
    let mut curri = 0usize;
    let mut maxs = tops[0];
    for (i, &s) in tops.iter().enumerate() {
        if s > maxs {
            curri = i;
            maxs = s;
        } else if (s - maxs).abs() < f64::EPSILON {
            let mut a = 0;
            let mut b = 0;
            if assess[i].c < assess[curri].c {
                a += 1;
            } else if assess[i].c != assess[curri].c {
                b += 1;
            }
            if assess[i].a > assess[curri].a {
                a += 1;
            } else if assess[i].a != assess[curri].a {
                b += 1;
            }
            if assess[i].x < assess[curri].x {
                a += 1;
            } else if assess[i].x != assess[curri].x {
                b += 1;
            }
            if a > b {
                curri = i;
                maxs = s;
            }
        }
    }
    curri
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
            let chunks = get_chunks(wordpos, nextcur, &t, total_count, Some(max_chunk_count), seg);
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
            let n1 = char_at(text, cur + 2).map(|c| c.to_string()).unwrap_or_default();
            let n2 = char_at(text, cur + 3).map(|c| c.to_string()).unwrap_or_default();
            if DOUBLE_NAME_1.contains(&n1) && DOUBLE_NAME_2.contains(&n2) {
                name = Some(format!("{f2}{n1}{n2}"));
            } else if SINGLE_NAME.contains(&n1) {
                let extra = if n1 == n2 { n2 } else { String::new() };
                name = Some(format!("{f2}{n1}{extra}"));
            }
        }
        let f1 = char_at(text, cur).map(|c| c.to_string()).unwrap_or_default();
        if name.is_none() && FAMILY_NAME_1.contains(&f1) {
            let n1 = char_at(text, cur + 1).map(|c| c.to_string()).unwrap_or_default();
            let n2 = char_at(text, cur + 2).map(|c| c.to_string()).unwrap_or_default();
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

fn split_jp(text: &str) -> Option<Vec<Word>> {
    let b1 = text.chars().any(is_hira);
    let b2 = text.chars().any(is_kata);
    if !b1 || !b2 {
        if (b1 && text.chars().all(is_hira)) || (b2 && text.chars().all(is_kata)) {
            return Some(vec![Word::new(text)]);
        }
        return None;
    }
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(concat!(
            r"((?:[^ァ-ヴーｱ-ﾝﾞｰ]+)?[ぁ-ん]+(?=[ァ-ヴーｱ-ﾝﾞｰ])",
            r"|(?:[^ぁ-ん]+)?[ァ-ヴーｱ-ﾝﾞｰ]+(?=[ぁ-ん]))"
        ))
        .unwrap()
    });
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
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"([\u{31A0}-\u{31BA}\u{3105}-\u{312E}]+)").unwrap());
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
}
