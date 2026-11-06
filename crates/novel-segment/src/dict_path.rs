//! Long-span DictTokenizer search: beam-prune by path totals, then rank survivors
//! with the shared Assess / `get_tops`.

use crate::datetime::DATETIME;
use crate::names::{FAMILY_NAME_1, FAMILY_NAME_2};
use crate::postag::POSTAG;
use crate::segment::Segment;
use crate::text::{char_len, hit_text, leading_repeat_run_units, utf16_byte_of};
use crate::trie::DictHit;
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
    hit: DictHit,
    p: u32,
    in_dict: bool,
    prev_hit: Option<DictHit>,
    prev_p: u32,
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

pub(crate) fn best_path(
    wordpos: &[Vec<DictHit>],
    text: &str,
    units: &[u16],
    preword: Option<&Word>,
    seg: &Segment,
) -> Vec<Word> {
    best_path_with_cap(wordpos, text, units, preword, seg, resolve_max_states())
}

fn best_path_with_cap(
    wordpos: &[Vec<DictHit>],
    text: &str,
    units: &[u16],
    preword: Option<&Word>,
    seg: &Segment,
    cap: usize,
) -> Vec<Word> {
    let n = units.len();
    if n == 0 {
        return Vec::new();
    }
    let byte_of = utf16_byte_of(text);

    let mut nodes: Vec<Node> = vec![Node {
        hit: DictHit {
            start: 0,
            len: 0,
            f: 0.0,
        },
        p: 0,
        in_dict: false,
        prev_hit: None,
        prev_p: preword.map(|w| w.pos()).unwrap_or(0),
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
        let outgoing = outgoing(wordpos, units, pos);
        let slice = outgoing.as_slice();
        if slice.is_empty() {
            continue;
        }
        let n_at = at[pos].len();
        for k in 0..n_at {
            let ni = at[pos][k];
            for &cand in slice {
                let end = pos + cand.len;
                if end > n || cand.len == 0 {
                    continue;
                }
                let child = extend(
                    &nodes[ni],
                    cand,
                    preword,
                    text,
                    units,
                    &byte_of,
                    seg,
                    ni,
                );
                insert_node(&mut nodes, &mut at[end], child, cap);
            }
        }
    }

    let complete: Vec<usize> = at[n].iter().copied().filter(|&i| !nodes[i].dummy).collect();
    if complete.is_empty() {
        return Vec::new();
    }

    let mut assess = Vec::with_capacity(complete.len());
    for &i in &complete {
        assess.push(assess_node(&nodes[i], n));
    }
    reconstruct(&nodes, complete[get_tops_beam(&assess, n)], text, units, &byte_of)
}

enum Outgoing<'a> {
    One(DictHit),
    Bucket(&'a [DictHit]),
}

impl<'a> Outgoing<'a> {
    fn as_slice(&self) -> &[DictHit] {
        match self {
            Outgoing::One(h) => std::slice::from_ref(h),
            Outgoing::Bucket(s) => s,
        }
    }
}

fn outgoing<'a>(wordpos: &'a [Vec<DictHit>], units: &[u16], pos: usize) -> Outgoing<'a> {
    if pos < units.len() {
        if let Some(run) = leading_repeat_run_units(&units[pos..]) {
            return Outgoing::One(DictHit {
                start: pos,
                len: run,
                f: 0.0,
            });
        }
    }
    match wordpos.get(pos) {
        Some(v) if !v.is_empty() => Outgoing::Bucket(v),
        _ if pos < units.len() => Outgoing::One(DictHit {
            start: pos,
            len: 1,
            f: 0.0,
        }),
        _ => Outgoing::Bucket(&[]),
    }
}

