//! Shared CLI / API / MCP segmentation helpers.

use crate::options::DoSegmentOptions;
use crate::segment::Segment;
use crate::word::{stringify, Word};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentMode {
    /// API / file-process profile (`autoCjk` + `all_mod` + `convertSynonym`).
    #[default]
    Novel,
    /// MCP / test-CLI profile (`nodeNovelMode` synonym files).
    NodeNovel,
}

fn novel_segment() -> &'static Mutex<Segment> {
    static S: OnceLock<Mutex<Segment>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Segment::with_novel_default().expect("load novel default")))
}

fn node_novel_segment() -> &'static Mutex<Segment> {
    static S: OnceLock<Mutex<Segment>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(Segment::with_node_novel_default().expect("load node novel default"))
    })
}

pub fn segment_words(text: &str, mode: SegmentMode, opts: DoSegmentOptions) -> Vec<Word> {
    let lock = match mode {
        SegmentMode::Novel => novel_segment(),
        SegmentMode::NodeNovel => node_novel_segment(),
    };
    let seg = lock.lock().expect("segment mutex");
    seg.do_segment(text, opts)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRequest {
    pub text: Option<String>,
    pub file: Option<String>,
    pub expected_full: Option<String>,
    pub expected_full_file: Option<String>,
    pub expected_contains: Option<Vec<ExpectedItem>>,
    pub expected_contains_not: Option<Vec<ExpectedItem>>,
    pub expected_index_of: Option<Vec<ExpectedItem>>,
    pub expected_index_of_not: Option<Vec<ExpectedItem>>,
    pub dict_entries: Option<Vec<Vec<Value>>>,
    pub synonym_entries: Option<Vec<Vec<String>>>,
    pub blacklist_words: Option<Vec<String>>,
    pub debug_each: Option<bool>,
    pub output_file: Option<String>,
    pub output_format: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedItem {
    One(String),
    Any(Vec<String>),
}

impl ExpectedItem {
    pub fn label(&self) -> String {
        match self {
            ExpectedItem::One(s) => s.clone(),
            ExpectedItem::Any(v) => v.join("/"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub changed: bool,
    pub match_results: MatchResults,
    pub result: Vec<Value>,
    pub output_text: String,
    pub output_words: Vec<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_failures: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResults {
    pub match_expected_full: Option<bool>,
    pub match_expected_contains: Option<bool>,
    pub match_expected_contains_not: Option<bool>,
    pub match_expected_index_of: Option<bool>,
    pub match_expected_index_of_not: Option<bool>,
}

pub fn apply_extras(
    dict_entries: &[Vec<Value>],
    synonym_entries: &[Vec<String>],
    blacklist: &[String],
) {
    let mut seg = node_novel_segment().lock().expect("segment mutex");
    for row in dict_entries {
        let spec = row
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if spec.is_empty() {
            continue;
        }
        let p = row.get(1).and_then(|v| {
            v.as_u64()
                .map(|n| n as u32)
                .or_else(|| v.as_f64().map(|n| n as u32))
        });
        let f = row.get(2).and_then(|v| v.as_f64());
        let _ = seg.add_word(&spec, p, f);
    }
    for row in synonym_entries {
        if row.len() < 2 {
            continue;
        }
        let refs: Vec<&str> = row[1..].iter().map(|s| s.as_str()).collect();
        seg.add_synonym(&row[0], &refs);
    }
    for w in blacklist {
        seg.add_blacklist(w);
    }
}

pub fn run_test(req: TestRequest) -> TestResult {
    let mut text = req.text.unwrap_or_default();
    if let Some(path) = req.file.as_deref() {
        match std::fs::read_to_string(path) {
            Ok(s) => text = s,
            Err(e) => {
                return error_result(format!("Failed to read input file: {e}"), e.to_string())
            }
        }
    }
    let mut expected_full = req.expected_full.clone();
    if let Some(path) = req.expected_full_file.as_deref() {
        match std::fs::read_to_string(path) {
            Ok(s) => expected_full = Some(s),
            Err(e) => {
                return error_result(format!("Failed to read expected file: {e}"), e.to_string())
            }
        }
    }
    if text.trim().is_empty() {
        return error_result(
            "No text provided for segmentation",
            "No text provided for segmentation",
        );
    }

    if let Some(rows) = req.dict_entries.as_deref() {
        apply_extras(rows, &[], &[]);
    }
    if let Some(rows) = req.synonym_entries.as_deref() {
        apply_extras(&[], rows, &[]);
    }
    if let Some(rows) = req.blacklist_words.as_deref() {
        apply_extras(&[], &[], rows);
    }

    let words = if req.debug_each.unwrap_or(false) {
        debug_each_segment(&text)
    } else {
        segment_words(&text, SegmentMode::NodeNovel, DoSegmentOptions::default())
    };
    let output_words: Vec<String> = words.iter().map(|w| w.w.clone()).collect();
    let output_text = stringify(&words);
    let changed = normalize_text(&output_text) != normalize_text(&text);

    let mut match_results = MatchResults::default();
    let mut failures = json!({});
    let mut diff = None;

    if let Some(exp) = expected_full.as_deref() {
        let ne = normalize_text(exp);
        let na = normalize_text(&output_text);
        let ok = ne == na;
        match_results.match_expected_full = Some(ok);
        if !ok {
            diff = Some(json!({
                "expected": ne,
                "actual": na,
                "positions": diff_positions(&ne, &na),
            }));
        }
    }

    if let Some(exp) = req.expected_contains.as_deref() {
        if !exp.is_empty() {
            let (ok, failed) = ordered_contains(&output_words, exp);
            match_results.match_expected_contains = Some(ok);
            if !ok {
                failures["contains"] = json!(failed);
            }
        }
    }
    if let Some(exp) = req.expected_contains_not.as_deref() {
        if !exp.is_empty() {
            let (hit, failed) = ordered_contains(&output_words, exp);
            match_results.match_expected_contains_not = Some(!hit);
            if hit {
                failures["containsNot"] = json!(failed);
            }
        }
    }
    if let Some(exp) = req.expected_index_of.as_deref() {
        if !exp.is_empty() {
            let (ok, failed) = index_of_all(&output_text, exp);
            match_results.match_expected_index_of = Some(ok);
            if !ok {
                failures["indexOf"] = json!(failed);
            }
        }
    }
    if let Some(exp) = req.expected_index_of_not.as_deref() {
        if !exp.is_empty() {
            let failed = index_of_not_failed(&output_text, exp);
            let ok = failed.is_empty();
            match_results.match_expected_index_of_not = Some(ok);
            if !ok {
                failures["indexOfNot"] = json!(failed);
            }
        }
    }

    let success = calculate_success(&match_results, changed);
    let message = build_message(&match_results, changed);
    let match_failures = if failures.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        Some(failures)
    } else {
        None
    };

    let result = TestResult {
        success,
        changed,
        match_results,
        result: words
            .iter()
            .map(|w| json!({"w": w.w, "p": w.p, "f": w.f}))
            .collect(),
        output_text,
        output_words,
        message,
        diff,
        match_failures,
        error: None,
    };

    if let Some(path) = req.output_file.as_deref() {
        let _ = std::fs::write(path, serde_json::to_string_pretty(&result).unwrap_or_default());
    }
    result
}

fn debug_each_segment(text: &str) -> Vec<Word> {
    let re = regex::Regex::new(r"([\n\p{Punctuation}])").expect("split regex");
    let mut out = Vec::new();
    for part in split_keep(&re, text) {
        out.extend(segment_words(
            &part,
            SegmentMode::NodeNovel,
            DoSegmentOptions::default(),
        ));
    }
    out
}

fn split_keep(re: &regex::Regex, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0;
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
    out
}

pub fn convert_joined(text: &str, tw2cn: bool) -> String {
    #[cfg(feature = "default-dict")]
    {
        if tw2cn {
            return novel_segment_dict::convert_tw2cn(text);
        }
        return novel_segment_dict::convert_cn2tw(text);
    }
    #[cfg(not(feature = "default-dict"))]
    {
        let _ = tw2cn;
        text.to_string()
    }
}

pub fn process_text(text: &str, convert_to_zh_tw: bool, crlf: bool) -> String {
    let words = segment_words(text, SegmentMode::Novel, DoSegmentOptions::default());
    let mut out = stringify(&words);
    if convert_to_zh_tw {
        #[cfg(feature = "default-dict")]
        {
            out = novel_segment_dict::convert_cn2tw(&out);
        }
    }
    if crlf {
        out = crlf_normalize(&out);
    }
    out
}

pub fn crlf_normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn normalize_text(text: &str) -> String {
    crlf_normalize(text.trim())
}

fn ordered_contains(got: &[String], expected: &[ExpectedItem]) -> (bool, Vec<String>) {
    let mut i = 0;
    let mut failed = Vec::new();
    for exp in expected {
        let mut found = None;
        for (idx, w) in got.iter().enumerate().skip(i) {
            if item_hit(w, exp) {
                found = Some(idx);
                break;
            }
        }
        if let Some(idx) = found {
            i = idx + 1;
        } else {
            failed.push(exp.label());
        }
    }
    (failed.is_empty(), failed)
}

fn item_hit(w: &str, exp: &ExpectedItem) -> bool {
    match exp {
        ExpectedItem::One(s) => w == s,
        ExpectedItem::Any(v) => v.iter().any(|s| s == w),
    }
}

fn index_of_all(joined: &str, expected: &[ExpectedItem]) -> (bool, Vec<String>) {
    let mut pos = 0;
    let mut failed = Vec::new();
    for exp in expected {
        match find_item(joined, pos, exp) {
            Some((at, len)) => pos = at + len,
            None => failed.push(exp.label()),
        }
    }
    (failed.is_empty(), failed)
}

fn find_item(joined: &str, from: usize, exp: &ExpectedItem) -> Option<(usize, usize)> {
    let tail = &joined[from.min(joined.len())..];
    match exp {
        ExpectedItem::One(s) => tail.find(s).map(|i| (from + i, s.len())),
        ExpectedItem::Any(v) => v
            .iter()
            .filter_map(|s| tail.find(s).map(|i| (from + i, s.len())))
            .min_by_key(|(i, _)| *i),
    }
}

fn index_of_not_failed(joined: &str, expected: &[ExpectedItem]) -> Vec<String> {
    let mut failed = Vec::new();
    for exp in expected {
        let hit = match exp {
            ExpectedItem::One(s) => joined.contains(s),
            ExpectedItem::Any(v) => v.iter().any(|s| joined.contains(s)),
        };
        if hit {
            failed.push(exp.label());
        }
    }
    failed
}

fn calculate_success(m: &MatchResults, changed: bool) -> bool {
    if m.match_expected_full == Some(false)
        || m.match_expected_contains == Some(false)
        || m.match_expected_contains_not == Some(false)
        || m.match_expected_index_of == Some(false)
        || m.match_expected_index_of_not == Some(false)
    {
        return false;
    }
    if m.match_expected_full.is_none()
        && m.match_expected_contains.is_none()
        && m.match_expected_contains_not.is_none()
        && m.match_expected_index_of.is_none()
        && m.match_expected_index_of_not.is_none()
    {
        return !changed;
    }
    true
}

fn build_message(m: &MatchResults, changed: bool) -> String {
    let mut messages = Vec::new();
    push_msg(&mut messages, m.match_expected_full, "Full match");
    push_msg(&mut messages, m.match_expected_contains, "Contains match");
    push_msg(
        &mut messages,
        m.match_expected_contains_not,
        "Contains-not match",
    );
    push_msg(&mut messages, m.match_expected_index_of, "Index-of match");
    push_msg(
        &mut messages,
        m.match_expected_index_of_not,
        "Index-of-not match",
    );
    if messages.is_empty() {
        if changed {
            "Text was changed during segmentation (no validation tests provided)".into()
        } else {
            "Text was not changed during segmentation".into()
        }
    } else {
        messages.join("; ")
    }
}

fn push_msg(out: &mut Vec<String>, v: Option<bool>, label: &str) {
    if let Some(ok) = v {
        out.push(format!(
            "{label}: {}",
            if ok { "PASSED" } else { "FAILED" }
        ));
    }
}

fn diff_positions(expected: &str, actual: &str) -> Vec<Value> {
    let ev: Vec<char> = expected.chars().collect();
    let av: Vec<char> = actual.chars().collect();
    let max = ev.len().max(av.len());
    let mut i = 0;
    let mut positions = Vec::new();
    while i < max {
        let e = ev.get(i).copied();
        let a = av.get(i).copied();
        if e != a {
            let start = i;
            let mut exp = String::new();
            let mut act = String::new();
            while i < max && ev.get(i).copied() != av.get(i).copied() {
                if let Some(c) = ev.get(i) {
                    exp.push(*c);
                }
                if let Some(c) = av.get(i) {
                    act.push(*c);
                }
                i += 1;
                if i - start > 32 {
                    break;
                }
            }
            positions.push(json!({"start": start, "end": i, "expected": exp, "actual": act}));
        } else {
            i += 1;
        }
    }
    positions
}

fn error_result(message: impl Into<String>, error: impl Into<String>) -> TestResult {
    TestResult {
        success: false,
        changed: false,
        match_results: MatchResults::default(),
        result: Vec::new(),
        output_text: String::new(),
        output_words: Vec::new(),
        message: message.into(),
        diff: None,
        match_failures: None,
        error: Some(error.into()),
    }
}

pub fn load_json_config(path: &Path) -> std::result::Result<TestRequest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_crlf() {
        assert_eq!(normalize_text("  a\r\nb  "), "a\nb");
    }

    #[test]
    fn ordered_contains_any() {
        let got = vec!["兩個".into(), "中國".into()];
        let exp = vec![ExpectedItem::Any(vec!["兩個".into(), "两个".into()])];
        assert!(ordered_contains(&got, &exp).0);
    }
}
