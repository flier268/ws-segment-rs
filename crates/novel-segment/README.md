# ws-segment-rs

中文分詞函式庫（TypeScript [`novel-segment`](../../packages/novel-segment) 的 Rust 移植）。以盤古分詞詞庫為基礎，並加入網路小說用語。

crates.io 套件名是 `ws-segment-rs`；Rust 裡仍 `use novel_segment::…`。

## 引用

本機 path：

```toml
[dependencies]
ws-segment-rs = { path = "../ws-segment-rs/crates/novel-segment" }
```

Git / crates.io：

```toml
[dependencies]
ws-segment-rs = "0.1"
```

關閉內建字典：

```toml
ws-segment-rs = { version = "0.1", default-features = false }
```

字典根目錄可用環境變數 `NOVEL_SEGMENT_DICT_ROOT` 覆寫（預設為倉庫內 `packages/segment-dict/dict`）。

## DictTokenizer 路徑搜尋

`DictTokenizer` 用詞庫重疊詞組成切法，再用 MMSEG 式的 `Assess`／`get_tops` 打分（詞數、平均詞頻、詞長變異、未知詞、相鄰詞加分）。

詞庫匹配走 UTF-16 trie（每個位置沿前綴往下走，只在真正成詞時才取出）。短段 `get_chunks` 有 memo。

JS／舊 Rust 對**整段**做 `get_chunks` 枚舉，未切開的長漢字會指數展開，記憶體與時間都會爆。Rust 現在依**目前這一段**的 UTF-16 長度分流（不是把詞從中間硬切）：

| 段長 | 作法 |
| --- | --- |
| ≤ `DICT_ENUM_MAX_SPAN`（24） | 仍走原本 `get_chunks` 枚舉（有 memo），對齊 JS 短句結果 |
| > 24 | DAG beam search 留下存活路徑，再套同一套 Assess／`get_tops`（不是對全部路徑做精確 `get_tops`） |

長段每個字位置最多留下一批存活狀態（預設 **64**）。長段打分用固定單字包絡（`x = n`、`b = 0`，不把存活集的 max-`a` 當 +1）。beam 之後只對「還含單字」的窗（最多 6 個詞、長度 ≤24）再跑短段枚舉，詞數更少的切法會換上。上限在每次長段搜尋開始時讀一次環境變數（執行中改 env 不會影響已在跑的搜尋；改完請重啟使用此 crate 的程式）：

```bash
# 同一段長文多跑幾次，比輸出是否還變、時間是否還掉
WS_SEGMENT_MAX_STATES=16
WS_SEGMENT_MAX_STATES=64   # 預設
WS_SEGMENT_MAX_STATES=256
```

- 64 與 256 輸出相同、64 比較快 → 64 夠用，略鬆  
- 64 與 256 輸出不同 → 64 太緊，提高到兩邊一樣為止  
- 上限打滿只代表該位置候選很多，不代表結果一定不穩  

`SegmentOptions::max_chunk_count`／`min_chunk_count`（JS 預設 40／30）**只影響短段枚舉**；長段 DP 不走這兩個上限。

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

`add_synonym` 會把錯字寫進詞表，並在 `convert_synonym` 同一趟改成正字。

預設會回傳所有字元（含換行與空白），與 JS 2.x 相同。

小說長篇語料（`word.novel.test.ts`）請用 `Segment::with_node_novel_default()`：會清空同義詞再載入 `synonym` + `zht.synonym` + `badword.synonym` + `zht.common.synonym`。

CLI / HTTP API / MCP 見 [crates/novel-segment-cli](../novel-segment-cli)（crate：`ws-segment-rs-cli`）。Node 若要直接 `require`，可建 `ws-segment-rs-napi`：

```bash
cargo build -p ws-segment-rs-napi
node packages/novel-segment-native/scripts/copy-addon.js
```