fn extend(
    parent: &Node,
    cand: DictHit,
    preword: Option<&Word>,
    text: &str,
    units: &[u16],
    byte_of: &[usize],
    seg: &Segment,
    parent_i: usize,
) -> Node {
    let len = cand.len as f64;
    let cand_w = hit_text(text, byte_of, units, cand.start, cand.len);
    let cand_w = cand_w.as_ref();
    let mut sum_f = parent.sum_f;
    let mut c = parent.c;
    let mut d = parent.d;
    let mut has_d_v = parent.has_d_v;

    let parent_w = if parent.dummy {
        None
    } else {
        Some(hit_text(
            text,
            byte_of,
            units,
            parent.hit.start,
            parent.hit.len,
        ))
    };

    if !parent.dummy && parent.in_dict {
        let parent_s = parent_w.as_deref().unwrap_or("");
        let pp_cow;
        let pp_s: Option<&str> = if let Some(pp) = parent.prev_hit {
            pp_cow = hit_text(text, byte_of, units, pp.start, pp.len);
            Some(pp_cow.as_ref())
        } else {
            preword.map(|w| w.w.as_str())
        };
        if let Some(pp_s) = pp_s {
            d += next_bonus(
                pp_s,
                parent.prev_p,
                parent_s,
                parent.p,
                cand_w,
                0,
                cand.len,
                seg,
            );
        }
    }

    let (in_dict, wp, freq) = if let Some(e) = seg.table.table.get(cand_w) {
        (true, e.p, cand.f)
    } else {
        (false, 0, 0.0)
    };
    if in_dict {
        sum_f += freq;
        if parent.dummy && preword.is_none() && wp & POSTAG::D_V != 0 {
            has_d_v = true;
        }
        let (pw_s, pw_p) = if parent.dummy {
            (
                preword.map(|w| w.w.as_str()),
                preword.map(|w| w.pos()).unwrap_or(0),
            )
        } else {
            (parent_w.as_deref(), parent.p)
        };
        if let Some(pw_s) = pw_s {
            d += pair_bonus(pw_s, pw_p, cand_w, wp, &mut has_d_v);
        }
    } else {
        c += 1.0;
    }

    Node {
        hit: cand,
        p: wp,
        in_dict,
        prev_hit: if parent.dummy { None } else { Some(parent.hit) },
        prev_p: if parent.dummy {
            preword.map(|w| w.pos()).unwrap_or(0)
        } else {
            parent.p
        },
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
    let mut dominated = false;
    let mut drop: Vec<usize> = Vec::new();
    for (idx, &existing) in slot.iter().enumerate() {
        let e = &nodes[existing];
        if e.hit.start != child.hit.start
            || e.hit.len != child.hit.len
            || e.prev_hit.map(|h| (h.start, h.len)) != child.prev_hit.map(|h| (h.start, h.len))
            || e.has_d_v != child.has_d_v
            || e.x != child.x
        {
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

#[allow(dead_code)]
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
                row.d += pair_bonus(&pw.w, pw.pos(), &w.w, wp, &mut has_d_v);
                if let Some(nextw) = chunk.get(j + 1) {
                    row.d += next_bonus(
                        &pw.w,
                        pw.pos(),
                        &w.w,
                        wp,
                        &nextw.w,
                        nextw.pos(),
                        char_len(&nextw.w),
                        seg,
                    );
                } else {
                    row.d += last_bonus(pw.pos(), wp);
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

/// Score a short-span hit path without allocating `Word` strings first.
///
/// `span_len` is the UTF-16 length of the span being scored (the whole short
/// span, or a refine window).
pub(crate) fn assess_hits(
    chunk: &[DictHit],
    preword: Option<&Word>,
    text: &str,
    units: &[u16],
    byte_of: &[usize],
    span_len: usize,
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
    let parts: Vec<_> = chunk
        .iter()
        .map(|h| hit_text(text, byte_of, units, h.start, h.len))
        .collect();
    let sp = span_len as f64 / chunk.len() as f64;
    let mut has_d_v = false;
    let mut pre_w: Option<&str> = preword.map(|w| w.w.as_str());
    let mut pre_p = preword.map(|w| w.pos()).unwrap_or(0);
    for (j, h) in chunk.iter().enumerate() {
        let w = parts[j].as_ref();
        if let Some(e) = seg.table.table.get(w) {
            let wp = e.p;
            row.a += h.f;
            if j == 0 && preword.is_none() && wp & POSTAG::D_V != 0 {
                has_d_v = true;
            }
            if let Some(pw) = pre_w {
                row.d += pair_bonus(pw, pre_p, w, wp, &mut has_d_v);
                if let Some(next) = parts.get(j + 1) {
                    let next_w = next.as_ref();
                    let next_p = seg
                        .table
                        .table
                        .get(next_w)
                        .map(|e| e.p)
                        .unwrap_or(0);
                    row.d += next_bonus(
                        pw,
                        pre_p,
                        w,
                        wp,
                        next_w,
                        next_p,
                        chunk[j + 1].len,
                        seg,
                    );
                } else {
                    row.d += last_bonus(pre_p, wp);
                }
            }
            pre_w = Some(w);
            pre_p = wp;
        } else {
            row.c += 1.0;
            pre_w = Some(w);
            pre_p = 0;
        }
        row.b += (sp - h.len as f64).powi(2);
    }
    if !has_d_v {
        row.d -= 0.5;
    }
    row.a /= chunk.len() as f64;
    row.b /= chunk.len() as f64;
    row
}

fn assess_node(n: &Node, span: usize) -> Assess {
    let x = n.x as f64;
    if x == 0.0 {
        return Assess {
            x: 0.0,
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
        };
    }
    let mut d = n.d;
    if n.in_dict {
        d += last_bonus(n.prev_p, n.p);
    }
    if !n.has_d_v {
        d -= 0.5;
    }
    let sp = span as f64 / x;
    Assess {
        x,
        a: n.sum_f / x,
        b: n.sum_len2 / x - sp * sp,
        c: n.c,
        d,
    }
}

fn reconstruct(
    nodes: &[Node],
    i: usize,
    text: &str,
    units: &[u16],
    byte_of: &[usize],
) -> Vec<Word> {
    let mut out = Vec::new();
    reconstruct_into(nodes, i, text, units, byte_of, &mut out);
    out
}

fn reconstruct_into(
    nodes: &[Node],
    mut i: usize,
    text: &str,
    units: &[u16],
    byte_of: &[usize],
    out: &mut Vec<Word>,
) {
    out.clear();
    loop {
        let n = &nodes[i];
        if n.dummy {
            break;
        }
        let mut w = Word::new(hit_text(text, byte_of, units, n.hit.start, n.hit.len).into_owned())
            .with_c(n.hit.start)
            .with_f(n.hit.f);
        if n.in_dict {
            w.p = Some(n.p);
        }
        out.push(w);
        match n.parent {
            Some(p) => i = p,
            None => break,
        }
    }
    out.reverse();
}

fn pair_bonus(pw_w: &str, pw_p: u32, w: &str, wp: u32, has_d_v: &mut bool) -> f64 {
    let mut d = 0.0;
    if pw_w == "年" && matches!(w, "历史" | "歷史" | "歴史") {
        d += 0.5;
    }
    if pw_p & POSTAG::A_M != 0 && (wp & POSTAG::A_Q != 0 || DATETIME.contains(w)) {
        d += 1.0;
    }
    if wp & POSTAG::D_V != 0 {
        *has_d_v = true;
        if pw_p & POSTAG::D_D != 0 {
            d += 1.0;
        }
    }
    if (pw_p & POSTAG::A_NS != 0 || pw_p & POSTAG::A_NT != 0 || pw_p & POSTAG::D_A != 0)
        && (wp & POSTAG::D_N != 0
            || wp & POSTAG::A_NR != 0
            || wp & POSTAG::A_NS != 0
            || wp & POSTAG::A_NZ != 0
            || wp & POSTAG::A_NT != 0)
    {
        d += 1.0;
    }
    if pw_p & POSTAG::D_F != 0 && (wp & POSTAG::A_M != 0 || wp & POSTAG::D_MQ != 0) {
        d += 1.0;
    }
    if (FAMILY_NAME_1.contains(pw_w) || FAMILY_NAME_2.contains(pw_w))
        && (wp & POSTAG::D_N != 0 || wp & POSTAG::A_NZ != 0)
    {
        d += 1.0;
    }
    if POSTAG::any(pw_p, &[POSTAG::D_S, POSTAG::A_NS]) && POSTAG::any(wp, &[POSTAG::D_F]) {
        d += 0.5;
    }
    d
}

fn next_bonus(
    pw_w: &str,
    pw_p: u32,
    w: &str,
    wp: u32,
    next_w: &str,
    next_p_fallback: u32,
    next_len: usize,
    seg: &Segment,
) -> f64 {
    let np = seg
        .table
        .table
        .get(next_w)
        .map(|e| e.p)
        .unwrap_or(next_p_fallback);
    let mut d = 0.0;
    let mut temp_ok = true;
    if (w == "的" || w == "之")
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
    } else if pw_p != 0 && wp & POSTAG::D_C != 0 {
        let p = pw_p & np;
        if pw_p == np {
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
        if np & POSTAG::A_NR != 0 && next_len > 1 {
            d += 1.0;
            if pw_w == "的" {
                d += 1.0;
                temp_ok = false;
            }
        }
    }
    if temp_ok && wp & POSTAG::D_P != 0 {
        if POSTAG::any(pw_p, &[POSTAG::D_N]) && POSTAG::any(np, &[POSTAG::D_N, POSTAG::D_V]) {
            d += 1.0;
        } else if POSTAG::any(pw_p, &[POSTAG::D_R]) && POSTAG::any(np, &[POSTAG::D_R]) {
            d += 0.5;
        }
    }
    if next_w == "后"
        && wp & POSTAG::D_T != 0
        && POSTAG::any(pw_p, &[POSTAG::D_MQ, POSTAG::A_M])
    {
        d += 1.0;
    } else if (next_w == "后" || next_w == "後") && POSTAG::any(wp, &[POSTAG::D_F]) {
        d += 1.0;
    } else if (w == "后" || w == "後")
        && POSTAG::any(wp, &[POSTAG::D_F])
        && POSTAG::any(np, &[POSTAG::D_N])
    {
        d += 1.0;
    }
    d
}

fn last_bonus(pw_p: u32, wp: u32) -> f64 {
    if wp & POSTAG::D_F != 0 && POSTAG::any(pw_p, &[POSTAG::D_N]) {
        1.0
    } else {
        0.0
    }
}

fn assess_extrema(assess: &[Assess]) -> Assess {
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
    top
}

fn score_against(ass: &Assess, top: &Assess) -> f64 {
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
    s
}

fn pick_tops(assess: &[Assess], top: &Assess) -> usize {
    let tops: Vec<f64> = assess.iter().map(|ass| score_against(ass, top)).collect();
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

/// Rank beam survivors against a fixed unigram envelope, not the survivor set.
///
/// Full `get_chunks` always includes unigrams (`x = n`, `b = 0`). Unique max-`a`
/// among survivors is set-dependent and can flip a near-tie; it is omitted here.
fn get_tops_beam(assess: &[Assess], n: usize) -> usize {
    if assess.is_empty() {
        return 0;
    }
    let top = Assess {
        x: n as f64,
        a: f64::INFINITY,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    pick_tops(assess, &top)
}

pub(crate) fn get_tops(assess: &[Assess]) -> usize {
    if assess.is_empty() {
        return 0;
    }
    pick_tops(assess, &assess_extrema(assess))
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
        Assess, MaxStatesOverride, DEFAULT_MAX_STATES_PER_POS, MAX_SLOT_SEEN, PRUNE_EVENTS,
    };
    use crate::{DoSegmentOptions, Segment};
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    fn seg() -> Segment {
        Segment::with_novel_default().expect("load dict")
    }

    #[test]
    fn long_span_keeps_chongshangqian_and_faqi() {
        let seg = seg();
        let text = "之前一直觉得自己碍事的外围士兵们不顾死活地冲上前发起进攻。";
        assert!(
            crate::text::char_len(text) > crate::tokenizers::DICT_ENUM_MAX_SPAN,
            "fixture must take the long path"
        );
        let words = seg.do_segment_simple(text, DoSegmentOptions::convert_synonym());
        assert!(
            words.iter().any(|w| w == "衝上前"),
            "expected token 衝上前; tokens={words:?}"
        );
        assert!(
            words.iter().any(|w| w == "發起"),
            "expected token 發起; tokens={words:?}"
        );
    }

    #[test]
    fn long_span_unigram_refine_merges_najiushi() {
        let seg = seg();
        let text = "「難道又是老樣子藏在了大自然的險要之地里？如果是這樣的話那就是南洋的海底或者是世界盡頭之壁了啊」";
        assert!(
            crate::text::char_len(text) > crate::tokenizers::DICT_ENUM_MAX_SPAN,
            "fixture must take the long path"
        );
        let words = seg.do_segment_simple(text, DoSegmentOptions::default());
        assert!(
            words.iter().any(|w| w == "那就是"),
            "unigram refine should merge 那/就是; tokens={words:?}"
        );
    }

    #[test]
    fn beam_unigram_envelope_prefers_fewer_words() {
        let chong = Assess {
            x: 15.0,
            a: 1304.93,
            b: 0.2489,
            c: 0.0,
            d: 1.0,
        };
        let qianfa = Assess {
            x: 16.0,
            a: 1289.75,
            b: 0.1875,
            c: 0.0,
            d: 2.0,
        };
        let high_a = Assess {
            x: 17.0,
            a: 1448.70,
            b: 0.2284,
            c: 0.0,
            d: 2.0,
        };
        let assess = [chong, qianfa, high_a];
        assert_eq!(super::get_tops(&assess), 1, "raw get_tops keeps the min-b path");
        assert_eq!(
            super::get_tops_beam(&assess, 28),
            0,
            "unigram envelope restores the fewer-word path"
        );
    }

    #[test]
    fn beam_ranking_ignores_survivor_extrema() {
        // Longer path uniquely has max-a and min-b among survivors. Raw get_tops
        // awards both +1 and prefers it; the fixed envelope does not.
        let few = Assess {
            x: 15.0,
            a: 1000.0,
            b: 0.30,
            c: 0.0,
            d: 2.0,
        };
        let long = Assess {
            x: 16.0,
            a: 2000.0,
            b: 0.20,
            c: 0.0,
            d: 2.0,
        };
        let assess = [few, long];
        assert_eq!(super::get_tops(&assess), 1);
        assert_eq!(super::get_tops_beam(&assess, 28), 0);
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
