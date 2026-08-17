//! Built-in optimizers.

use crate::colors::{COLOR_ALL, COLOR_HAIR};
use crate::datetime::DATETIME;
use crate::names::{
    is_family_name, DOUBLE_NAME_1, DOUBLE_NAME_2, SINGLE_NAME, SINGLE_NAME_NO_REPEAT,
};
use crate::pipeline::{
    splice_created, splice_token, Optimizer, ADJECTIVE_OPTIMIZER, CHS_NAME_OPTIMIZER,
    DATETIME_OPTIMIZER, DICT_OPTIMIZER, EMAIL_OPTIMIZER, FOREIGN_OPTIMIZER,
};
use crate::postag::POSTAG;
use crate::segment::Segment;
use crate::word::Word;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

pub fn builtin_optimizers() -> Vec<Box<dyn Optimizer>> {
    vec![
        Box::new(EmailOptimizer),
        Box::new(ChsNameOptimizer),
        Box::new(DictOptimizer),
        Box::new(DatetimeOptimizer),
        Box::new(ForeignOptimizer),
        Box::new(crate::zht::ZhtSynonymOptimizer),
        Box::new(AdjectiveOptimizer),
    ]
}

// ---------------------------------------------------------------------------
// Email
// ---------------------------------------------------------------------------

static EMAIL_CHAR: Lazy<HashSet<char>> = Lazy::new(|| {
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&'*+-/=?^_`{|}~.@\""
        .chars()
        .collect()
});

pub struct EmailOptimizer;

impl Optimizer for EmailOptimizer {
    fn name(&self) -> &'static str {
        EMAIL_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        let mut i = 0usize;
        let mut addr_start: Option<usize> = None;
        let mut has_at = false;
        while i + 1 < words.len() {
            let is_ascii = words[i].pos() == POSTAG::A_NX
                || (words[i].pos() == POSTAG::A_M
                    && words[i].w.chars().next().map(|c| (c as u32) < 128).unwrap_or(false));
            if addr_start.is_none() && is_ascii {
                addr_start = Some(i);
                i += 1;
                continue;
            }
            if !has_at && words[i].w == "@" {
                has_at = true;
                i += 1;
                continue;
            }
            let in_email = words[i].w.chars().next().map(|c| EMAIL_CHAR.contains(&c)).unwrap_or(false);
            if has_at && i > 0 && words[i - 1].w != "@" && !is_ascii && !in_email {
                if let Some(start) = addr_start {
                    let mail: String = words[start..i].iter().map(|w| w.w.as_str()).collect();
                    let len = i - start;
                    splice_token(&mut words, start, len, Word::new(mail).with_p(POSTAG::URL));
                    i = start + 1;
                    addr_start = None;
                    has_at = false;
                    continue;
                }
            }
            if addr_start.is_some() && (is_ascii || in_email) {
                i += 1;
                continue;
            }
            addr_start = None;
            has_at = false;
            i += 1;
        }
        if let Some(start) = addr_start {
            if has_at {
                if let Some(last) = words.last() {
                    let is_ascii = last.pos() == POSTAG::A_NX
                        || (last.pos() == POSTAG::A_M
                            && last.w.chars().count() == 1
                            && last.w.chars().next().map(|c| EMAIL_CHAR.contains(&c)).unwrap_or(false));
                    if is_ascii {
                        let mail: String = words[start..].iter().map(|w| w.w.as_str()).collect();
                        let len = words.len() - start;
                        splice_token(&mut words, start, len, Word::new(mail).with_p(POSTAG::URL));
                    }
                }
            }
        }
        words
    }
}

// ---------------------------------------------------------------------------
// ChsName
// ---------------------------------------------------------------------------

pub struct ChsNameOptimizer;

