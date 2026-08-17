//! Built-in punctuation / separator characters for PunctuationTokenizer.

use once_cell::sync::Lazy;
use std::collections::{BTreeMap, HashMap};

const TABLE: &str = concat!(
    r#" ,.;+-|/\'":?<>[]{}=!@#$%^&*()~`"#,
    "。，、＇：∶；?‘’“”〝〞ˆˇ﹕︰﹔﹖﹑·¨….¸;！´？！～—ˉ｜‖＂〃｀@﹫¡¿﹏﹋﹌︴々﹟#﹩$﹠&﹪%*﹡﹢﹦",
    "﹤‐￣¯―﹨ˆ˜﹍﹎+=<­＿_-",
    r"\",
    "ˇ~﹉﹊（）〈〉‹›﹛﹜『』〖〗［］《》〔〕{}「」【】︵︷︿︹︽_﹁﹃︻︶︸",
    "﹀︺︾ˉ﹂﹄︼＋－×÷﹢﹣±／＝≈≡≠∧∨∑∏∪∩∈⊙⌒⊥∥∠∽≌＜＞≤≥≮≯∧∨√﹙﹚[]﹛﹜∫∮∝∞⊙∏",
    "┌┬┐┏┳┓╒╤╕─│├┼┤┣╋┫╞╪╡━┃└┴┘┗┻┛╘╧╛┄┆┅┇╭─╮┏━┓╔╦╗┈┊│╳│┃┃╠╬╣┉┋╰─╯┗━┛",
    "╚╩╝╲╱┞┟┠┡┢┦┧┨┩┪╉╊┭┮┯┰┱┲┵┶┷┸╇╈┹┺┽┾┿╀╁╂╃╄╅╆",
    "○◇□△▽☆●◆■▲▼★♠♥♦♣☼☺◘♀√☻◙♂×▁▂▃▄▅▆▇█⊙◎۞卍卐╱╲▁▏↖↗↑←↔◤◥╲╱▔▕↙↘↓→↕◣◢∷▒░℡™",
    "．・　※",
    "⋯",
    "丶",
);

/// length → punctuation → length
pub static STOPWORD2: Lazy<BTreeMap<usize, HashMap<String, usize>>> = Lazy::new(|| {
    let mut map: BTreeMap<usize, HashMap<String, usize>> = BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    for ch in TABLE.chars() {
        let w = ch.to_string();
        if w.is_empty() || !seen.insert(w.clone()) {
            continue;
        }
        let len = crate::text::char_len(&w);
        map.entry(len).or_default().insert(w, len);
    }
    map
});
