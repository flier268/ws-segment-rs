//! Long-span DictTokenizer search: beam-prune by path totals, then rank survivors
//! with the shared Assess / `get_tops`.

use crate::datetime::DATETIME;
use crate::names::{FAMILY_NAME_1, FAMILY_NAME_2};
use crate::postag::POSTAG;
use crate::segment::Segment;
use crate::text::{char_len, char_slice, leading_repeat_run_units};
use crate::word::Word;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Default DAG survivors per character position. Override with `WS_SEGMENT_MAX_STATES`.
const DEFAULT_MAX_STATES_PER_POS: usize = 64;

thread_local! {
    /// `0` means use env / default. Tests override per-thread so parallel suites do not race.
    static MAX_STATES_OVERRIDE: Cell<usize> = const { Cell::new(0) };
}

static PRUNE_EVENTS: AtomicU64 = AtomicU64::new(0);
static MAX_SLOT_SEEN: AtomicUsize = AtomicUsize::new(0);

fn resolve_max_states() -> usize {
    let over = MAX_STATES_OVERRIDE.with(|c| c.get());
    if over > 0 {
        return over;
    }
    std::env::var("WS_SEGMENT_MAX_STATES")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .filter(|value| *value >= 1)
        .unwrap_or(DEFAULT_MAX_STATES_PER_POS)
}

