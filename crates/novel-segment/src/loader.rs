//! Dictionary file parsers.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Parsed `詞|詞性|詞權值` row.
#[derive(Clone, Debug, PartialEq)]
pub struct DictRow {
    pub w: String,
    pub p: u32,
    pub f: f64,
}

pub fn parse_pos(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>()
            .ok()
            .or_else(|| s.parse::<f64>().ok().map(|n| n as u32))
    }
}

pub fn parse_dict_line(line: &str) -> Option<DictRow> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
        return None;
    }
    let mut parts = line.split('|');
    let w = parts.next()?.trim();
    if w.is_empty() {
        return None;
    }
    let p = parts.next().and_then(parse_pos).unwrap_or(0);
    let f = parts
        .next()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    Some(DictRow {
        w: w.to_string(),
        p,
        f,
    })
}

pub fn load_dict_file(path: &Path) -> Result<Vec<DictRow>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text.lines().filter_map(parse_dict_line).collect())
}

pub fn load_line_file(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .collect())
}

/// Synonym line: `正字,錯字,...` (2.0 format).
pub fn parse_synonym_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
        return None;
    }
    let mut parts: Vec<String> = line
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let canonical = parts.remove(0);
    Some((canonical, parts))
}

pub fn load_synonym_file(path: &Path) -> Result<Vec<(String, Vec<String>)>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text.lines().filter_map(parse_synonym_line).collect())
}

pub fn glob_txt_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.eq_ignore_ascii_case("readme.md") || name.ends_with(".md") {
                continue;
            }
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

pub fn resolve_named(root: &Path, name: &str) -> Result<Vec<PathBuf>> {
    if name.contains('*') {
        let parent = name.rsplit_once('/').map(|(a, _)| a).unwrap_or("");
        let dir = if parent.is_empty() {
            root.to_path_buf()
        } else {
            root.join(parent)
        };
        let files = glob_txt_files(&dir)?;
        if files.is_empty() {
            return Err(Error::DictNotFound {
                name: name.to_string(),
            });
        }
        return Ok(files);
    }

    let candidates = [
        root.join(name),
        root.join(format!("{name}.txt")),
        root.join(format!("{name}.utf8")),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(vec![c]);
        }
    }
    Err(Error::DictNotFound {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_row() {
        let row = parse_dict_line("一会|0x300000|33").unwrap();
        assert_eq!(row.w, "一会");
        assert_eq!(row.p, 0x300000);
        assert_eq!(row.f, 33.0);
    }

    #[test]
    fn parse_synonym() {
        let (c, vs) = parse_synonym_line("一併,一並,一并").unwrap();
        assert_eq!(c, "一併");
        assert_eq!(vs, vec!["一並", "一并"]);
    }
}
