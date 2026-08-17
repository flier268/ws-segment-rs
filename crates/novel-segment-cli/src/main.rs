//! Unified CLI / API / MCP entry point.

mod http;
mod mcp;

use novel_segment::{
    load_json_config, process_text, run_test, ExpectedItem, TestRequest, TestResult,
};
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") && args.first().map(|s| s.as_str()) != Some("process") {
        if matches!(args.first().map(|s| s.as_str()), Some("serve") | Some("mcp") | Some("process")) {
            // fall through to subcommand help
        } else {
            print_help();
            return;
        }
    }

    match args.first().map(|s| s.as_str()) {
        Some("serve") => {
            let bind = flag_value(&args[1..], "--bind")
                .or_else(|| flag_value(&args[1..], "--port").map(|p| format!("127.0.0.1:{p}")))
                .unwrap_or_else(|| "127.0.0.1:3000".into());
            if let Err(e) = http::serve(&bind) {
                eprintln!("api error: {e}");
                process::exit(1);
            }
        }
        Some("mcp") => {
            if let Err(e) = mcp::serve() {
                eprintln!("mcp error: {e}");
                process::exit(1);
            }
        }
        Some("process") => run_process(&args[1..]),
        _ => run_test_cli(&args),
    }
}

fn run_test_cli(args: &[String]) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let mut req = TestRequest::default();
    let mut config: Option<PathBuf> = None;
    let mut output = "json".to_string();
    let mut quiet = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-t" | "--text" => {
                i += 1;
                req.text = args.get(i).cloned();
            }
            "-f" | "--file" => {
                i += 1;
                req.file = args.get(i).cloned();
            }
            "--expected-full" | "--expectedFull" | "-efull" => {
                i += 1;
                req.expected_full = args.get(i).cloned();
            }
            "--expected-full-file" | "--expectedFullFile" | "-efullfile" => {
                i += 1;
                req.expected_full_file = args.get(i).cloned();
            }
            "--expected-contains" | "--expectedContains" | "-ec" => {
                i += 1;
                req.expected_contains = args.get(i).map(|s| parse_expected(s));
            }
            "--expected-contains-not" | "--expectedContainsNot" | "-ecn" => {
                i += 1;
                req.expected_contains_not = args.get(i).map(|s| parse_expected(s));
            }
            "--expected-index-of" | "--expectedIndexOf" | "-eio" => {
                i += 1;
                req.expected_index_of = args.get(i).map(|s| parse_expected(s));
            }
            "--expected-index-of-not" | "--expectedIndexOfNot" | "-eion" => {
                i += 1;
                req.expected_index_of_not = args.get(i).map(|s| parse_expected(s));
            }
            "--dict-entries" => {
                i += 1;
                req.dict_entries = args.get(i).and_then(|s| serde_json::from_str(s).ok());
            }
            "--synonym-entries" => {
                i += 1;
                req.synonym_entries = args.get(i).and_then(|s| serde_json::from_str(s).ok());
            }
            "--blacklist" => {
                i += 1;
                req.blacklist_words = args.get(i).and_then(|s| serde_json::from_str(s).ok());
            }
            "-c" | "--config" => {
                i += 1;
                config = args.get(i).map(PathBuf::from);
            }
            "-o" | "--output" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    output = v.clone();
                }
            }
            "--output-file" | "--outputFile" => {
                i += 1;
                req.output_file = args.get(i).cloned();
            }
            "-q" | "--quiet" => quiet = true,
            "--debug-each" | "--debugEach" => req.debug_each = Some(true),
            _ if !a.starts_with('-') && req.text.is_none() => req.text = Some(a.to_string()),
            _ => {}
        }
        i += 1;
    }

    if let Some(path) = config {
        match load_json_config(&path) {
            Ok(file_req) => req = merge_req(file_req, req),
            Err(e) => {
                eprintln!("failed to load config: {e}");
                process::exit(1);
            }
        }
    }

    if req.text.is_none() && req.file.is_none() {
        eprintln!("error: provide --text or --file");
        print_help();
        process::exit(1);
    }

    let result = run_test(req);
    emit_result(&result, &output, quiet);
    if !result.success {
        process::exit(1);
    }
}

fn merge_req(mut file: TestRequest, cli: TestRequest) -> TestRequest {
    if cli.text.is_some() {
        file.text = cli.text;
    }
    if cli.file.is_some() {
        file.file = cli.file;
    }
    if cli.expected_full.is_some() {
        file.expected_full = cli.expected_full;
    }
    if cli.expected_full_file.is_some() {
        file.expected_full_file = cli.expected_full_file;
    }
    file.expected_contains = merge_opt(file.expected_contains, cli.expected_contains);
    file.expected_contains_not = merge_opt(file.expected_contains_not, cli.expected_contains_not);
    file.expected_index_of = merge_opt(file.expected_index_of, cli.expected_index_of);
    file.expected_index_of_not = merge_opt(file.expected_index_of_not, cli.expected_index_of_not);
    file.dict_entries = merge_opt(file.dict_entries, cli.dict_entries);
    file.synonym_entries = merge_opt(file.synonym_entries, cli.synonym_entries);
    file.blacklist_words = merge_opt(file.blacklist_words, cli.blacklist_words);
    if cli.debug_each.is_some() {
        file.debug_each = cli.debug_each;
    }
    if cli.output_file.is_some() {
        file.output_file = cli.output_file;
    }
    file
}

