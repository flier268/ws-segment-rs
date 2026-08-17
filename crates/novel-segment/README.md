# novel-segment

中文分詞函式庫（TypeScript [`novel-segment`](../../packages/novel-segment) 的 Rust 移植）。以盤古分詞詞庫為基礎，並加入網路小說用語。

其他 Rust 程式請只依賴這個 crate。

## 引用

本機 path：

```toml
[dependencies]
novel-segment = { path = "../ws-segment-rs/crates/novel-segment" }
```

Git：

```toml
[dependencies]
novel-segment = { git = "https://github.com/flier268/ws-segment-rs" }
```

關閉內建字典：

```toml
novel-segment = { path = "...", default-features = false }
```

字典根目錄可用環境變數 `NOVEL_SEGMENT_DICT_ROOT` 覆寫（預設為倉庫內 `packages/segment-dict/dict`）。

## 使用

```rust
use novel_segment::{DoSegmentOptions, Segment, POSTAG};

fn main() -> Result<(), novel_segment::Error> {
    // JS `new Segment(); useDefault()` — auto_cjk 預設關閉。
    // 小說／測試路徑與 JS createSegment 相同：auto_cjk + all_mod + convertSynonym
    let mut seg = Segment::new(novel_segment::SegmentOptions {
        auto_cjk: true,
        ..Default::default()
    });
    seg.use_default()?;

    let words = seg.do_segment(
        "这是一个基于Rust的中文分词模块。",
        DoSegmentOptions::default(),
    );

    let simple = seg.do_segment_simple("...", DoSegmentOptions::convert_synonym());

    seg.add_word("專有名詞", Some(POSTAG::D_N), Some(100.0))?;
    seg.add_synonym("標準詞", &["錯字", "異體"]);
    Ok(())
}
```

`DoSegmentOptions` 對應 JS `doSegment`：`simple`、`strip_punctuation`、`convert_synonym`、`strip_stopword`、`strip_space`。

預設會回傳所有字元（含換行與空白），與 JS 2.x 相同。

小說長篇語料（`word.novel.test.ts`）請用 `Segment::with_node_novel_default()`：會清空同義詞再載入 `synonym` + `zht.synonym` + `badword.synonym` + `zht.common.synonym`。

CLI / HTTP API / MCP 見 [crates/novel-segment-cli](../novel-segment-cli)。Node 若要直接 `require`，可建 `novel-segment-napi`：

```bash
cargo build -p novel-segment-napi
node packages/novel-segment-native/scripts/copy-addon.js
```
