//! Traditional-Chinese synonym optimizer (port of `ZhtSynonymOptimizer.ts`).

use crate::colors::COLOR_HAIR;
use crate::pipeline::{splice_created, Optimizer, ZHT_SYNONYM_OPTIMIZER};
use crate::postag::POSTAG;
use crate::segment::Segment;
use crate::text::char_len;
use crate::word::Word;
use once_cell::sync::Lazy;
use regex::Regex;

const CLOSE_P: &[&str] = &["】", "」", "》", "』", "］", "’", "”", "〉"];
const SEP_P: &[&str] = &["、", ",", "…"];

fn hex_and_any(p: u32, flags: &[u32]) -> bool {
    flags.iter().any(|f| p & f != 0)
}

fn get_synonym(seg: &Segment, w: &str, mut nw: String) -> String {
    if let Some(s) = seg.synonym.get(w) {
        nw = s.to_string();
    }
    if let Some(s) = seg.synonym.get(&nw) {
        nw = s.to_string();
    }
    nw
}

fn replace_li(s: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(.)里|里(.)").unwrap());
    RE.replace_all(s, |caps: &regex::Captures| {
        let a = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let b = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("{a}裡{b}")
    })
    .into_owned()
}

fn replace_hou(s: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(.)后|后(.)").unwrap());
    RE.replace_all(s, |caps: &regex::Captures| {
        let a = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let b = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("{a}後{b}")
    })
    .into_owned()
}

fn replace_shen(s: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"蔘(.)").unwrap());
    RE.replace_all(s, "參$1").into_owned()
}

pub struct ZhtSynonymOptimizer;