fn merge_opt<T>(a: Option<Vec<T>>, b: Option<Vec<T>>) -> Option<Vec<T>> {
    match (a, b) {
        (Some(mut x), Some(y)) => {
            x.extend(y);
            Some(x)
        }
        (None, b) => b,
        (a, None) => a,
    }
}

fn emit_result(result: &TestResult, output: &str, quiet: bool) {
    if output == "text" || output == "simple" {
        println!("{}", result.output_text);
        return;
    }
    let text = if quiet {
        serde_json::to_string(result).unwrap()
    } else {
        serde_json::to_string_pretty(result).unwrap()
    };
    println!("{text}");
}

fn run_process(args: &[String]) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_process_help();
        return;
    }
    let mut files: Vec<String> = Vec::new();
    let mut globs: Vec<String> = Vec::new();
    let mut text: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut create_dir = false;
    let mut convert_to_zh_tw = false;
    let mut crlf = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" | "--text" => {
                i += 1;
                text = args.get(i).cloned();
            }
            "-f" | "--file" => {
                i += 1;
                if let Some(f) = args.get(i) {
                    files.push(f.clone());
                }
            }
            "-g" | "--glob" => {
                i += 1;
                if let Some(g) = args.get(i) {
                    globs.push(g.clone());
                }
            }
            "-o" | "--out-dir" | "--outDir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--create-dir" | "--createDir" => create_dir = true,
            "--convert-to-zh-tw" | "--convertToZhTw" | "--zh-tw" => convert_to_zh_tw = true,
            "--crlf" => crlf = true,
            _ => {}
        }
        i += 1;
    }

    if let Some(t) = text {
        println!("{}", process_text(&t, convert_to_zh_tw, crlf));
        return;
    }

    for g in &globs {
        if let Ok(paths) = glob::glob(g) {
            for p in paths.flatten() {
                files.push(p.to_string_lossy().into_owned());
            }
        }
    }
    if files.is_empty() {
        if let Ok(paths) = glob::glob("*.txt") {
            for p in paths.flatten() {
                files.push(p.to_string_lossy().into_owned());
            }
        }
    }
    if files.is_empty() {
        eprintln!("error: provide --text, --file, or --glob");
        print_process_help();
        process::exit(1);
    }

    if let Some(dir) = &out_dir {
        if !dir.exists() {
            if create_dir {
                if let Err(e) = std::fs::create_dir_all(dir) {
                    eprintln!("cannot create out-dir: {e}");
                    process::exit(1);
                }
            } else {
                eprintln!("out-dir does not exist: {}", dir.display());
                process::exit(1);
            }
        }
    }

    let total = files.len();
    for (idx, file) in files.iter().enumerate() {
        let raw = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[{}/{}] {file}: {e}", idx + 1, total);
                continue;
            }
        };
        let out = process_text(&raw, convert_to_zh_tw, crlf);
        if out.is_empty() {
            eprintln!("[{}/{}] {file} (empty)", idx + 1, total);
            continue;
        }
        eprintln!("[{}/{}] {file}", idx + 1, total);
        let dest = if let Some(dir) = &out_dir {
            dir.join(std::path::Path::new(file).file_name().unwrap_or_default())
        } else {
            PathBuf::from(file)
        };
        if let Err(e) = std::fs::write(&dest, out) {
            eprintln!("write {}: {e}", dest.display());
        }
    }
}

fn parse_expected(s: &str) -> Vec<ExpectedItem> {
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        if let Ok(items) = serde_json::from_value::<Vec<ExpectedItem>>(v) {
            return items;
        }
    }
    vec![ExpectedItem::One(s.to_string())]
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn print_help() {
    println!(
        "\
ws-segment-rs — Chinese word segmentation (CLI / API / MCP)

Usage:
  ws-segment-rs [--text|-t <text>] [--file|-f <path>] [text]
  ws-segment-rs process [options]
  ws-segment-rs serve [--bind HOST:PORT]
  ws-segment-rs mcp

Test options (dev-segment-cli):
  -t, --text <text>           Text to segment
  -f, --file <path>           Read text from file
  -c, --config <path>         JSON config
  -o, --output <json|text>    Output format (default json)
  --output-file <path>        Write JSON result to file
  --expected-full <text>      Full-string match
  --expected-contains <json>  Ordered subsequence match
  --expected-contains-not <json>
  --expected-index-of <json>
  --expected-index-of-not <json>
  --dict-entries <json>
  --synonym-entries <json>
  --blacklist <json>
  --debug-each
  -q, --quiet
  -h, --help
"
    );
}

fn print_process_help() {
    println!(
        "\
ws-segment-rs process — rewrite files with synonym conversion

  -t, --text <text>           Process text and print
  -f, --file <path>           File to rewrite (repeatable)
  -g, --glob <pattern>        Glob of files
  -o, --out-dir <dir>         Write to directory (else overwrite)
  --create-dir                Create out-dir if missing
  --zh-tw, --convert-to-zh-tw Convert to Traditional after segment
  --crlf                      Normalize line endings to LF
"
    );
}
