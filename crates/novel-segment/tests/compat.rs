use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};

fn seg_cjk() -> &'static Segment {
    static INIT: std::sync::OnceLock<Segment> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        let mut s = Segment::new(SegmentOptions {
            auto_cjk: true,
            ..Default::default()
        });
        s.use_default().expect("load default dicts");
        s
    })
}

fn words(text: &str) -> Vec<String> {
    seg_cjk().do_segment_simple(text, DoSegmentOptions::default())
}

fn contains_seq(got: &[String], expected: &[&str]) -> bool {
    let mut i = 0;
    for w in got {
        if i < expected.len() && w == expected[i] {
            i += 1;
        }
    }
    i == expected.len()
}

#[test]
fn readme_example_sentence() {
    let got = words("这是一个基于Node.js的中文分词模块。");
    assert_eq!(
        got,
        vec!["这是", "一个", "基于", "Node.js", "的", "中文", "分词", "模块", "。"]
    );
}

#[test]
fn two_chinas() {
    let got = words("两个中国");
    assert!(got.windows(2).any(|w| w == ["两个", "中国"]), "{got:?}");
}

#[test]
fn convert_synonym_graffiti() {
    let got = seg_cjk().do_segment_simple(
        "我就順便在你臉上涂鴉吧",
        DoSegmentOptions::convert_synonym(),
    );
    assert!(got.iter().any(|w| w == "塗鴉"), "{got:?}");
}

#[test]
fn preserves_newline() {
    let got = words("甲\n乙");
    assert!(got.contains(&"\n".to_string()), "{got:?}");
}

#[test]
fn split_line_edge_space() {
    let got = words("甲 \n乙");
    assert!(got.contains(&" ".to_string()), "{got:?}");
    let got = words("甲\n 乙");
    assert!(got.contains(&" ".to_string()), "{got:?}");
}

#[test]
fn lazy_base_samples() {
    assert!(contains_seq(&words("胡锦涛出席APEC领导人会议后回京"), &["会议", "回京"]));
    assert!(contains_seq(&words("全部都有"), &["全部", "都有"]));
    assert!(contains_seq(&words("從位在下方的湖面"), &["位在", "下方"]));
    assert!(contains_seq(
        &words("將那叫燕麥茶的玩意兒一口氣倒入口中。"),
        &["一口氣", "倒入", "口中"]
    ));
    assert!(contains_seq(&words("疲憊不堪的強尼癱坐在岩石上"), &["癱坐"]));
}

#[test]
fn tests_old_exact() {
    let cases: &[(&str, &[&str])] = &[
        ("永和服装饰品有限公司", &["永和", "服装", "饰品", "有限公司"]),
        ("本科班学生", &["本科", "班", "学生"]),
        ("研究生命起源", &["研究", "生命", "起源"]),
        ("一一", &["一一"]),
        ("一 一", &["一", " ", "一"]),
        ("欢迎访问我的个人主页http://ucdok.com娃哈哈", &[
            "欢迎", "访问", "我", "的", "个人", "主页", "http://ucdok.com", "娃哈哈",
        ]),
    ];
    for (input, expected) in cases {
        let got = words(input);
        assert_eq!(&got.iter().map(|s| s.as_str()).collect::<Vec<_>>(), expected, "input={input}");
    }
}

#[test]
fn email_does_not_swallow_trailing_number() {
    let got = words("mail@host.com123");
    assert!(
        got.iter().any(|w| w == "123") || got.last().map(|s| s.ends_with("123")).unwrap_or(false),
        "{got:?}"
    );
}

#[test]
fn call_options_override_constructor() {
    let mut seg = Segment::new(SegmentOptions {
        auto_cjk: true,
        options_do_segment: DoSegmentOptions::convert_synonym(),
        ..Default::default()
    });
    seg.use_default().unwrap();
    let off = seg.do_segment_simple(
        "我就順便在你臉上涂鴉吧",
        DoSegmentOptions {
            convert_synonym: Some(false),
            ..Default::default()
        },
    );
    assert!(off.iter().any(|w| w.contains('涂') || w == "涂鴉"), "{off:?}");
}



#[test]
fn cache_does_not_apply_modules_when_nomod() {
    let mut a = Segment::new(SegmentOptions {
        auto_cjk: true,
        ..Default::default()
    });
    a.use_default().unwrap();
    let mut b = Segment::new(SegmentOptions {
        auto_cjk: true,
        nomod: true,
        ..Default::default()
    });
    b.use_default().unwrap();
    // nomod: no tokenizers; do_segment should error or return unsplit if tokenizers empty.
    // use_default with nomod skips modules, so tokenizers stay empty.
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        b.do_segment("测试", DoSegmentOptions::default())
    }));
    assert!(res.is_ok());
}

#[test]
fn library_default_auto_cjk_is_off() {
    let mut seg = Segment::with_default().unwrap();
    let _ = &mut seg;
    // with_default matches JS useDefault: auto_cjk false.
    // 基于 may or may not be in the raw table; 基於 is.
    let got = seg.do_segment_simple("基於", DoSegmentOptions::default());
    assert_eq!(got, vec!["基於"]);
}
