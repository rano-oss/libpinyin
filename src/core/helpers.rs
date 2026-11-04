//! Windows-style `v` / `u` input helpers (numerals, dates, symbols).

use crate::core::Candidate;

const DIGITS_CN: &[char] = &['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

fn digit_to_cn(d: u8) -> Option<char> {
    if d <= 9 {
        Some(DIGITS_CN[d as usize])
    } else {
        None
    }
}

fn digits_to_chinese(s: &str) -> Option<String> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut out = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        let d = (c as u8) - b'0';
        out.push(digit_to_cn(d)?);
    }
    Some(out)
}

fn year_forms(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(cn) = digits_to_chinese(s) {
        out.push(cn.clone());
        out.push(format!("{cn}年"));
        if s.len() == 4 {
            out.push(format!("{s}年"));
        }
    }
    out
}

/// Expand `v`-mode remainder (buffer after leading `v`) into helper candidates.
pub fn expand_v_input(rest: &str) -> Vec<Candidate> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Vec::new();
    }
    let mut texts: Vec<String> = Vec::new();
    if rest.chars().all(|c| c.is_ascii_digit()) {
        if let Some(cn) = digits_to_chinese(rest) {
            texts.push(cn);
        }
        if rest.len() == 4 {
            texts.extend(year_forms(rest));
        } else if rest.len() == 8 {
            // YYYYMMDD → 二〇二六年十月四日-style + numeric date
            let y = &rest[0..4];
            let m = &rest[4..6];
            let d = &rest[6..8];
            if let (Ok(mi), Ok(di)) = (m.parse::<u32>(), d.parse::<u32>()) {
                if (1..=12).contains(&mi) && (1..=31).contains(&di) {
                    texts.push(format!("{y}年{mi}月{di}日"));
                    if let (Some(yc), Some(mc), Some(dc)) = (
                        digits_to_chinese(y),
                        digits_to_chinese(&mi.to_string()),
                        digits_to_chinese(&di.to_string()),
                    ) {
                        texts.push(format!("{yc}年{mc}月{dc}日"));
                    }
                }
            }
        }
    }
    // Dedup preserving order
    let mut seen = std::collections::HashSet::new();
    texts
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .enumerate()
        .map(|(i, t)| Candidate::new(t, 10.0 - i as f32 * 0.1))
        .collect()
}

/// Small offline symbol table for `u`-mode (code → symbol).
fn u_symbol(code: &str) -> Option<&'static str> {
    match code.to_ascii_lowercase().as_str() {
        "heart" | "xin" => Some("♥"),
        "star" | "xing" => Some("★"),
        "smile" => Some("☺"),
        "arrow" | "right" => Some("→"),
        "left" => Some("←"),
        "up" => Some("↑"),
        "down" => Some("↓"),
        "check" | "ok" => Some("✓"),
        "cross" | "x" => Some("✗"),
        "dot" => Some("·"),
        "deg" | "degree" => Some("°"),
        "celsius" | "sheshi" => Some("℃"),
        "euro" => Some("€"),
        "pound" => Some("£"),
        "yen" | "rmb" | "yuan" => Some("¥"),
        "copy" | "copyright" => Some("©"),
        "tm" => Some("™"),
        "reg" => Some("®"),
        "infty" | "inf" => Some("∞"),
        "neq" => Some("≠"),
        "leq" => Some("≤"),
        "geq" => Some("≥"),
        "plusminus" | "pm" => Some("±"),
        "multiply" | "times" => Some("×"),
        "divide" => Some("÷"),
        "section" => Some("§"),
        "para" | "pilcrow" => Some("¶"),
        "bullet" => Some("•"),
        "ellipsis" | "ddd" => Some("…"),
        "dash" | "em" => Some("—"),
        "square" => Some("■"),
        "circle" => Some("●"),
        "diamond" => Some("◆"),
        "spade" => Some("♠"),
        "club" => Some("♣"),
        "diamond2" => Some("♦"),
        "male" => Some("♂"),
        "female" => Some("♀"),
        "sun" => Some("☀"),
        "moon" => Some("☾"),
        "cloud" => Some("☁"),
        "music" => Some("♪"),
        "phone" => Some("☎"),
        "email" | "mail" => Some("✉"),
        "warn" => Some("⚠"),
        "info" => Some("ℹ"),
        _ => None,
    }
}

/// Expand `u`-mode remainder into symbol candidates (exact + prefix matches).
pub fn expand_u_input(rest: &str) -> Vec<Candidate> {
    let rest = rest.trim().to_ascii_lowercase();
    if rest.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(sym) = u_symbol(&rest) {
        out.push(Candidate::new(sym, 20.0));
    }
    // Prefix suggestions from a fixed code list
    const CODES: &[&str] = &[
        "heart", "star", "smile", "arrow", "left", "up", "down", "check", "cross", "dot", "degree",
        "celsius", "euro", "pound", "yen", "yuan", "copy", "tm", "reg", "infty", "neq", "leq",
        "geq", "pm", "times", "divide", "section", "bullet", "ellipsis", "dash", "square",
        "circle", "diamond", "spade", "club", "music", "phone", "email", "warn",
    ];
    for code in CODES {
        if code.starts_with(&rest) && *code != rest.as_str() {
            if let Some(sym) = u_symbol(code) {
                out.push(Candidate::new(sym, 5.0));
            }
        }
        if out.len() >= 8 {
            break;
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.text.clone()));
    out.truncate(8);
    out
}
