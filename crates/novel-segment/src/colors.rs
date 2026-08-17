//! Color word lists for AdjectiveOptimizer.

mod color_rgb {
    include!("color_rgb.rs");
}

use once_cell::sync::Lazy;
use std::collections::HashSet;

const COLOR_HAIR_RAW: &[&str] = &[
    "乌", "朱", "栗", "桃", "棕", "橘", "橙", "灰", "白", "碧", "紅", "紫", "綠", "红", "绯",
    "绿", "翠", "苍", "茜", "蓝", "藍", "褐", "赤", "金", "银", "青", "靛", "黃", "黄", "黑",
    "黒", "茶",
];

const COLOR_EXTRA: &[&str] = &[
    "丹", "彤", "绛", "纁", "赭", "驼", "曙", "墨", "米", "缃", "藕", "玄", "皂", "黛", "黝",
    "素", "杏", "缟", "鹤", "皓", "华",
];

fn expand(raw: &[&str]) -> HashSet<String> {
    #[cfg(feature = "default-dict")]
    {
        novel_segment_dict::arr_cjk(raw.iter().copied()).into_iter().collect()
    }
    #[cfg(not(feature = "default-dict"))]
    {
        raw.iter().map(|s| (*s).to_string()).collect()
    }
}

pub static COLOR_HAIR: Lazy<HashSet<String>> = Lazy::new(|| expand(COLOR_HAIR_RAW));

pub static COLOR_ALL: Lazy<HashSet<String>> = Lazy::new(|| {
    let mut s = expand(COLOR_HAIR_RAW);
    s.extend(expand(COLOR_EXTRA));
    s.extend(expand(color_rgb::COLOR_WITH_RGB));
    s
});