impl Optimizer for ChsNameOptimizer {
    fn name(&self) -> &'static str {
        CHS_NAME_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        let mut i = 0;
        while i + 1 < words.len() {
            let w = words[i].w.clone();
            let n1 = words[i + 1].w.clone();
            let nw = format!("{w}{n1}");
            if seg.blacklist_optimizer.contains(&nw) {
                i += 1;
                continue;
            }
            let valid = {
                let ew = seg.table.table.get(&nw);
                ew.map(|e| e.p == 0 || e.p & POSTAG::A_NR != 0).unwrap_or(true)
            };
            if !valid {
                i += 1;
                continue;
            }
            if i + 2 < words.len() {
                let n2 = words[i + 2].w.clone();
                let three = format!("{nw}{n2}");
                if n2.chars().count() <= 2
                    && w != "于"
                    && words[i + 2].pos() & POSTAG::D_P == 0
                    && is_family_name(&w)
                    && is_first_name(&n1, &n2)
                    && !seg.blacklist_optimizer.contains(&three)
                {
                    splice_created(&mut words, i, 3, Word::new(three).with_p(POSTAG::A_NR), seg);
                    i += 2;
                    continue;
                }
            }
            if (w == "小" || w == "老") && is_family_name(&n1) {
                splice_created(&mut words, i, 2, Word::new(nw).with_p(POSTAG::A_NR), seg);
                i += 1;
                continue;
            }
            if is_family_name(&w)
                && words[i + 1].pos() & POSTAG::A_NR != 0
                && crate::text::char_len(&n1) <= 2
            {
                splice_created(&mut words, i, 2, Word::new(nw).with_p(POSTAG::A_NR), seg);
                i += 1;
                continue;
            }
            if words[i].pos_falsy() || words[i + 1].pos_falsy() {
                if is_first_name(&w, &n1) {
                    splice_created(&mut words, i, 2, Word::new(nw.clone()).with_p(POSTAG::A_NR), seg);
                    if i > 0 {
                        let pre = words[i - 1].w.clone();
                        let three = format!("{pre}{w}{n1}");
                        if !pre.is_empty()
                            && is_family_name(&pre)
                            && !seg.blacklist_optimizer.contains(&three)
                        {
                            splice_created(&mut words, i - 1, 2, Word::new(three).with_p(POSTAG::A_NR), seg);
                            continue;
                        }
                    }
                    i += 1;
                    continue;
                }
            }
            if is_family_name(&w)
                && (words[i].pos_falsy() || words[i + 1].pos_falsy())
                && words[i].pos() & POSTAG::D_W == 0
                && words[i + 1].pos() & POSTAG::D_W == 0
            {
                splice_created(&mut words, i, 2, Word::new(nw).with_p(POSTAG::A_NR), seg);
            }
            i += 1;
        }
        i = 0;
        while i + 1 < words.len() {
            let w = words[i].w.clone();
            let n1 = words[i + 1].w.clone();
            let nw = format!("{w}{n1}");
            if seg.blacklist_optimizer.contains(&nw) {
                i += 1;
                continue;
            }
            if is_family_name(&w) && SINGLE_NAME.contains(&n1) {
                let ew = seg.table.table.get(&nw);
                if ew.map(|e| e.p == 0 || e.p & POSTAG::A_NR != 0).unwrap_or(true) {
                    splice_created(&mut words, i, 2, Word::new(nw).with_p(POSTAG::A_NR), seg);
                    i += 1;
                    continue;
                }
            }
            i += 1;
        }
        words
    }
}

fn is_first_name(w1: &str, w2: &str) -> bool {
    (SINGLE_NAME_NO_REPEAT.contains(w1) && SINGLE_NAME.contains(w1) && w2 == w1)
        || (DOUBLE_NAME_1.contains(w1) && DOUBLE_NAME_2.contains(w2))
}

// ---------------------------------------------------------------------------
// Dict optimizer
// ---------------------------------------------------------------------------

static RE_HOW_MANY: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:[數数幾几][百千萬十億兆万亿]|毎)$").unwrap());
static RE_JI: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[數数幾几第]$").unwrap());
static RE_DIR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[東西南北东]+$").unwrap());

pub struct DictOptimizer;

impl Optimizer for DictOptimizer {
    fn name(&self) -> &'static str {
        DICT_OPTIMIZER
    }

    fn do_optimize(&self, words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        let once = dict_optimize_pass(words, seg);
        dict_optimize_pass(once, seg)
    }
}

