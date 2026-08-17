//! Part-of-speech bit flags. Values match `@novel-segment/postag`.

use std::fmt;

/// POS tag bit flags (identical numeric values to the TypeScript `POSTAG` enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct POSTAG;

impl POSTAG {
    /// 錯字
    pub const BAD: u32 = 0x8000_0000;
    /// 形容詞
    pub const D_A: u32 = 0x4000_0000;
    /// 區別詞
    pub const D_B: u32 = 0x2000_0000;
    /// 連詞
    pub const D_C: u32 = 0x1000_0000;
    /// 副詞
    pub const D_D: u32 = 0x0800_0000;
    /// 嘆詞
    pub const D_E: u32 = 0x0400_0000;
    /// 方位詞
    pub const D_F: u32 = 0x0200_0000;
    /// 成語
    pub const D_I: u32 = 0x0100_0000;
    /// 習語
    pub const D_L: u32 = 0x0080_0000;
    /// 數詞
    pub const A_M: u32 = 0x0040_0000;
    /// 數量詞
    pub const D_MQ: u32 = 0x0020_0000;
    /// 名詞
    pub const D_N: u32 = 0x0010_0000;
    /// 擬聲詞
    pub const D_O: u32 = 0x0008_0000;
    /// 介詞
    pub const D_P: u32 = 0x0004_0000;
    /// 量詞
    pub const A_Q: u32 = 0x0002_0000;
    /// 代詞
    pub const D_R: u32 = 0x0001_0000;
    /// 處所詞
    pub const D_S: u32 = 0x0000_8000;
    /// 時間詞
    pub const D_T: u32 = 0x0000_4000;
    /// 助詞
    pub const D_U: u32 = 0x0000_2000;
    /// 動詞
    pub const D_V: u32 = 0x0000_1000;
    /// 標點符號
    pub const D_W: u32 = 0x0000_0800;
    /// 非語素字
    pub const D_X: u32 = 0x0000_0400;
    /// 語氣詞
    pub const D_Y: u32 = 0x0000_0200;
    /// 狀態詞
    pub const D_Z: u32 = 0x0000_0100;
    /// 人名
    pub const A_NR: u32 = 0x0000_0080;
    /// 地名
    pub const A_NS: u32 = 0x0000_0040;
    /// 機構團體
    pub const A_NT: u32 = 0x0000_0020;
    /// 外文字符
    pub const A_NX: u32 = 0x0000_0010;
    /// 其他專名
    pub const A_NZ: u32 = 0x0000_0008;
    /// 前接成分
    pub const D_ZH: u32 = 0x0000_0004;
    /// 後接成分
    pub const D_K: u32 = 0x0000_0002;
    /// 網址、郵箱
    pub const URL: u32 = 0x0000_0001;
    /// 未知
    pub const UNK: u32 = 0x0000_0000;

    /// True if `p` has any of the listed flags.
    pub fn any(p: u32, flags: &[u32]) -> bool {
        flags.iter().any(|f| p & f != 0)
    }
}

impl fmt::Display for POSTAG {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("POSTAG")
    }
}
