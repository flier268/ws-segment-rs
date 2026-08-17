//! MCP stdio server (JSON-RPC with Content-Length framing).

use novel_segment::{run_test, TestRequest};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

const PROTOCOL: &str = "2024-11-05";

pub fn serve() -> io::Result<()> {
    eprintln!("novel-segment MCP Server started");
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    loop {
        let Some(msg) = read_message(&mut stdin)? else {
            break;
        };
        if let Some(resp) = handle(msg) {
            write_message(&resp)?;
        }
    }
    Ok(())
}

fn handle(msg: Value) -> Option<Value> {
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    if id.is_none() {
        return None;
    }

    let result = match method {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOL),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "novel-segment", "version": env!("CARGO_PKG_VERSION") },
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(name, args) {
                Ok(text) => json!({
                    "content": [{ "type": "text", "text": text }],
                }),
                Err(e) => {
                    return Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32000, "message": e },
                    }));
                }
            }
        }
        "shutdown" => json!({}),
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Unknown method: {method}") },
            }));
        }
    };

    Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

fn call_tool(name: &str, args: Value) -> Result<String, String> {
    let req = match name {
        "segment_text" => TestRequest {
            text: args.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
            expected_full: args.get("expectedFull").and_then(|v| v.as_str()).map(|s| s.to_string()),
            expected_contains: parse_expected(args.get("expectedContains")),
            expected_contains_not: parse_expected(args.get("expectedContainsNot")),
            expected_index_of: parse_expected(args.get("expectedIndexOf")),
            expected_index_of_not: parse_expected(args.get("expectedIndexOfNot")),
            dict_entries: args
                .get("dictEntries")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            synonym_entries: args
                .get("synonymEntries")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            blacklist_words: args
                .get("blacklistWords")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
            debug_each: args.get("debugEach").and_then(|v| v.as_bool()),
            output_format: Some("json".into()),
            ..Default::default()
        },
        "segment_file" => TestRequest {
            file: args.get("file").and_then(|v| v.as_str()).map(|s| s.to_string()),
            expected_full_file: args
                .get("expectedFullFile")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            output_file: args
                .get("outputFile")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            output_format: args
                .get("outputFormat")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        },
        "segment_config" => {
            let path = args
                .get("config")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "config is required".to_string())?;
            let mut req = novel_segment::load_json_config(std::path::Path::new(path))?;
            if let Some(text) = args.get("text").and_then(|v| v.as_str()) {
                req.text = Some(text.to_string());
            }
            req
        }
        _ => return Err(format!("Unknown tool: {name}")),
    };
    let result = run_test(req);
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

fn parse_expected(v: Option<&Value>) -> Option<Vec<novel_segment::ExpectedItem>> {
    v.and_then(|x| serde_json::from_value(x.clone()).ok())
}

fn tools() -> Value {
    json!([
        {
            "name": "segment_text",
            "description": "對文字進行分詞測試 / Perform segmentation test on text",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "待分詞的文字內容" },
                    "expectedFull": { "type": "string" },
                    "expectedContains": { "type": "array" },
                    "expectedContainsNot": { "type": "array" },
                    "expectedIndexOf": { "type": "array" },
                    "expectedIndexOfNot": { "type": "array" },
                    "dictEntries": { "type": "array" },
                    "synonymEntries": { "type": "array" },
                    "blacklistWords": { "type": "array", "items": { "type": "string" } },
                    "debugEach": { "type": "boolean", "default": false }
                },
                "required": ["text"]
            }
        },
        {
            "name": "segment_file",
            "description": "從檔案讀取文字進行分詞測試 / Read text from file for segmentation test",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "expectedFullFile": { "type": "string" },
                    "outputFile": { "type": "string" },
                    "outputFormat": { "type": "string", "enum": ["json", "text"], "default": "json" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "segment_config",
            "description": "使用 JSON 設定檔進行分詞測試",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "config": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["config"]
            }
        }
    ])
}

fn read_message(stdin: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        let n = stdin.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            if headers.is_empty() {
                continue;
            }
            break;
        }
        headers.push_str(&line);
    }

    let mut content_length = None;
    for line in headers.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.eq_ignore_ascii_case("content-length") {
            content_length = v.trim().parse::<usize>().ok();
        }
    }

    if let Some(len) = content_length {
        let mut buf = vec![0u8; len];
        stdin.read_exact(&mut buf)?;
        let value = serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        return Ok(Some(value));
    }

    // Fallback: newline-delimited JSON (handy for manual tests).
    if let Some(line) = headers.lines().find(|l| l.starts_with('{')) {
        let value = serde_json::from_str(line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        return Ok(Some(value));
    }
    Ok(None)
}

fn write_message(value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value)?;
    let mut out = io::stdout().lock();
    write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
    out.write_all(&body)?;
    out.flush()
}
