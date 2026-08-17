//! Dictionary paths and CJK variant expansion matching `@lazy-cjk/zh-table-list`.

use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn dict_root() -> PathBuf {
    if let Ok(p) = std::env::var("NOVEL_SEGMENT_DICT_ROOT") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/segment-dict/dict")
}

pub fn segment_dict_root() -> PathBuf {
    dict_root().join("segment")
}
pub fn synonym_dict_root() -> PathBuf {
    dict_root().join("synonym")
}
pub fn stopword_dict_root() -> PathBuf {
    dict_root().join("stopword")
}
pub fn blacklist_dict_root() -> PathBuf {
    dict_root().join("blacklist")
}

fn opencc_dir() -> PathBuf {
    dict_root().join("opencc/dictionary")
}

struct CjkTables {
    /// char → variant chars from alias tables + OpenCC
    alias: HashMap<char, Vec<char>>,
    jp2zht: HashMap<char, char>,
    jp2zhs: HashMap<char, char>,
    zht2zhs: HashMap<char, char>,
    zhs2zht: HashMap<char, char>,
    zht2jp: HashMap<char, char>,
    zhs2jp: HashMap<char, char>,
    cn2tw: HashMap<char, char>,
    tw2cn: HashMap<char, char>,
}

fn tables() -> &'static CjkTables {
    static T: OnceLock<CjkTables> = OnceLock::new();
    T.get_or_init(load_tables)
}

fn load_tables() -> CjkTables {
    let mut alias: HashMap<char, BTreeSet<char>> = HashMap::new();
    if let Ok(v) = serde_json::from_str::<Value>(include_str!("../data/zh-table-alias.json")) {
        for key in ["table_plus", "table_jp", "table_jp_core", "table_plus_core"] {
            if let Some(obj) = v.get(key).and_then(|x| x.as_object()) {
                for (k, arr) in obj {
                    let members: Vec<char> = std::iter::once(k.chars().next())
                        .flatten()
                        .chain(
                            arr.as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|x| x.as_str().and_then(|s| s.chars().next())),
                        )
                        .collect();
                    for &m in &members {
                        alias.entry(m).or_default().extend(members.iter().copied());
                    }
                }
            }
        }
        if let Some(obj) = v.get("table_tw").and_then(|x| x.as_object()) {
            for (k, val) in obj {
                if let (Some(from), Some(to)) = (k.chars().next(), val.as_str().and_then(|s| s.chars().next())) {
                    alias.entry(from).or_default().extend([from, to]);
                    alias.entry(to).or_default().extend([from, to]);
                }
            }
        }
        if let Some(obj) = v.get("table_cn").and_then(|x| x.as_object()) {
            for (k, arr) in obj {
                let from = k.chars().next();
                let tos: Vec<char> = arr
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().and_then(|s| s.chars().next()))
                    .collect();
                if let Some(from) = from {
                    alias.entry(from).or_default().insert(from);
                    alias.entry(from).or_default().extend(tos.iter().copied());
                }
            }
        }
    }

    let mut jp2zht = HashMap::new();
    let mut jp2zhs = HashMap::new();
    let mut zht2zhs = HashMap::new();
    let mut zhs2zht = HashMap::new();
    let mut zht2jp = HashMap::new();
    let mut zhs2jp = HashMap::new();
    if let Ok(rows) = serde_json::from_str::<Vec<[String; 3]>>(include_str!("../data/jp-table-safe.json")) {
        for row in rows {
            let jp = row[0].chars().next();
            let zht = row[1].chars().next();
            let zhs = row[2].chars().next();
            if let (Some(jp), Some(zht), Some(zhs)) = (jp, zht, zhs) {
                jp2zht.insert(jp, zht);
                jp2zhs.insert(jp, zhs);
                zht2zhs.insert(zht, zhs);
                zhs2zht.insert(zhs, zht);
                zht2jp.insert(zht, jp);
                zhs2jp.insert(zhs, jp);
            }
        }
    }

    let mut cn2tw = HashMap::new();
    let mut tw2cn = HashMap::new();
    merge_opencc_pair(&mut cn2tw, &mut alias, &opencc_dir().join("STCharacters.txt"));
    merge_opencc_pair(&mut tw2cn, &mut alias, &opencc_dir().join("TSCharacters.txt"));
    for name in ["JPVariants.txt", "TWVariants.txt", "HKVariants.txt"] {
        merge_opencc_alias(&mut alias, &opencc_dir().join(name));
    }
    // Phrase tables still help single-char pairs when the source is one character.
    merge_opencc_alias(&mut alias, &opencc_dir().join("STPhrases.txt"));
    merge_opencc_alias(&mut alias, &opencc_dir().join("TSPhrases.txt"));

    CjkTables {
        alias: alias
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().filter(|c| *c != k).collect()))
            .collect(),
        jp2zht,
        jp2zhs,
        zht2zhs,
        zhs2zht,
        zht2jp,
        zhs2jp,
        cn2tw,
        tw2cn,
    }
}

fn merge_opencc_pair(map: &mut HashMap<char, char>, alias: &mut HashMap<char, BTreeSet<char>>, path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(src) = parts.next() else { continue };
        let Some(from) = src.chars().next() else { continue };
        let Some(dsts) = parts.next() else { continue };
        let dests: Vec<char> = dsts.split_whitespace().filter_map(|s| s.chars().next()).collect();
        if let Some(&first) = dests.first() {
            map.entry(from).or_insert(first);
        }
        alias.entry(from).or_default().insert(from);
        alias.entry(from).or_default().extend(dests.iter().copied());
        for &to in &dests {
            alias.entry(to).or_default().insert(from);
            alias.entry(to).or_default().insert(to);
        }
    }
}

