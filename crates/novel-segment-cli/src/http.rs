//! HTTP API port of `@novel-segment/api-server`.

use novel_segment::{convert_joined, segment_words, DoSegmentOptions, SegmentMode, Word};
use serde_json::{json, Map, Value};
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Request, Response, Server};
use url::Url;

pub fn serve(bind: &str) -> Result<(), String> {
    let _ = segment_words("", SegmentMode::Novel, DoSegmentOptions::default());
    let server = Server::http(bind).map_err(|e| e.to_string())?;
    eprintln!("ws-segment-rs API listening on http://{bind}");
    for request in server.incoming_requests() {
        eprintln!("incoming {} {}", request.method(), request.url());
        match handle(request) {
            Ok(()) => eprintln!("handled ok"),
            Err(e) => eprintln!("handled err: {e}"),
        }
    }
    Ok(())
}

fn handle(mut request: Request) -> Result<(), String> {
    let url = format!("http://localhost{}", request.url());
    let parsed = match Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            let body = json!({"code":0,"error":true,"message":e.to_string()}).to_string();
            let _ = request.respond(json_response(400, body, None));
            return Err(e.to_string());
        }
    };
    let path = parsed.path().to_string();
    let mut rq = query_map(&parsed);

    if matches!(*request.method(), Method::Post | Method::Put | Method::Patch) {
        let mut buf = String::new();
        if let Err(e) = request.as_reader().read_to_string(&mut buf) {
            let body = json!({"code":0,"error":true,"message":e.to_string()}).to_string();
            let _ = request.respond(json_response(500, body, None));
            return Err(e.to_string());
        }
        if !buf.trim().is_empty() {
            if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&buf) {
                merge_map(&mut rq, obj);
            } else {
                merge_map(&mut rq, form_map(&buf));
            }
        }
    }

    if path == "/conv" {
        return respond_conv(request, rq);
    }
    respond_segment(request, rq)
}

fn respond_segment(request: Request, mut rq: Map<String, Value>) -> Result<(), String> {
    let timestamp = now_ms();
    let inputs = collect_input(rq.remove("input"));
    if inputs.is_empty() {
        let body = json!({
            "code": 0,
            "error": true,
            "timestamp": timestamp,
            "message": "參數錯誤",
            "request": rq,
        })
        .to_string();
        return request
            .respond(json_response(400, body, Some(10_000)))
            .map_err(|e| e.to_string());
    }

    let options = rq.get("options").cloned().unwrap_or(json!({}));
    let opts = parse_do_opts(options.as_object());
    let debug = as_bool(rq.get("debug"));
    let nocache = as_bool(rq.get("nocache"));

    let results: Vec<Value> = inputs
        .iter()
        .map(|line| {
            let words = segment_words(line, SegmentMode::Novel, opts.clone());
            words_to_json(&words, opts.simple.unwrap_or(false))
        })
        .collect();

    let mut json = json!({
        "code": 1,
        "count": results.len(),
        "timestamp": timestamp,
        "time": now_ms() - timestamp,
        "results": results,
        "options": options,
    });
    if debug {
        json["request"] = json!({ "rq": rq, "url": request.url() });
    }
    let max_age = if nocache || debug { None } else { Some(3_600_000) };
    request
        .respond(json_response(200, json.to_string(), max_age))
        .map_err(|e| e.to_string())
}

fn respond_conv(request: Request, mut rq: Map<String, Value>) -> Result<(), String> {
    let timestamp = now_ms();
    let inputs = collect_input(rq.remove("input"));
    let tw2cn = rq
        .get("options")
        .and_then(|o| o.get("tw2cn"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let debug = as_bool(rq.get("debug"));
    let nocache = as_bool(rq.get("nocache"));

    let results: Vec<String> = inputs
        .iter()
        .map(|line| {
            let words = segment_words(
                line,
                SegmentMode::Novel,
                DoSegmentOptions {
                    simple: Some(true),
                    ..Default::default()
                },
            );
            let joined: String = words.into_iter().map(|w| w.w).collect();
            convert_joined(&joined, tw2cn)
        })
        .collect();

    let json = json!({
        "code": 1,
        "count": results.len(),
        "timestamp": timestamp,
        "time": now_ms() - timestamp,
        "results": results,
    });
    let max_age = if nocache || debug { None } else { Some(3_600_000) };
    request
        .respond(json_response(200, json.to_string(), max_age))
        .map_err(|e| e.to_string())
}

fn words_to_json(words: &[Word], simple: bool) -> Value {
    if simple {
        json!(words.iter().map(|w| w.w.clone()).collect::<Vec<_>>())
    } else {
        json!(words
            .iter()
            .map(|w| json!({"w": w.w, "p": w.p, "f": w.f}))
            .collect::<Vec<_>>())
    }
}

fn parse_do_opts(obj: Option<&Map<String, Value>>) -> DoSegmentOptions {
    let Some(o) = obj else {
        return DoSegmentOptions::default();
    };
    DoSegmentOptions {
        simple: o.get("simple").and_then(|v| v.as_bool()),
        strip_punctuation: o.get("stripPunctuation").and_then(|v| v.as_bool()),
        convert_synonym: o.get("convertSynonym").and_then(|v| v.as_bool()),
        strip_stopword: o.get("stripStopword").and_then(|v| v.as_bool()),
        strip_space: o.get("stripSpace").and_then(|v| v.as_bool()),
        disable_modules: Vec::new(),
    }
}

fn collect_input(input: Option<Value>) -> Vec<String> {
    match input {
        Some(Value::String(s)) if !s.is_empty() => vec![s],
        Some(Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s),
                Value::Array(bytes) => {
                    let u8s: Vec<u8> = bytes.iter().filter_map(|b| b.as_u64().map(|n| n as u8)).collect();
                    Some(String::from_utf8_lossy(&u8s).into_owned())
                }
                _ => v.as_str().map(|s| s.to_string()),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn query_map(url: &Url) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, v) in url.query_pairs() {
        insert_query(&mut map, &k, &v);
    }
    map
}

fn insert_query(map: &mut Map<String, Value>, key: &str, value: &str) {
    if let Ok(parsed) = serde_json::from_str::<Value>(value) {
        map.insert(key.to_string(), parsed);
        return;
    }
    if value == "true" {
        map.insert(key.to_string(), Value::Bool(true));
    } else if value == "false" {
        map.insert(key.to_string(), Value::Bool(false));
    } else {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn merge_map(dst: &mut Map<String, Value>, src: Map<String, Value>) {
    for (k, v) in src {
        dst.insert(k, v);
    }
}

fn form_map(body: &str) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, v) in url::form_urlencoded::parse(body.as_bytes()) {
        insert_query(&mut map, &k, &v);
    }
    map
}

fn as_bool(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "1" || s.eq_ignore_ascii_case("true"),
        Some(Value::Number(n)) => n.as_u64() == Some(1),
        _ => false,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn json_response(status: u16, body: String, max_age: Option<u32>) -> Response<Cursor<Vec<u8>>> {
    let mut resp = Response::from_data(body.into_bytes()).with_status_code(status);
    if let Ok(h) = "Content-Type: application/json; charset=utf-8".parse::<Header>() {
        resp = resp.with_header(h);
    }
    if let Ok(h) = "Access-Control-Allow-Origin: *".parse::<Header>() {
        resp = resp.with_header(h);
    }
    if let Some(age) = max_age {
        if let Ok(h) = format!("Cache-Control: public, max-age={age}").parse::<Header>() {
            resp = resp.with_header(h);
        }
    }
    resp
}
