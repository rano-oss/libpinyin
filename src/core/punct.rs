//! Chinese / full-width punctuation helpers (Mac/Windows-class defaults).

/// Preferred Chinese punctuation for an ASCII key (Mac/Windows 中-mode default).
///
/// Returns the primary form that should be committed immediately without a picker.
pub fn preferred_chinese_punct(ch: char) -> Option<&'static str> {
    match ch {
        ',' => Some("，"),
        '.' => Some("。"),
        ';' => Some("；"),
        ':' => Some("："),
        '?' => Some("？"),
        '!' => Some("！"),
        '"' => Some("\u{201C}"),
        '\'' => Some("\u{2018}"),
        '(' => Some("（"),
        ')' => Some("）"),
        '[' => Some("【"),
        ']' => Some("】"),
        '{' => Some("「"),
        '}' => Some("」"),
        '<' => Some("《"),
        '>' => Some("》"),
        '~' => Some("～"),
        '\\' => Some("、"),
        '^' => Some("……"),
        '$' => Some("￥"),
        _ => None,
    }
}