fn merge_opencc_alias(map: &mut HashMap<char, BTreeSet<char>>, path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(src) = parts.next() else { continue };
        let Some(dsts) = parts.next() else { continue };
        let Some(from) = src.chars().next() else { continue };
        for dest in dsts.split_whitespace() {
            if let Some(to) = dest.chars().next() {
                map.entry(from).or_default().insert(to);
                map.entry(to).or_default().insert(from);
            }
        }
    }
}

fn map_or_self(map: &HashMap<char, char>, c: char) -> char {
    map.get(&c).copied().unwrap_or(c)
}

/// `@lazy-cjk/zh-table-list` `auto()` with `{ safe: true }` (no greedy table).
pub fn auto_char(ch: char) -> Vec<char> {
    let t = tables();
    let mut out = BTreeSet::new();
    out.insert(ch);
    if let Some(vs) = t.alias.get(&ch) {
        out.extend(vs.iter().copied());
    }
    let tw = map_or_self(&t.cn2tw, ch);
    let cn = map_or_self(&t.tw2cn, ch);
    out.insert(tw);
    out.insert(cn);
    let jt = map_or_self(&t.jp2zht, ch);
    let js = map_or_self(&t.jp2zhs, ch);
    out.insert(jt);
    out.insert(js);
    out.insert(map_or_self(&t.tw2cn, jt));
    out.insert(map_or_self(&t.cn2tw, js));
    if let Some(vs) = t.alias.get(&jt) {
        out.extend(vs.iter().copied());
    }
    if let Some(vs) = t.alias.get(&js) {
        out.extend(vs.iter().copied());
    }
    out.into_iter().collect()
}

/// `textList` — Cartesian product of `auto()` per character, then sorted.
pub fn expand_cjk_variants(word: &str) -> Vec<String> {
    text_list(word)
}

pub fn text_list(word: &str) -> Vec<String> {
    if word.is_empty() {
        return vec![word.to_string()];
    }
    let chars: Vec<char> = word.chars().collect();
    let opts: Vec<Vec<char>> = chars.iter().map(|c| auto_char(*c)).collect();
    let product = opts.iter().try_fold(1usize, |acc, o| acc.checked_mul(o.len()));
    if product.map(|p| p > 65_536).unwrap_or(true) {
        return arr_cjk(std::iter::once(word)).into_iter().collect();
    }
    let mut acc = vec![String::new()];
    for choices in opts {
        let mut next = Vec::with_capacity(acc.len().saturating_mul(choices.len()));
        for prefix in &acc {
            for &opt in &choices {
                let mut s = prefix.clone();
                s.push(opt);
                next.push(s);
            }
        }
        acc = next;
    }
    acc.sort();
    acc.dedup();
    if !acc.iter().any(|s| s == word) {
        acc.insert(0, word.to_string());
    }
    acc
}

fn convert_word(word: &str, f: impl Fn(char) -> char) -> String {
    word.chars().map(f).collect()
}

/// Character-level Simplified → Traditional (OpenCC STCharacters).
pub fn convert_cn2tw(text: &str) -> String {
    let t = tables();
    convert_word(text, |c| map_or_self(&t.cn2tw, c))
}

/// Character-level Traditional → Simplified (OpenCC TSCharacters).
pub fn convert_tw2cn(text: &str) -> String {
    let t = tables();
    convert_word(text, |c| map_or_self(&t.tw2cn, c))
}

/// `arrCjk` — original plus whole-string cjk2zht / cn2tw / cjk2zhs / cjk2jp.
pub fn arr_cjk<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let t = tables();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let w = item.as_ref();
        for s in [
            w.to_string(),
            convert_word(w, |c| map_or_self(&t.jp2zht, map_or_self(&t.zhs2zht, c))),
            convert_word(w, |c| map_or_self(&t.cn2tw, c)),
            convert_word(w, |c| map_or_self(&t.jp2zhs, map_or_self(&t.zht2zhs, c))),
            convert_word(w, |c| map_or_self(&t.zht2jp, map_or_self(&t.zhs2jp, c))),
        ] {
            if seen.insert(s.clone()) {
                out.push(s);
            }
        }
    }
    out
}

/// Expand every item with `textList` (dict-add path).
pub fn expand_cjk_list<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        for v in text_list(item.as_ref()) {
            if seen.insert(v.clone()) {
                out.push(v);
            }
        }
    }
    out
}

pub fn is_dict_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if name.eq_ignore_ascii_case("readme.md") || name.ends_with(".md") {
        return false;
    }
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dict_root_exists() {
        assert!(segment_dict_root().join("dict.txt").is_file());
    }

    #[test]
    fn expand_includes_original() {
        let vs = expand_cjk_variants("中国");
        assert!(vs.iter().any(|s| s == "中国"));
    }

    #[test]
    fn text_list_sima() {
        let vs = text_list("司马");
        assert!(vs.iter().any(|s| s == "司馬"), "{vs:?}");
        assert!(vs.iter().any(|s| s == "司马"), "{vs:?}");
    }

    #[test]
    fn expand_jiyu_simplified() {
        let vs = expand_cjk_variants("基於");
        assert!(vs.iter().any(|s| s == "基于"), "{vs:?}");
    }

    #[test]
    fn auto_jie_includes_borrow() {
        let vs = auto_char('藉');
        assert!(vs.contains(&'借'), "{vs:?}");
        let list = text_list("藉口");
        assert!(list.iter().any(|s| s == "借口"), "{list:?}");
    }

    #[test]
    fn arr_cjk_ouyang() {
        let vs = arr_cjk(["欧阳"]);
        assert!(vs.iter().any(|s| s == "歐陽" || s == "欧阳"), "{vs:?}");
    }
}
