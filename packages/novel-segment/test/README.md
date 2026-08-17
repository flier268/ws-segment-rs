# 測試語料

JS 單元測試已移除。回歸測試在 Rust：

```bash
cargo test -p ws-segment-rs
```

`res/` 裡的長篇檔仍被 `crates/novel-segment/tests/word_novel.rs` 讀取。
