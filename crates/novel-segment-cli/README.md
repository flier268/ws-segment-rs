# ws-segment-rs-cli

Rust 版 CLI、HTTP API、MCP。分詞走 [`ws-segment-rs`](../novel-segment) crate。

```bash
cargo run -p ws-segment-rs-cli -- --text "这是一个中文分词模块。" --output text
cargo run -p ws-segment-rs-cli -- process --text "我就順便在你臉上涂鴉吧"
cargo run -p ws-segment-rs-cli -- serve --bind 127.0.0.1:3000
cargo run -p ws-segment-rs-cli -- mcp
```

編譯後的二進位是 `target/debug/ws-segment-rs`（或 `release`）。

## MCP

```json
{
  "mcpServers": {
    "novel-segment": {
      "command": "/path/to/novel-segment",
      "args": ["mcp"]
    }
  }
}
```

工具：`segment_text`、`segment_file`、`segment_config`。

## HTTP API

與原本 `@novel-segment/api-server` 相同：

- `GET|POST /?input=兩個中國`
- `GET|POST /conv?input=...`（可加 `options={"tw2cn":true}`）