fn dict_optimize_pass(mut words: Vec<Word>, seg: &Segment) -> Vec<Word> {
    let mut i = 0;
    while i + 1 < words.len() {
        let w1 = words[i].clone();
        let w2 = words[i + 1].clone();
        let nw = format!("{}{}", w1.w, w2.w);
        let cache = seg.table.table.get(&nw).cloned();

        if w1.w != "了" && w1.pos() & POSTAG::D_A != 0 && w2.pos() & POSTAG::D_U != 0 {
            let mut p = POSTAG::D_A;
            let mut f = None;
            if cache.as_ref().map(|e| e.p & POSTAG::D_A != 0).unwrap_or(false) || cache.is_none() {
                if let Some(e) = &cache {
                    if e.p & POSTAG::D_A != 0 {
                        p = e.p;
                        f = Some(e.f);
                    }
                } else if w1.pos() & POSTAG::BAD != 0 {
                    p = POSTAG::D_A | POSTAG::BAD;
                }
                let mut tok = Word::new(nw.clone()).with_p(p);
                tok.f = f;
                splice_created(&mut words, i, 2, tok, seg);
                continue;
            }
        }

        if w1.pos() & POSTAG::D_A != 0 && w2.pos() & POSTAG::D_N != 0 {
            if let Some(e) = &cache {
                if e.p & POSTAG::D_N != 0 {
                    splice_created(
                        &mut words,
                        i,
                        2,
                        Word::new(nw.clone()).with_p(e.p).with_f(e.f),
                        seg,
                    );
                    continue;
                }
            }
        }

        if is_mergeable(&w1, &w2, &nw, cache.is_some(), cache.as_ref()) {
            if let Some(e) = &cache {
                splice_created(&mut words, i, 2,
                    Word::new(nw.clone()).with_p(e.p).with_f(e.f),
                    seg,
                );
                continue;
            }
        }

        if w1.pos() & POSTAG::A_M != 0 {
            if (w2.pos() & POSTAG::A_M != 0 && !w2.w.starts_with('第'))
                || w2.w == "%"
                || w2.w == "％"
            {
                splice_created(
                    &mut words,
                    i,
                    2,
                    Word::new(format!("{}{}", w1.w, w2.w)).with_p(POSTAG::A_M),
                    seg,
                );
                continue;
            }
            if w2.pos() & POSTAG::A_Q != 0 {
                let p = merge_how_many(POSTAG::D_MQ, w2.pos(), cache.as_ref().map(|e| e.p));
                splice_created(&mut words, i, 2, Word::new(nw.clone()).with_p(p), seg);
                continue;
            }
            if i + 2 < words.len() && words[i + 2].pos() & POSTAG::A_M != 0 {
                let w3 = words[i + 2].clone();
                if matches!(w2.w.as_str(), "." | "点" | "點" | "分之") {
                    splice_created(
                        &mut words,
                        i,
                        3,
                        Word::new(format!("{}{}{}", w1.w, w2.w, w3.w)).with_p(POSTAG::A_M),
                        seg,
                    );
                    continue;
                }
                if w2.w == "," {
                    let r1 = Regex::new(r"^[\d０-９]+$").unwrap();
                    let r2 = Regex::new(r"^(?:(?:[\d０-９]+)?(?:\.[\d０-９]+)|(?:[\d０-９]+))$").unwrap();
                    if r1.is_match(&w1.w) && r2.is_match(&w3.w) {
                        splice_created(&mut words, i, 3,
                            Word::new(format!("{}{}{}", w1.w, w2.w, w3.w)).with_p(POSTAG::A_M),
                            seg,
                        );
                        continue;
                    }
                }
            }
        }

        if RE_HOW_MANY.is_match(&w1.w) && w2.pos() & POSTAG::A_Q != 0 {
            let p = merge_how_many(POSTAG::D_MQ, w2.pos(), cache.as_ref().map(|e| e.p));
            splice_created(&mut words, i, 2, Word::new(nw.clone()).with_p(p), seg);
            continue;
        }

        if RE_JI.is_match(&w1.w)
            && w2.pos() & POSTAG::A_M != 0
            && i + 2 < words.len()
            && words[i + 2].pos() & POSTAG::A_Q != 0
        {
            let w3 = words[i + 2].clone();
            let comb = format!("{}{}{}", w1.w, w2.w, w3.w);
            let c3 = seg.table.table.get(&comb);
            if c3.map(|e| e.p == 0).unwrap_or(true) {
                let p = merge_how_many(POSTAG::D_MQ, w3.pos(), c3.map(|e| e.p));
                splice_created(&mut words, i, 3, Word::new(comb).with_p(p), seg);
                continue;
            }
        }

        if w1.pos() & POSTAG::D_MQ != 0
            && (w1.w.ends_with('點') || w1.w.ends_with('点'))
            && w2.pos() & POSTAG::A_M != 0
        {
            let mut extra = w2.w.clone();
            let mut take = 2;
            let mut j = i + 2;
            while j < words.len() && words[j].pos() & POSTAG::A_M != 0 {
                extra.push_str(&words[j].w);
                take += 1;
                j += 1;
            }
            splice_created(
                &mut words,
                i,
                take,
                Word::new(format!("{}{}", w1.w, extra)).with_p(POSTAG::D_MQ),
                seg,
            );
            continue;
        }

        if RE_DIR.is_match(&w1.w) && RE_DIR.is_match(&w2.w) {
            let mut p = POSTAG::D_F;
            if let Some(e) = &cache {
                p |= e.p;
            }
            splice_created(&mut words, i, 2, Word::new(nw).with_p(p), seg);
            continue;
        }

        i += 1;
    }
    words
}

