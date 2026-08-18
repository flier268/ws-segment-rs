//! JS `String` helpers: length / slice use UTF-16 code units.

use once_cell::sync::Lazy;
use regex::Regex;

/// True when every code point is BMP (one UTF-16 unit). Fast path for typical CJK/ASCII.
#[inline]
pub fn is_bmp(s: &str) -> bool {
    // UTF-8 encodes non-BMP as 4-byte sequences (leading byte >= 0xF0).
    !s.as_bytes().iter().any(|&b| b >= 0xF0)
}

/// JS `String.length` (UTF-16 code units).
#[inline]
pub fn char_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else if is_bmp(s) {
        s.chars().count()
    } else {
        s.chars().map(char::len_utf16).sum()
    }
}

pub fn char_slice(s: &str, start: usize, len: usize) -> String {
    if len == 0 {
        return String::new();
    }
    if s.is_ascii() {
        if start >= s.len() {
            return String::new();
        }
        let end = (start + len).min(s.len());
        return s[start..end].to_string();
    }
    if is_bmp(s) {
        return s.chars().skip(start).take(len).collect();
    }
    let units: Vec<u16> = s.encode_utf16().collect();
    if start >= units.len() {
        return String::new();
    }
    let end = (start + len).min(units.len());
    String::from_utf16_lossy(&units[start..end])
}

pub fn char_substr_from(s: &str, start: usize) -> String {
    if s.is_ascii() {
        if start >= s.len() {
            return String::new();
        }
        return s[start..].to_string();
    }
    if is_bmp(s) {
        return s.chars().skip(start).collect();
    }
    let units: Vec<u16> = s.encode_utf16().collect();
    if start >= units.len() {
        return String::new();
    }
    String::from_utf16_lossy(&units[start..])
}

/// Slice already-materialized BMP code points (1:1 with UTF-16 units).
#[inline]
pub fn slice_chars(chars: &[char], start: usize, len: usize) -> Option<String> {
    let end = start.checked_add(len)?;
    if end > chars.len() {
        None
    } else {
        Some(chars[start..end].iter().collect())
    }
}

/// Slice already-materialized UTF-16 units.
#[inline]
pub fn slice_utf16(units: &[u16], start: usize, len: usize) -> Option<String> {
    let end = start.checked_add(len)?;
    if end > units.len() {
        None
    } else {
        Some(String::from_utf16_lossy(&units[start..end]))
    }
}

/// JS `charAt`: one UTF-16 unit. BMP CJK is a single `char`.
pub fn char_at(s: &str, i: usize) -> Option<char> {
    if s.is_ascii() {
        return s.as_bytes().get(i).copied().map(|b| b as char);
    }
    if is_bmp(s) {
        return s.chars().nth(i);
    }
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
    fn bmp_slice_matches_utf16() {
        let s = "里面开发abc";
        assert_eq!(char_slice(s, 0, 2), "里面");
        assert_eq!(char_slice(s, 2, 2), "开发");
        assert_eq!(char_substr_from(s, 4), "abc");
        assert_eq!(char_at(s, 0), Some('里'));
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(slice_chars(&chars, 0, 2).as_deref(), Some("里面"));
    }

    #[test]
    fn ascii_fast_path() {
        let s = "http://example.com/path";
        assert_eq!(char_len(s), s.len());
        assert_eq!(char_slice(s, 0, 7), "http://");
        assert_eq!(char_at(s, 4), Some(':'));
    }

    #[test]
    fn is_bmp_detects_non_bmp() {
        assert!(is_bmp(""));
        assert!(is_bmp("里面abc"));
        assert!(!is_bmp("😀"));
    }

    #[test]
    fn non_bmp_slice_uses_utf16_units() {
        let s = "😀a";
        assert_eq!(char_len(s), 3);
        assert_eq!(char_slice(s, 0, 2), "😀");
        assert_eq!(char_slice(s, 2, 1), "a");
        assert_eq!(char_substr_from(s, 2), "a");
        assert_eq!(
            slice_utf16(&s.encode_utf16().collect::<Vec<_>>(), 0, 2).as_deref(),
            Some("😀")
        );
    }

    #[test]
    fn repeat_run() {
        let s = "啊啊啊啊啊啊啊啊x";
        assert_eq!(leading_repeat_run(s), Some(8));
    }
}
