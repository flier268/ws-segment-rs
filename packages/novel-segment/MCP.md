# MCP

分詞 MCP 已改為 Rust 二進位 `novel-segment mcp`（crate：`novel-segment-cli`）。

```bash
cargo build -p ws-segment-rs-cli --release
```

## Claude Desktop / 其他客戶端

```json
{
  "mcpServers": {
    "novel-segment": {
      "command": "/absolute/path/to/target/release/ws-segment-rs",
      "args": ["mcp"]
    }
  }
}
```

在本倉庫開發時也可以用 npm bin（會轉呼叫已編譯的 Rust 二進位）：

```json
{
  "mcpServers": {
    "novel-segment": {
      "command": "node",
      "args": ["${workspaceFolder}/packages/novel-segment/bin/mcp-server.js"]
    }
  }
}
```

## 工具

- `segment_text` — 對文字分詞並可做 expected 比對
- `segment_file` — 從檔案讀取
- `segment_config` — 讀 JSON 設定檔