fn is_mergeable(w1: &Word, w2: &Word, _nw: &str, exists: bool, cache: Option<&crate::dict::DictEntry>) -> bool {
    if !exists {
        return false;
    }
    if w1.p == w2.p {
        return true;
    }
    if w1.pos() & w2.pos() != 0 {
        return true;
    }
    if w1.p.is_some() && w1.pos() != 0 && w2.p.is_none() {
        return true;
    }
    if w1.pos() & POSTAG::D_D != 0 && w2.pos() & POSTAG::D_V != 0 {
        if let Some(mw) = cache {
            return mw.p & POSTAG::D_D != 0 || mw.p & POSTAG::D_V != 0;
        }
    }
    false
}

fn merge_how_many(mut p: u32, p2: u32, p3: Option<u32>) -> u32 {
    if let Some(p3) = p3 {
        p = p3 | POSTAG::D_MQ;
    } else {
        if p2 & POSTAG::D_T != 0 {
            p |= POSTAG::D_T;
        }
        if p2 & POSTAG::D_N != 0 {
            p |= POSTAG::D_N;
        }
        if p2 & POSTAG::D_V != 0 {
            p |= POSTAG::D_V;
        }
    }
    p
}

// ---------------------------------------------------------------------------
// Datetime
// ---------------------------------------------------------------------------

pub struct DatetimeOptimizer;

impl Optimizer for DatetimeOptimizer {
    fn name(&self) -> &'static str {
        DATETIME_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        let mut i = 0;
        while i + 1 < words.len() {
            if words[i].pos() & POSTAG::A_M != 0 && DATETIME.contains(&words[i + 1].w) {
                let mut nw = format!("{}{}", words[i].w, words[i + 1].w);
                let mut len = 2;
                loop {
                    let a = i + len;
                    let b = i + len + 1;
                    if b < words.len()
                        && words[a].pos() & POSTAG::A_M != 0
                        && DATETIME.contains(&words[b].w)
                    {
                        nw.push_str(&words[a].w);
                        nw.push_str(&words[b].w);
                        len += 2;
                    } else {
                        break;
                    }
                }
                splice_token(&mut words, i, len, Word::new(nw).with_p(POSTAG::D_T));
                continue;
            }
            i += 1;
        }
        words
    }
}

// ---------------------------------------------------------------------------
// Foreign
// ---------------------------------------------------------------------------

pub struct ForeignOptimizer;

impl Optimizer for ForeignOptimizer {
    fn name(&self) -> &'static str {
        FOREIGN_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        let mut i = 0;
        while i + 1 < words.len() {
            if words[i].pos() != POSTAG::A_NX {
                i += 1;
                continue;
            }
            let nw = format!("{}{}", words[i].w, words[i + 1].w);
            if let Some(e) = seg.table.table.get(&nw) {
                splice_created(
                    &mut words,
                    i,
                    2,
                    Word::new(nw).with_p(e.p).with_f(e.f),
                    seg,
                );
                continue;
            }
            i += 1;
        }
        words
    }
}

// ---------------------------------------------------------------------------
// Adjective
// ---------------------------------------------------------------------------

pub struct AdjectiveOptimizer;

impl Optimizer for AdjectiveOptimizer {
    fn name(&self) -> &'static str {
        ADJECTIVE_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, _seg: &Segment) -> Vec<Word> {
        for i in 0..words.len() {
            if i + 1 >= words.len() {
                break;
            }
            let next_p = words[i + 1].pos();
            let w = words[i].w.clone();
            if next_p & POSTAG::D_U != 0 && COLOR_ALL.contains(&w) {
                words[i].op = words[i].op.or(words[i].p);
                words[i].p = Some(words[i].pos() | POSTAG::D_A);
            }
            if words[i].pos() & POSTAG::D_N != 0 && is_nominal(next_p) && COLOR_ALL.contains(&w) {
                words[i].op = words[i].op.or(words[i].p);
                words[i].p = Some(words[i].pos() | POSTAG::D_A | POSTAG::D_N);
            }
            if (w == "純" || w == "纯") && COLOR_HAIR.contains(&words[i + 1].w) {
                words[i].op = words[i].op.or(words[i].p);
                words[i].p = Some(words[i].pos() | POSTAG::D_A);
            }
        }
        words
    }
}

fn is_nominal(pos: u32) -> bool {
    pos == POSTAG::D_N
        || pos == POSTAG::A_NT
        || pos == POSTAG::A_NX
        || pos == POSTAG::A_NZ
        || pos == POSTAG::A_NR
        || pos == POSTAG::A_NS
        || pos == POSTAG::URL
}