impl Optimizer for ZhtSynonymOptimizer {
    fn name(&self) -> &'static str {
        ZHT_SYNONYM_OPTIMIZER
    }

    fn do_optimize(&self, mut words: Vec<Word>, seg: &Segment) -> Vec<Word> {
        let mut i = 0;
        while i < words.len() {
            if seg.blacklist_synonym.contains(&words[i].w) {
                i += 1;
                continue;
            }
            let w0 = if i > 0 { Some(words[i - 1].clone()) } else { None };
            let w2 = words.get(i + 1).cloned();
            let w1_len = char_len(&words[i].w);
            let mut bool_ok = false;
            let mut new_p: Option<u32> = None;

            if w1_len == 1 {
                let w = words[i].w.clone();
                if w == "里" {
                    if w0.as_ref().map(|p| p.w.ends_with('的') || p.w == "和").unwrap_or(false) {
                        // keep
                    } else if w0.as_ref().map(|p| CLOSE_P.contains(&p.w.as_str())).unwrap_or(false)
                        || w0.as_ref().map(|p| {
                            hex_and_any(p.pos(), &[POSTAG::D_N, POSTAG::D_S, POSTAG::D_F, POSTAG::D_T, POSTAG::D_V])
                        }).unwrap_or(false)
                    {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "裡".into()));
                        bool_ok = true;
                    }
                } else if w == "后" {
                    if w0.as_ref().map(|p| p.w == "和").unwrap_or(false) {
                        // keep
                    } else if w0.as_ref().map(|p| CLOSE_P.contains(&p.w.as_str())).unwrap_or(false)
                        || w0.as_ref().map(|p| p.w == "腰").unwrap_or(false)
                        || w0.as_ref().map(|p| {
                            p.p.is_some()
                                && p.pos() != 0
                                && hex_and_any(
                                    p.pos(),
                                    &[
                                        POSTAG::D_V,
                                        POSTAG::D_S,
                                        POSTAG::D_T,
                                        POSTAG::D_N,
                                        POSTAG::D_MQ,
                                        POSTAG::A_M,
                                        POSTAG::D_F,
                                        POSTAG::D_D,
                                        POSTAG::D_R,
                                    ],
                                )
                        }).unwrap_or(false)
                        || w2.as_ref().map(|n| n.p.is_some() && hex_and_any(n.pos(), &[POSTAG::D_V])).unwrap_or(false)
                        || (w2.is_some()
                            && w0.is_some()
                            && w0.as_ref().unwrap().pos_falsy()
                            && w2.as_ref().map(|n| n.p.is_some() && hex_and_any(n.pos(), &[POSTAG::D_D])).unwrap_or(false))
                        || (w2.is_some()
                            && w0.as_ref().map(|p| p.pos_falsy()).unwrap_or(true)
                            && w2.as_ref().map(|n| SEP_P.contains(&n.w.as_str())).unwrap_or(false))
                    {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "後".into()));
                        bool_ok = true;
                    }
                } else if w == "发" || w == "發" {
                    if let Some(prev) = &w0 {
                        if COLOR_HAIR.contains(&prev.w) {
                            let nw = get_synonym(seg, &w, "髮".into());
                            if nw != w {
                                words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                                new_p = Some(POSTAG::D_N);
                                bool_ok = true;
                            }
                        }
                    }
                    if !bool_ok && words[i].w == "发" && w2.as_ref().map(|n| n.w == "的").unwrap_or(false) {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "發".into()));
                        bool_ok = true;
                    }
                    if !bool_ok
                        && words[i].w == "发"
                        && w0.as_ref().map(|p| p.pos() & POSTAG::D_R != 0).unwrap_or(false)
                        && w2.as_ref().map(|n| n.pos() & POSTAG::D_R != 0).unwrap_or(false)
                    {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "發".into()));
                        bool_ok = true;
                    }
                    if !bool_ok
                        && words[i].w == "发"
                        && w2.as_ref().map(|n| n.w == "那麼" || n.w == "那么").unwrap_or(false)
                    {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "發".into()));
                        bool_ok = true;
                    }
                } else if w == "于" {
                    let start_ok = w0.as_ref().map(|p| p.pos() & POSTAG::D_W != 0).unwrap_or(true);
                    if start_ok
                        && w2.as_ref().map(|n| {
                            n.p.is_some()
                                && n.pos() != 0
                                && hex_and_any(
                                    n.pos(),
                                    &[
                                        POSTAG::D_N,
                                        POSTAG::D_V,
                                        POSTAG::D_R,
                                        POSTAG::D_D,
                                        POSTAG::D_T,
                                        POSTAG::A_NR,
                                        POSTAG::D_S,
                                        POSTAG::D_F,
                                    ],
                                )
                        }).unwrap_or(false)
                    {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "於".into()));
                        new_p = Some(POSTAG::D_P);
                        words[i].p = new_p;
                        bool_ok = true;
                    } else if let (Some(prev), Some(next)) = (&w0, &w2) {
                        let cond = (hex_and_any(prev.pos(), &[POSTAG::D_V, POSTAG::D_R, POSTAG::D_A, POSTAG::D_T, POSTAG::D_F])
                            && hex_and_any(next.pos(), &[POSTAG::D_N, POSTAG::D_V, POSTAG::D_R, POSTAG::D_S, POSTAG::A_NX, POSTAG::D_F, POSTAG::D_W]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_N]) && hex_and_any(next.pos(), &[POSTAG::D_N]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_V, POSTAG::D_N])
                                && hex_and_any(next.pos(), &[POSTAG::D_F, POSTAG::D_T, POSTAG::A_NR, POSTAG::D_R, POSTAG::D_S, POSTAG::D_W]))
                            || (hex_and_any(prev.pos(), &[POSTAG::A_NS, POSTAG::D_T, POSTAG::D_C])
                                && hex_and_any(next.pos(), &[POSTAG::A_NS, POSTAG::D_T]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_D]) && hex_and_any(next.pos(), &[POSTAG::D_N]))
                            || (hex_and_any(prev.pos(), &[POSTAG::A_NR])
                                && hex_and_any(next.pos(), &[POSTAG::A_NS, POSTAG::A_NT, POSTAG::D_S, POSTAG::D_N, POSTAG::D_V]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_V]) && hex_and_any(next.pos(), &[POSTAG::D_W]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_D]) && hex_and_any(next.pos(), &[POSTAG::D_V]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_V]) && hex_and_any(next.pos(), &[POSTAG::D_D]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_N]) && hex_and_any(next.pos(), &[POSTAG::D_V]))
                            || (hex_and_any(prev.pos(), &[POSTAG::D_D]) && hex_and_any(next.pos(), &[POSTAG::D_F]));
                        if cond {
                            words[i].ow = Some(std::mem::replace(&mut words[i].w, "於".into()));
                            new_p = Some(POSTAG::D_P);
                            words[i].p = new_p;
                            bool_ok = true;
                        } else if let Some(w3) = words.get(i + 2) {
                            if prev.pos() & POSTAG::D_V != 0
                                && next.pos() & POSTAG::D_D != 0
                                && w3.pos() & POSTAG::D_V != 0
                            {
                                words[i].ow = Some(std::mem::replace(&mut words[i].w, "於".into()));
                                new_p = Some(POSTAG::D_P);
                                words[i].p = new_p;
                                bool_ok = true;
                            }
                        }
                    }
                    if !bool_ok && w2.as_ref().map(|n| n.pos() & POSTAG::D_T != 0).unwrap_or(false) {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "於".into()));
                        new_p = Some(POSTAG::D_P);
                        words[i].p = new_p;
                        bool_ok = true;
                    }
                } else if w == "么" {
                    if w2.as_ref().map(|n| n.pos() & POSTAG::D_W != 0).unwrap_or(true) {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, "麼".into()));
                        bool_ok = true;
                    }
                } else if w == "余" {
                    if w2.as_ref().map(|n| n.w == "力").unwrap_or(false)
                        && words.get(i + 2).map(|n| n.pos() & POSTAG::D_W != 0).unwrap_or(false)
                    {
                        let nw = format!("{}{}", words[i].w, words[i + 1].w);
                        let (p, f) = seg
                            .table
                            .table
                            .get(&nw)
                            .map(|e| (e.p, Some(e.f)))
                            .unwrap_or((0x101000, None));
                        let mut tok = Word::new(nw).with_p(p);
                        tok.f = f;
                        splice_created(&mut words, i, 2, tok, seg);
                        continue;
                    }
                }
            } else if w1_len > 1 {
                let w = words[i].w.clone();
                if let Some(caps) = Regex::new(r"^(.+)[发發]$").unwrap().captures(&w) {
                    let c = caps.get(1).unwrap().as_str().to_string();
                    if COLOR_HAIR.contains(&c) {
                        let nw = get_synonym(seg, &w, format!("{c}髮"));
                        if nw != w {
                            words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                            bool_ok = true;
                        }
                    } else if w == format!("{c}发") && words[i].pos() & POSTAG::D_MQ != 0 {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, format!("{c}發")));
                        bool_ok = true;
                    } else if w == format!("{c}发")
                        && (w0.is_none() || w0.as_ref().map(|p| p.pos() == POSTAG::D_W).unwrap_or(false))
                    {
                        let nw = format!("{c}髮");
                        if let Some(ow) = seg.table.table.get(&nw) {
                            if ow.s {
                                words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                                new_p = Some(ow.p);
                                bool_ok = true;
                            }
                        }
                    }
                } else if words[i].pos() & POSTAG::D_MQ != 0
                    && Regex::new(r"^(.+)余$").unwrap().is_match(&w)
                {
                    let nw = Regex::new(r"^(.+)余$").unwrap().replace(&w, "${1}餘").into_owned();
                    words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                    bool_ok = true;
                } else if words[i].pos() & POSTAG::D_MQ != 0
                    && w.starts_with('几')
                    && words[i].m.as_ref().map(|m| m.len() > 1).unwrap_or(false)
                {
                    let nw = w.replacen('几', "幾", 1);
                    words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                    bool_ok = true;
                } else if words[i].pos() & POSTAG::BAD != 0 {
                    let mut nw = replace_li(&w);
                    nw = replace_hou(&nw);
                    nw = replace_shen(&nw);
                    nw = get_synonym(seg, &w, nw);
                    if nw != w {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                        bool_ok = true;
                    }
                } else if words[i].pos() & POSTAG::D_F != 0 {
                    let mut nw = replace_li(&w);
                    nw = replace_hou(&nw);
                    nw = get_synonym(seg, &w, nw);
                    if nw != w {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                        bool_ok = true;
                    }
                } else if words[i].pos() & POSTAG::D_S != 0 {
                    let nw = Regex::new(r"(.)里$").unwrap().replace(&w, "${1}裡").into_owned();
                    let nw = get_synonym(seg, &w, nw);
                    if nw != w {
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                        bool_ok = true;
                    }
                } else if words[i].pos() & POSTAG::D_T != 0 || words[i].pos() & POSTAG::D_V != 0 {
                    let nw = replace_hou(&w);
                    let nw = get_synonym(seg, &w, nw);
                    if nw != w {
                        words[i].op = words[i].op.or(words[i].p);
                        words[i].ow = Some(std::mem::replace(&mut words[i].w, nw));
                        bool_ok = true;
                    }
                }
            }

            if bool_ok {
                if let Some(ow_s) = words[i].ow.clone() {
                    if ow_s != words[i].w {
                        if let Some(e) = seg.table.table.get(&words[i].w) {
                            if let Some(np) = new_p {
                                words[i].op = words[i].op.or(Some(e.p));
                                words[i].p = Some(np);
                            } else if e.p != words[i].pos() {
                                words[i].op = words[i].op.or(words[i].p);
                                words[i].p = Some(e.p);
                            }
                            if Some(e.s) != words[i].s {
                                words[i].os = words[i].os.or(words[i].s).or(Some(false));
                                words[i].s = Some(e.s);
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        words
    }
}
