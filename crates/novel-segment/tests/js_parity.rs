//! Port of JS `word.test.ts`, `res/default.ts`, `z.gc.test.ts`, and lazy fixtures.

use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};
use serde_json::Value;

fn novel_seg() -> &'static Segment {
    static INIT: std::sync::OnceLock<Segment> = std::sync::OnceLock::new();
    INIT.get_or_init(|| Segment::with_novel_default().expect("load novel default"))
}

fn cjk_seg() -> &'static Segment {
    static INIT: std::sync::OnceLock<Segment> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let mut s = Segment::new(SegmentOptions {
            auto_cjk: true,
            ..Default::default()
        });
        s.use_default().expect("load cjk default");
        s
    })
}

fn simple(text: &str) -> Vec<String> {
    novel_seg().do_segment_simple(text, DoSegmentOptions::default())
}

fn joined(text: &str) -> String {
    simple(text).concat()
}

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/js_tests.json")).expect("js_tests.json")
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

fn item_hit(w: &str, exp: &Value) -> bool {
    if let Some(s) = exp.as_str() {
        return w == s;
    }
    if let Some(arr) = exp.as_array() {
        return arr.iter().any(|x| x.as_str() == Some(w));
    }
    false
}

fn expected_list(row: &Value) -> Vec<Value> {
    row.get(1)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

#[test]
fn lazy_base() {
    let data = fixtures();
    for row in data["lazy_base"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let expected = expected_list(row);
        let got = simple(input);
        assert!(
            ordered_contains(&got, &expected),
            "lazy_base failed\ninput={input}\ngot={got:?}\nexpected={expected:?}"
        );
    }
}

#[test]
fn lazy_base_not() {
    let data = fixtures();
    for row in data["lazy_base_not"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let forbidden = expected_list(row);
        let got = simple(input);
        assert!(
            !ordered_contains(&got, &forbidden),
            "lazy_base_not unexpectedly matched\ninput={input}\ngot={got:?}\nforbidden={forbidden:?}"
        );
    }
}

#[test]
fn lazy_array() {
    let data = fixtures();
    for row in data["lazy_array"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let alts = row[1].as_array().unwrap();
        let got = simple(input);
        let ok = alts.iter().any(|alt| {
            alt.as_array()
                .map(|seq| ordered_contains(&got, seq))
                .unwrap_or(false)
        });
        assert!(ok, "lazy_array failed\ninput={input}\ngot={got:?}\nalts={alts:?}");
    }
}

#[test]
fn lazy_indexof() {
    let data = fixtures();
    let mut failed = 0usize;
    let mut first = None;
    for row in data["lazy_indexof"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let needles = expected_list(row);
        let text = joined(input);
        let ok = needles.iter().all(|n| {
            if let Some(s) = n.as_str() {
                text.contains(s)
            } else if let Some(arr) = n.as_array() {
                arr.iter().any(|x| x.as_str().map(|s| text.contains(s)).unwrap_or(false))
            } else {
                false
            }
        });
        if !ok {
            failed += 1;
            if first.is_none() {
                first = Some((input.to_string(), text, needles));
            }
        }
    }
    if failed > 0 {
        let (input, text, needles) = first.unwrap();
        panic!("lazy_indexof {failed} failures; first input={input} joined={text} needles={needles:?}");
    }
}

#[test]
fn lazy_indexof_not() {
    let data = fixtures();
    for row in data["lazy_indexof_not"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let needles = expected_list(row);
        let text = joined(input);
        let hit = needles.iter().any(|n| n.as_str().map(|s| text.contains(s)).unwrap_or(false));
        assert!(!hit, "lazy_indexof_not failed input={input} joined={text} needles={needles:?}");
    }
}

#[test]
fn tests_old_exact() {
    let data = fixtures();
    for row in data["exact"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let expected: Vec<&str> = row[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let got = cjk_seg().do_segment_simple(input, DoSegmentOptions::default());
        assert_eq!(got, expected, "exact input={input}");
    }
}

#[test]
fn gc_does_not_panic() {
    let data = fixtures();
    for row in data["gc"].as_array().unwrap() {
        let input = row.as_str().unwrap();
        let _ = simple(input);
    }
}

#[test]
fn bug_constructor_not_native_code() {
    let text = "inspection.dead.code.problem.synopsis28.constructor=构造函数有一个用法,但它是不可到达的从入口点.";
    let mut seg = Segment::new(SegmentOptions {
        auto_cjk: true,
        node_novel_mode: true,
        ..Default::default()
    });
    seg.use_default().unwrap();
    let joined = seg
        .do_segment(text, DoSegmentOptions::default())
        .into_iter()
        .map(|w| w.w)
        .collect::<String>();
    assert!(joined.contains("inspection.dead.code.problem.synopsis28.constructor"));
    assert!(!joined.contains("[native code]"));
}
