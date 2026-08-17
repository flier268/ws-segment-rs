use novel_segment::{DoSegmentOptions, Segment};

fn main() -> Result<(), novel_segment::Error> {
    let mut seg = Segment::new(novel_segment::SegmentOptions {
        auto_cjk: true,
        ..Default::default()
    });
    seg.use_default()?;
    let text = "这是一个基于Rust的中文分词模块。";
    let words = seg.do_segment(text, DoSegmentOptions::default());
    println!("{text}");
    println!(
        "{}",
        words
            .iter()
            .map(|w| format!("{}:{:x}", w.w, w.p.unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(" / ")
    );

    let simple = seg.do_segment_simple(text, DoSegmentOptions::default());
    println!("{}", simple.join(" / "));

    seg.add_word("專有名詞", Some(novel_segment::POSTAG::D_N), Some(100.0))?;
    Ok(())
}
