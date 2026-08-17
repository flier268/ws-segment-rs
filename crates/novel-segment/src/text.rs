//! JS `String` helpers: length / slice use UTF-16 code units.

use once_cell::sync::Lazy;
use regex::Regex;

/// JS `String.length` (UTF-16 code units).
pub fn char_len(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn char_slice(s: &str, start: usize, len: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().skip(start).take(len).collect();
    String::from_utf16_lossy(&units)
}

pub fn char_substr_from(s: &str, start: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().skip(start).collect();
    String::from_utf16_lossy(&units)
}

/// JS `charAt`: one UTF-16 unit. BMP CJK is a single `char`.
pub fn char_at(s: &str, i: usize) -> Option<char> {
    let u = s.encode_utf16().nth(i)?;
    char::decode_utf16(std::iter::once(u)).next()?.ok()
}

/// Split using JS `/([\r\n]+|^[　\s]+|[　\s]+$|[　\s]{2,})/gm`.
pub fn split_sections(text: &str) -> Vec<String> {
    static RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?m)([\r\n]+|^[　\s]+|[　\s]+$|[　\s]{2,})").unwrap());
    let mut out = Vec::new();
    let mut last = 0usize;
    for m in RE.find_iter(text) {
        if m.start() > last {
            out.push(text[last..m.start()].to_string());
        }
        out.push(m.as_str().to_string());
        last = m.end();
    }
    if last < text.len() {
        out.push(text[last..].to_string());
    }
    if out.is_empty() {
        vec![text.to_string()]
    } else {
        out
    }
}

pub fn is_newline_only(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c == '\r' || c == '\n')
}

/// Repeated-prefix match for `^((.+)\2{5,})` (same chunk 6+ times).
pub fn leading_repeat_run(text: &str) -> Option<usize> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let n = units.len();
    if n < 6 {
        return None;
    }
    for unit in 1..=n / 6 {
        if n < unit * 6 {
            continue;
        }
        let pattern = &units[..unit];
        let mut times = 1;
        let mut pos = unit;
        while pos + unit <= n && &units[pos..pos + unit] == pattern {
            times += 1;
            pos += unit;
        }
        if times >= 6 {
            return Some(pos);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_newlines() {
        let parts = split_sections("a\nb");
        assert_eq!(parts, vec!["a", "\n", "b"]);
    }

    #[test]
    fn split_multiline_single_space() {
        assert_eq!(split_sections("a \nb"), vec!["a", " ", "\n", "b"]);
        assert_eq!(split_sections("a\n b"), vec!["a", "\n", " ", "b"]);
    }

    #[test]
    fn utf16_len_emoji() {
        assert_eq!(char_len("😀"), 2);
        assert_eq!(char_len("中"), 1);
    }

    #[test]
    fn repeat_run() {
        let s = "啊啊啊啊啊啊啊啊x";
        assert_eq!(leading_repeat_run(s), Some(8));
    }
}
