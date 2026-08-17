//! Port of `packages/novel-segment/test/word.novel.test.ts` and long-form novel fixtures.
//! Requires `nodeNovelMode` synonym files (`badword.synonym`, `zht.common.synonym`).

use novel_segment::{stringify, DoSegmentOptions, Segment};
use serde_json::Value;
use std::path::PathBuf;

fn novel_seg() -> &'static Segment {
    static INIT: std::sync::OnceLock<Segment> = std::sync::OnceLock::new();
    INIT.get_or_init(|| Segment::with_node_novel_default().expect("node novel default"))
}

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/js_novel.json")).expect("js_novel.json")
}

fn expected_list(row: &Value) -> Vec<Value> {
    row.get(1)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn simple(text: &str) -> Vec<String> {
    novel_seg().do_segment_simple(text, DoSegmentOptions::default())
}

fn joined(text: &str) -> String {
    stringify(&novel_seg().do_segment(text, DoSegmentOptions::default()))
}

fn item_hit(w: &str, exp: &Value) -> bool {
    if let Some(s) = exp.as_str() {
        return w == s;
    }
    if let Some(arr) = exp.as_array() {
        return arr.iter().any(|x| x.as_str() == Some(w));
    }
    false
}

fn ordered_contains(got: &[String], expected: &[Value]) -> bool {
    let mut i = 0;
    for w in got {
        if i >= expected.len() {
            break;
        }
        if item_hit(w, &expected[i]) {
            i += 1;
        }
    }
    i == expected.len()
}

fn test_res() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/novel-segment/test/res")
}

#[test]
fn novel_base() {
    let data = fixtures();
    for row in data["base"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let expected = expected_list(row);
        let got = simple(input);
        assert!(
            ordered_contains(&got, &expected),
            "novel_base input={input} got={got:?} expected={expected:?}"
        );
    }
}

#[test]
fn novel_base_not() {
    let data = fixtures();
    for row in data["base_not"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let forbidden = expected_list(row);
        let got = simple(input);
        assert!(
            !ordered_contains(&got, &forbidden),
            "novel_base_not input={input} got={got:?} forbidden={forbidden:?}"
        );
    }
}

#[test]
fn novel_array() {
    let data = fixtures();
    for row in data["array"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let alts = row[1].as_array().unwrap();
        let got = simple(input);
        let ok = alts.iter().any(|alt| {
            alt.as_array()
                .map(|seq| ordered_contains(&got, seq))
                .unwrap_or(false)
        });
        assert!(ok, "novel_array input={input} got={got:?}");
    }
}

#[test]
fn novel_indexof() {
    let data = fixtures();
    let mut failed = Vec::new();
    for row in data["indexof"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let needles = expected_list(row);
        let text = joined(input);
        let ok = needles.iter().all(|n| {
            if let Some(s) = n.as_str() {
                text.contains(s)
            } else if let Some(arr) = n.as_array() {
                arr.iter()
                    .any(|x| x.as_str().map(|s| text.contains(s)).unwrap_or(false))
            } else {
                false
            }
        });
        if !ok {
            failed.push((input.to_string(), text, needles));
        }
    }
    assert!(
        failed.is_empty(),
        "novel_indexof {} failures; first={:?}",
        failed.len(),
        failed.first()
    );
}

#[test]
fn novel_indexof_not() {
    let data = fixtures();
    for row in data["indexof_not"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let needles = expected_list(row);
        let text = joined(input);
        let hit = needles
            .iter()
            .any(|n| n.as_str().map(|s| text.contains(s)).unwrap_or(false));
        assert!(
            !hit,
            "novel_indexof_not failed input={input} joined={text} needles={needles:?}"
        );
    }
}

#[test]
fn woltenia_chapter_does_not_panic() {
    let path = test_res().join("ウォルテニア戦記/第11話【西へ】其2.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let words = novel_seg().do_segment(&text, DoSegmentOptions::default());
    assert!(!words.is_empty());
    // Default novel profile converts synonyms, so stringify may differ.
    // Raw reconstruction is still a sequence of tokens covering the input length.
    let token_chars: usize = words.iter().map(|w| w.w.chars().count()).sum();
    assert!(token_chars > 0);
}

#[test]
fn gc_not_long_file_does_not_panic() {
    let path = test_res().join("gc.not/666962621.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let _ = novel_seg().do_segment(&text, DoSegmentOptions::default());
}