#[derive(Clone, Debug)]
pub(crate) struct Assess {
    pub x: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

struct Node {
    word: Word,
    prev: Option<Word>,
    /// Path word count (must fit spans longer than `u16::MAX` units).
    x: u32,
    sum_f: f64,
    sum_len2: f64,
    c: f64,
    d: f64,
    has_d_v: bool,
    parent: Option<usize>,
    dummy: bool,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct MergeKey {
    last: String,
    prev: String,
    has_d_v: bool,
    x: u32,
}

pub(crate) fn best_path(
    wordpos: &[Vec<Word>],
    text: &str,
    preword: Option<&Word>,
    seg: &Segment,
) -> Vec<Word> {
    best_path_with_cap(wordpos, text, preword, seg, resolve_max_states())
}

fn best_path_with_cap(
    wordpos: &[Vec<Word>],
    text: &str,
    preword: Option<&Word>,
    seg: &Segment,
    cap: usize,
) -> Vec<Word> {
    let n = char_len(text);
    if n == 0 {
        return Vec::new();
    }
    let units: Vec<u16> = text.encode_utf16().collect();

    let mut nodes: Vec<Node> = vec![Node {
        word: Word::new(""),
        prev: preword.cloned(),
        x: 0,
        sum_f: 0.0,
        sum_len2: 0.0,
        c: 0.0,
        d: 0.0,
        has_d_v: false,
        parent: None,
        dummy: true,
    }];
    let mut at: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
    at[0].push(0);

    for pos in 0..n {
        if at[pos].is_empty() {
            continue;
        }
        let outgoing = outgoing(wordpos, text, &units, pos);
        if outgoing.is_empty() {
            continue;
        }
        for ni in at[pos].clone() {
            for cand in &outgoing {
                let len = char_len(&cand.w);
                let end = pos + len;
                if end > n || len == 0 {
                    continue;
                }
                let child = extend(&nodes[ni], cand, preword, seg, ni);
                insert_node(&mut nodes, &mut at[end], child, cap);
            }
        }
    }

    let complete: Vec<usize> = at[n].iter().copied().filter(|&i| !nodes[i].dummy).collect();
    if complete.is_empty() {
        return Vec::new();
    }

    let chunks: Vec<Vec<Word>> = complete.iter().map(|&i| reconstruct(&nodes, i)).collect();
    let assess: Vec<Assess> = chunks
        .iter()
        .map(|chunk| assess_chunk(chunk, preword, text, seg))
        .collect();
    chunks[get_tops(&assess)].clone()
}

fn outgoing(wordpos: &[Vec<Word>], text: &str, units: &[u16], pos: usize) -> Vec<Word> {
    if pos < units.len() {
        if let Some(run) = leading_repeat_run_units(&units[pos..]) {
            let s1 = char_slice(text, pos, run);
            return vec![Word::new(s1).with_c(pos).with_f(0.0)];
        }
    }
    let mut outs = wordpos.get(pos).cloned().unwrap_or_default();
    // Keep the DAG traversable when a UTF-16 unit has no dictionary hit
    // (e.g. surrogate half that `char_at` cannot map to a `char`).
    if outs.is_empty() && pos < units.len() {
        let unit = String::from_utf16_lossy(&units[pos..pos + 1]);
        outs.push(Word::new(unit).with_c(pos).with_f(0.0));
    }
    outs
}

fn extend(
    parent: &Node,
    cand: &Word,
    preword: Option<&Word>,
    seg: &Segment,
    parent_i: usize,
) -> Node {
    let len = char_len(&cand.w) as f64;
    let mut word = cand.clone();
    let mut sum_f = parent.sum_f;
    let mut c = parent.c;
    let mut d = parent.d;
    let mut has_d_v = parent.has_d_v;

    let pw = if parent.dummy {
        preword
    } else {
        Some(&parent.word)
    };

    if !parent.dummy {
        if let Some(e_prev) = seg.table.table.get(&parent.word.w) {
            if let Some(pp) = parent.prev.as_ref() {
                d += next_bonus(pp, &parent.word, e_prev.p, cand, seg);
            }
        }
    }

    if let Some(e) = seg.table.table.get(&cand.w) {
        let wp = e.p;
        word.p = Some(wp);
        word.f = Some(cand.f.unwrap_or(e.f));
        sum_f += word.f.unwrap_or(e.f);
        if parent.dummy && preword.is_none() && wp & POSTAG::D_V != 0 {
            has_d_v = true;
        }
        if let Some(pw) = pw {
            d += pair_bonus(pw, &word, wp, &mut has_d_v);
        }
    } else {
        c += 1.0;
    }

    Node {
        prev: if parent.dummy {
            preword.cloned()
        } else {
            Some(parent.word.clone())
        },
        word,
        x: parent.x + 1,
        sum_f,
        sum_len2: parent.sum_len2 + len * len,
        c,
        d,
        has_d_v,
        parent: Some(parent_i),
        dummy: false,
    }
}

fn insert_node(nodes: &mut Vec<Node>, slot: &mut Vec<usize>, child: Node, cap: usize) {
    let key = MergeKey {
        last: child.word.w.clone(),
        prev: child.prev.as_ref().map(|w| w.w.clone()).unwrap_or_default(),
        has_d_v: child.has_d_v,
        x: child.x,
    };
    let mut dominated = false;
    let mut drop: Vec<usize> = Vec::new();
    for (idx, &existing) in slot.iter().enumerate() {
        let e = &nodes[existing];
        let e_prev = e.prev.as_ref().map(|w| w.w.as_str()).unwrap_or("");
        if e.word.w != key.last || e_prev != key.prev || e.has_d_v != key.has_d_v || e.x != key.x {
            continue;
        }
        if dominates(e, &child) {
            dominated = true;
            break;
        }
        if dominates(&child, e) {
            drop.push(idx);
        }
    }
    if dominated {
        return;
    }
    for idx in drop.into_iter().rev() {
        slot.swap_remove(idx);
    }
    slot.push(nodes.len());
    nodes.push(child);
    MAX_SLOT_SEEN.fetch_max(slot.len(), Ordering::Relaxed);
    if slot.len() > cap {
        PRUNE_EVENTS.fetch_add(1, Ordering::Relaxed);
        prune_slot(nodes, slot, cap);
    }
}

fn prune_slot(nodes: &[Node], slot: &mut Vec<usize>, cap: usize) {
    // get_tops strongly prefers fewer words (`x`); keep those, then higher `d`.
    slot.sort_by(|&a, &b| {
        let left = &nodes[a];
        let right = &nodes[b];
        left.x
            .cmp(&right.x)
            .then_with(|| {
                right
                    .d
                    .partial_cmp(&left.d)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                right
                    .sum_f
                    .partial_cmp(&left.sum_f)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    slot.truncate(cap);
}

fn dominates(a: &Node, b: &Node) -> bool {
    const EPS: f64 = 1e-9;
    let ge_f = a.sum_f + EPS >= b.sum_f;
    let ge_d = a.d + EPS >= b.d;
    let le_len = a.sum_len2 <= b.sum_len2 + EPS;
    let le_c = a.c <= b.c + EPS;
    let better = (a.sum_f > b.sum_f + EPS)
        || (a.d > b.d + EPS)
        || (a.sum_len2 + EPS < b.sum_len2)
        || (a.c + EPS < b.c);
    ge_f && ge_d && le_len && le_c && better
}

pub(crate) fn assess_chunk(
    chunk: &[Word],
    preword: Option<&Word>,
    text: &str,
    seg: &Segment,
) -> Assess {
    let mut row = Assess {
        x: chunk.len() as f64,
        a: 0.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    if chunk.is_empty() {
        return row;
    }
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
                row.d += pair_bonus(pw, w, wp, &mut has_d_v);
                if let Some(nextw) = chunk.get(j + 1) {
                    row.d += next_bonus(pw, w, wp, nextw, seg);
                } else {
                    row.d += last_bonus(pw, wp);
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
    row
}

fn reconstruct(nodes: &[Node], mut i: usize) -> Vec<Word> {
    let mut out = Vec::new();
    loop {
        let n = &nodes[i];
        if n.dummy {
            break;
        }
        out.push(n.word.clone());
        match n.parent {
            Some(p) => i = p,
            None => break,
        }
    }
    out.reverse();
    out
}

fn pair_bonus(pw: &Word, w: &Word, wp: u32, has_d_v: &mut bool) -> f64 {
    let mut d = 0.0;
    if pw.w == "年" && matches!(w.w.as_str(), "历史" | "歷史" | "歴史") {
        d += 0.5;
    }
    if pw.pos() & POSTAG::A_M != 0 && (wp & POSTAG::A_Q != 0 || DATETIME.contains(&w.w)) {
        d += 1.0;
    }
    if wp & POSTAG::D_V != 0 {
        *has_d_v = true;
        if pw.pos() & POSTAG::D_D != 0 {
            d += 1.0;
        }
    }
    if (pw.pos() & POSTAG::A_NS != 0 || pw.pos() & POSTAG::A_NT != 0 || pw.pos() & POSTAG::D_A != 0)
        && (wp & POSTAG::D_N != 0
            || wp & POSTAG::A_NR != 0
            || wp & POSTAG::A_NS != 0
            || wp & POSTAG::A_NZ != 0
            || wp & POSTAG::A_NT != 0)
    {
        d += 1.0;
    }
    if pw.pos() & POSTAG::D_F != 0 && (wp & POSTAG::A_M != 0 || wp & POSTAG::D_MQ != 0) {
        d += 1.0;
    }
    if (FAMILY_NAME_1.contains(&pw.w) || FAMILY_NAME_2.contains(&pw.w))
        && (wp & POSTAG::D_N != 0 || wp & POSTAG::A_NZ != 0)
    {
        d += 1.0;
    }
    if POSTAG::any(pw.pos(), &[POSTAG::D_S, POSTAG::A_NS]) && POSTAG::any(wp, &[POSTAG::D_F]) {
        d += 0.5;
    }
    d
}

fn next_bonus(pw: &Word, w: &Word, wp: u32, nextw: &Word, seg: &Segment) -> f64 {
    let np = seg
        .table
        .table
        .get(&nextw.w)
        .map(|e| e.p)
        .unwrap_or(nextw.pos());
    let mut d = 0.0;
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
        d += 1.5;
        temp_ok = false;
    } else if pw.pos() != 0 && wp & POSTAG::D_C != 0 {
        let p = pw.pos() & np;
        if pw.pos() == np {
            d += 1.0;
            temp_ok = false;
        } else if p != 0 {
            d += 0.25;
            temp_ok = false;
            if p & POSTAG::D_N != 0 {
                d += 0.75;
            }
        }
    }
    if temp_ok && wp & POSTAG::D_R != 0 && np & POSTAG::D_P != 0 {
        d += 1.0;
        temp_ok = false;
    }
    if temp_ok && np != 0 && wp & POSTAG::D_P != 0 {
        if np & POSTAG::A_NR != 0 && char_len(&nextw.w) > 1 {
            d += 1.0;
            if pw.w == "的" {
                d += 1.0;
                temp_ok = false;
            }
        }
    }
    if temp_ok && wp & POSTAG::D_P != 0 {
        if POSTAG::any(pw.pos(), &[POSTAG::D_N]) && POSTAG::any(np, &[POSTAG::D_N, POSTAG::D_V]) {
            d += 1.0;
        } else if POSTAG::any(pw.pos(), &[POSTAG::D_R]) && POSTAG::any(np, &[POSTAG::D_R]) {
            d += 0.5;
        }
    }
    if nextw.w == "后"
        && wp & POSTAG::D_T != 0
        && POSTAG::any(pw.pos(), &[POSTAG::D_MQ, POSTAG::A_M])
    {
        d += 1.0;
    } else if (nextw.w == "后" || nextw.w == "後") && POSTAG::any(wp, &[POSTAG::D_F]) {
        d += 1.0;
    } else if (w.w == "后" || w.w == "後")
        && POSTAG::any(pw.pos(), &[POSTAG::D_F])
        && POSTAG::any(np, &[POSTAG::D_N])
    {
        d += 1.0;
    }
    d
}

fn last_bonus(pw: &Word, wp: u32) -> f64 {
    if wp & POSTAG::D_F != 0 && POSTAG::any(pw.pos(), &[POSTAG::D_N]) {
        1.0
    } else {
        0.0
    }
}

pub(crate) fn get_tops(assess: &[Assess]) -> usize {
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

#[cfg(test)]
struct MaxStatesOverride {
    prev: usize,
}

#[cfg(test)]
impl MaxStatesOverride {
    fn set(cap: usize) -> Self {
        let prev = MAX_STATES_OVERRIDE.with(|c| {
            let p = c.get();
            c.set(cap);
            p
        });
        Self { prev }
    }
}

#[cfg(test)]
impl Drop for MaxStatesOverride {
    fn drop(&mut self) {
        MAX_STATES_OVERRIDE.with(|c| c.set(self.prev));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MaxStatesOverride, DEFAULT_MAX_STATES_PER_POS, MAX_SLOT_SEEN, PRUNE_EVENTS,
    };
    use crate::{DoSegmentOptions, Segment};
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    fn seg() -> Segment {
        Segment::with_novel_default().expect("load dict")
    }

    #[test]
    fn short_phrases_keep_expected_words() {
        let seg = seg();
        let samples = [
            "里面",
            "头发",
            "今天天气很好",
            "皇后面对表面",
            "几率",
            "这是一个中文分词模组",
        ];
        for sample in samples {
            let words = seg.do_segment_simple(sample, DoSegmentOptions::default());
            assert!(!words.is_empty(), "{sample}");
            assert_eq!(
                words.concat().chars().count(),
                sample.chars().count(),
                "{sample} tokens={words:?}"
            );
        }
    }

    #[test]
    fn long_unpunctuated_cjk_is_polynomial() {
        let seg = seg();
        let clause = "里面开发头发软件皇后面对表面今天天气很好几率日志面包";
        let text = clause.repeat(40);
        let started = Instant::now();
        let words = seg.do_segment_simple(&text, DoSegmentOptions::default());
        let elapsed = started.elapsed();
        assert_eq!(words.concat(), text);
        assert!(
            elapsed.as_secs_f64() < 2.0,
            "dict path DP too slow: {elapsed:?}"
        );
    }

    #[test]
    fn long_span_with_non_bmp_still_segments() {
        use crate::pipeline::{FOREIGN_TOKENIZER, ZHT_SYNONYM_OPTIMIZER};
        use crate::text::char_len;

        let seg = seg();
        // Without ForeignTokenizer, non-BMP stays inside the DictTokenizer span.
        let opts = DoSegmentOptions {
            disable_modules: vec![
                FOREIGN_TOKENIZER.to_string(),
                ZHT_SYNONYM_OPTIMIZER.to_string(),
            ],
            convert_synonym: Some(false),
            ..DoSegmentOptions::default()
        };
        let cjk = "今天天气很好里面开发头发软件皇后";
        let text = format!("{cjk}😀{cjk}");
        assert!(
            char_len(&text) > crate::tokenizers::DICT_ENUM_MAX_SPAN,
            "fixture must take the long path"
        );
        let words = seg.do_segment_simple(&text, opts);
        assert!(
            words.len() > 1,
            "long path must not collapse to one blob: {words:?}"
        );
        assert_eq!(words.concat(), text);
    }

    #[test]
    fn beam_width_quality_vs_time() {
        let seg = seg();
        let opts = DoSegmentOptions {
            convert_synonym: Some(false),
            ..DoSegmentOptions::default()
        };
        let mut text = String::new();
        for i in 0..40u32 {
            text.push_str(
                "里面开发头发软件皇后面对表面今天天气很好几率日志面包我人在板桥车站这边转车刚才赶快用手机看转播",
            );
            text.push(char::from_u32(0x4e00 + i).expect("cjk"));
        }
        let mut previous: Option<String> = None;
        let mut at_default: Option<String> = None;
        let mut at_high: Option<String> = None;
        for cap in [16usize, 32, 64, 128, 256] {
            let _guard = MaxStatesOverride::set(cap);
            PRUNE_EVENTS.store(0, Ordering::Relaxed);
            MAX_SLOT_SEEN.store(0, Ordering::Relaxed);
            let started = Instant::now();
            let words = seg.do_segment_simple(&text, opts.clone());
            let elapsed = started.elapsed();
            let joined = words.concat();
            let prunes = PRUNE_EVENTS.load(Ordering::Relaxed);
            let max_slot = MAX_SLOT_SEEN.load(Ordering::Relaxed);
            let same = previous.as_ref().is_none_or(|old| old == &joined);
            eprintln!(
                "cap={cap} time={elapsed:?} prunes={prunes} max_slot={max_slot} tokens={} same_as_smaller={same}",
                words.len()
            );
            if cap >= 64 {
                assert!(
                    same,
                    "cap={cap} changed tokenization vs smaller beam on fixture"
                );
            }
            if cap == DEFAULT_MAX_STATES_PER_POS {
                at_default = Some(joined.clone());
            }
            if cap == 256 {
                at_high = Some(joined.clone());
            }
            previous = Some(joined);
        }
        assert_eq!(
            at_default.as_ref(),
            at_high.as_ref(),
            "default beam ({DEFAULT_MAX_STATES_PER_POS}) must match high beam (256) on fixture"
        );
    }
}
