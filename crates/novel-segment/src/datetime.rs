//! Date/time unit words.

use once_cell::sync::Lazy;
use std::collections::HashSet;

const DATETIME_RAW: &[&str] = &[
    "世纪", "年", "年份", "年度", "月", "月份", "月度", "日", "号", "时", "点", "点钟", "分",
    "分钟", "秒", "毫秒",
];

pub static DATETIME: Lazy<HashSet<String>> = Lazy::new(|| {
    #[cfg(feature = "default-dict")]
    {
        novel_segment_dict::arr_cjk(DATETIME_RAW.iter().copied())
            .into_iter()
            .collect()
    }
    #[cfg(not(feature = "default-dict"))]
    {
        DATETIME_RAW.iter().map(|s| (*s).to_string()).collect()
    }
});
