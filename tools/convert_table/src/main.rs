//! Converts raw .table files directly to mmap-ready runtime format.
//!
//! Output files (per variant directory):
//!   - lexicon.fst (FST mapping keys to monotonic indices)
//!   - lexicon.dat (flat binary payloads, LXPD format)
//!
//! Variants built:
//!   - simplified: gb_char + merged + opengram + punct tables (pinyin keys)
//!   - traditional: OpenCC s2twp of simplified ∪ tsi (TW freqs overlay)
//!   - zhuyin_traditional: tsi.table with raw zhuyin keys
//!   - emoji: emoji.table (pinyin keywords)
//!   - addon/*: simplified addons; traditional/addon/*: s2twp of those

use anyhow::{Context, Result};
use ferrous_opencc::config::BuiltinConfig;
use ferrous_opencc::OpenCC;
use fst::MapBuilder;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tools_common::LexEntry;

fn parse_table_line(line: &str) -> Option<(String, String, u32, u32)> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 4 {
        return None;
    }
    let key = parts[0].to_string();
    let chars = parts[1].to_string();
    let token = parts[2].parse::<u32>().unwrap_or(0);
    let freq = parts[3].trim().parse::<u32>().unwrap_or(0);
    Some((key, chars, token, freq))
}

/// Load phrase→count from interpolation2.text `\1-gram` section.
/// Format: `\item token phrase count N`
fn load_interpolation_unigrams(path: &Path) -> Result<HashMap<String, u32>> {
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(f);
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut in_unigram = false;
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed == "\\1-gram" {
            in_unigram = true;
            continue;
        }
        if trimmed == "\\2-gram" || trimmed.starts_with("\\end") || trimmed.starts_with("\\3-gram")
        {
            break;
        }
        if !in_unigram || !trimmed.starts_with("\\item ") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 5 || parts[0] != "\\item" {
            continue;
        }
        let Some(count_idx) = parts.iter().position(|&p| p == "count") else {
            continue;
        };
        if count_idx < 3 || count_idx + 1 >= parts.len() {
            continue;
        }
        let Ok(count) = parts[count_idx + 1].parse::<u32>() else {
            continue;
        };
        let phrase = parts[2..count_idx].join("");
        if !phrase.is_empty() {
            *counts.entry(phrase).or_insert(0) += count;
        }
    }
    Ok(counts)
}

/// Flat pronunciation weight used in gb_char / opengram when no corpus count exists.
const FLAT_TABLE_WEIGHT: u32 = 100;

/// Prefer trained phrase-index unigrams over flat table pronunciation weights (~100).
fn apply_unigram_overlay(
    grouped: &mut BTreeMap<String, Vec<LexEntry>>,
    unigrams: &HashMap<String, u32>,
) -> (usize, usize) {
    let mut replaced = 0usize;
    let mut total = 0usize;
    for entries in grouped.values_mut() {
        for e in entries.iter_mut() {
            total += 1;
            if let Some(&count) = unigrams.get(&e.utf8) {
                if count > 0 {
                    e.freq = count;
                    replaced += 1;
                }
            }
        }
    }
    (replaced, total)
}

/// After overlay: demote leftover flat `100` weights that never got a corpus count.
///
/// Otherwise rare gb_char rows (e.g. `锊:100`) outrank real low-count unigrams
/// (e.g. `疟:67`) and break C candidate-bar overlap.
fn demote_unoverlaid_flat_weights(
    grouped: &mut BTreeMap<String, Vec<LexEntry>>,
    unigrams: &HashMap<String, u32>,
) -> usize {
    let mut demoted = 0usize;
    for entries in grouped.values_mut() {
        let mut changed = false;
        for e in entries.iter_mut() {
            if e.freq == FLAT_TABLE_WEIGHT && !unigrams.contains_key(&e.utf8) {
                e.freq = 1;
                demoted += 1;
                changed = true;
            }
        }
        if changed {
            entries.sort_by(|a, b| b.freq.cmp(&a.freq).then_with(|| a.utf8.cmp(&b.utf8)));
        }
    }
    demoted
}

fn overlay_paths(paths: &[&Path], grouped: &mut BTreeMap<String, Vec<LexEntry>>) -> Result<()> {
    let mut merged: HashMap<String, u32> = HashMap::new();
    for path in paths {
        if !path.exists() {
            println!("  skip missing unigram source {}", path.display());
            continue;
        }
        let counts = load_interpolation_unigrams(path)?;
        println!("  loaded {} unigrams from {}", counts.len(), path.display());
        for (phrase, count) in counts {
            merged
                .entry(phrase)
                .and_modify(|c| *c = (*c).max(count))
                .or_insert(count);
        }
    }
    if merged.is_empty() {
        println!("  warning: no unigram overlay applied");
        return Ok(());
    }
    let (replaced, total) = apply_unigram_overlay(grouped, &merged);
    let demoted = demote_unoverlaid_flat_weights(grouped, &merged);
    println!("  overlay: {replaced}/{total} entries got interpolation unigram freq");
    println!("  demoted: {demoted} leftover flat-{FLAT_TABLE_WEIGHT} weights → 1");
    Ok(())
}

fn write_grouped(grouped: &BTreeMap<String, Vec<LexEntry>>, out_dir: &Path) -> Result<()> {
    create_dir_all(out_dir)?;

    let fst_path = out_dir.join("lexicon.fst");
    let mut w = File::create(&fst_path)?;
    let mut map_builder = MapBuilder::new(&mut w)?;

    let mut payloads: Vec<&Vec<LexEntry>> = Vec::with_capacity(grouped.len());

    for (i, (k, v)) in grouped.iter().enumerate() {
        map_builder.insert(k, i as u64)?;
        payloads.push(v);
    }
    map_builder.finish()?;

    let num_keys = payloads.len() as u32;
    let mut key_offsets: Vec<u32> = Vec::with_capacity(payloads.len() + 1);
    let mut entries_data: Vec<u8> = Vec::new();

    for entries in &payloads {
        key_offsets.push(entries_data.len() as u32);
        for entry in entries.iter() {
            let bytes = entry.utf8.as_bytes();
            entries_data.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            entries_data.extend_from_slice(bytes);
            entries_data.extend_from_slice(&entry.freq.to_le_bytes());
        }
    }
    key_offsets.push(entries_data.len() as u32);

    let dat_path = out_dir.join("lexicon.dat");
    tools_common::mmap_write::write_lexicon_dat(&dat_path, num_keys, &key_offsets, &entries_data)?;

    println!(
        "  {} keys -> {} + {}",
        num_keys,
        fst_path.display(),
        dat_path.display()
    );

    Ok(())
}

fn build_lexicon<P: AsRef<Path>>(
    table_paths: &[(&str, P)],
    out_dir: &Path,
    key_type: &str,
    unigram_sources: &[&Path],
) -> Result<()> {
    let mut grouped: BTreeMap<String, Vec<LexEntry>> = BTreeMap::new();

    for (name, path) in table_paths.iter() {
        let f = File::open(path).with_context(|| format!("open {}", path.as_ref().display()))?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
            if let Some((key, chars, token, freq)) = parse_table_line(&l) {
                let actual_key = if name == &"tsi" {
                    match key_type {
                        "pinyin" => {
                            let raw = convert_zhuyin_key_to_pinyin(&key);
                            let parts: Vec<String> =
                                raw.split('\'').map(normalize_pinyin_syllable).collect();
                            parts.join("'")
                        }
                        "zhuyin" => key.clone(),
                        _ => key.clone(),
                    }
                } else {
                    key.clone()
                };
                grouped.entry(actual_key).or_default().push(LexEntry {
                    utf8: chars,
                    token,
                    freq,
                });
            }
        }
    }

    overlay_paths(unigram_sources, &mut grouped)?;
    dedupe_grouped(&mut grouped);
    write_grouped(&grouped, out_dir)
}

/// Keep highest freq per (key, phrase); stable token from the winning row.
fn dedupe_grouped(grouped: &mut BTreeMap<String, Vec<LexEntry>>) {
    for entries in grouped.values_mut() {
        let mut best: HashMap<String, LexEntry> = HashMap::new();
        for e in entries.drain(..) {
            best.entry(e.utf8.clone())
                .and_modify(|cur| {
                    if e.freq > cur.freq {
                        *cur = e.clone();
                    }
                })
                .or_insert(e);
        }
        *entries = best.into_values().collect();
        entries.sort_by(|a, b| b.freq.cmp(&a.freq).then_with(|| a.utf8.cmp(&b.utf8)));
    }
}

fn push_entry(grouped: &mut BTreeMap<String, Vec<LexEntry>>, key: String, entry: LexEntry) {
    grouped.entry(key).or_default().push(entry);
}

struct TsiIndex {
    by_key: HashMap<String, HashMap<String, u32>>,
    by_phrase: HashMap<String, u32>,
    rows: Vec<(String, String, u32, u32)>,
}

fn load_tsi_index(tsi_path: &Path) -> Result<TsiIndex> {
    let f = File::open(tsi_path).with_context(|| format!("open {}", tsi_path.display()))?;
    let mut by_key: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut by_phrase: HashMap<String, u32> = HashMap::new();
    let mut rows = Vec::new();

    for line in BufReader::new(f).lines() {
        let l = line?;
        if l.trim().is_empty() {
            continue;
        }
        let Some((zhuyin_key, phrase, token, freq)) = parse_table_line(&l) else {
            continue;
        };
        let py_key: String = convert_zhuyin_key_to_pinyin(&zhuyin_key)
            .split('\'')
            .map(normalize_pinyin_syllable)
            .collect::<Vec<_>>()
            .join("'");
        if py_key.is_empty() {
            continue;
        }
        by_key
            .entry(py_key.clone())
            .or_default()
            .entry(phrase.clone())
            .and_modify(|f| *f = (*f).max(freq))
            .or_insert(freq);
        by_phrase
            .entry(phrase.clone())
            .and_modify(|f| *f = (*f).max(freq))
            .or_insert(freq);
        rows.push((py_key, phrase, token, freq));
    }

    Ok(TsiIndex {
        by_key,
        by_phrase,
        rows,
    })
}

fn tw_freq_for(tsi: &TsiIndex, key: &str, phrase: &str, mainland_freq: u32) -> u32 {
    tsi.by_key
        .get(key)
        .and_then(|m| m.get(phrase).copied())
        .or_else(|| tsi.by_phrase.get(phrase).copied())
        .unwrap_or_else(|| (mainland_freq / 10).max(1))
}

fn build_traditional_hybrid(
    simp_tables: &[(&str, PathBuf)],
    tsi_path: &Path,
    out_dir: &Path,
    opencc: &OpenCC,
    unigram_sources: &[&Path],
) -> Result<()> {
    let tsi = load_tsi_index(tsi_path)?;
    let mut grouped: BTreeMap<String, Vec<LexEntry>> = BTreeMap::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for (_name, path) in simp_tables {
        let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
            let Some((key, chars, token, freq)) = parse_table_line(&l) else {
                continue;
            };
            let trad = opencc.convert(&chars);
            if trad.is_empty() {
                continue;
            }
            let tw_freq = tw_freq_for(&tsi, &key, &trad, freq);
            seen.insert((key.clone(), trad.clone()));
            push_entry(
                &mut grouped,
                key,
                LexEntry {
                    utf8: trad,
                    token,
                    freq: tw_freq,
                },
            );
        }
    }

    // Union: tsi-only phrases (true TW vocab not produced by s2twp of simp).
    let mut union_added = 0u32;
    for (key, phrase, token, freq) in &tsi.rows {
        if seen.insert((key.clone(), phrase.clone())) {
            push_entry(
                &mut grouped,
                key.clone(),
                LexEntry {
                    utf8: phrase.clone(),
                    token: *token,
                    freq: *freq,
                },
            );
            union_added += 1;
        }
    }

    overlay_paths(unigram_sources, &mut grouped)?;
    dedupe_grouped(&mut grouped);
    println!(
        "  hybrid: {} keys, {} tsi-only union phrases",
        grouped.len(),
        union_added
    );
    write_grouped(&grouped, out_dir)
}

fn build_addon_traditional(addon_table: &Path, out_dir: &Path, opencc: &OpenCC) -> Result<()> {
    let mut grouped: BTreeMap<String, Vec<LexEntry>> = BTreeMap::new();
    let f = File::open(addon_table)?;
    let reader = BufReader::new(f);
    for line in reader.lines() {
        let l = line?;
        if l.trim().is_empty() {
            continue;
        }
        let Some((key, chars, token, freq)) = parse_table_line(&l) else {
            continue;
        };
        let trad = opencc.convert(&chars);
        if trad.is_empty() {
            continue;
        }
        push_entry(
            &mut grouped,
            key,
            LexEntry {
                utf8: trad,
                token,
                freq: (freq / 10).max(1),
            },
        );
    }
    dedupe_grouped(&mut grouped);
    write_grouped(&grouped, out_dir)
}

// === Zhuyin -> Pinyin conversion ===

fn strip_zhuyin_tone(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(
                *c,
                '\u{02CA}' | '\u{02C7}' | '\u{02CB}' | '\u{02D9}' | '\u{0304}'
            )
        })
        .collect()
}

fn zhuyin_char_to_pinyin_fragment(ch: char) -> Option<&'static str> {
    match ch {
        'ㄅ' => Some("b"),
        'ㄆ' => Some("p"),
        'ㄇ' => Some("m"),
        'ㄈ' => Some("f"),
        'ㄉ' => Some("d"),
        'ㄊ' => Some("t"),
        'ㄋ' => Some("n"),
        'ㄌ' => Some("l"),
        'ㄍ' => Some("g"),
        'ㄎ' => Some("k"),
        'ㄏ' => Some("h"),
        'ㄐ' => Some("j"),
        'ㄑ' => Some("q"),
        'ㄒ' => Some("x"),
        'ㄓ' => Some("zh"),
        'ㄔ' => Some("ch"),
        'ㄕ' => Some("sh"),
        'ㄖ' => Some("r"),
        'ㄗ' => Some("z"),
        'ㄘ' => Some("c"),
        'ㄙ' => Some("s"),
        'ㄧ' => Some("i"),
        'ㄨ' => Some("u"),
        'ㄩ' => Some("v"),
        'ㄚ' => Some("a"),
        'ㄛ' => Some("o"),
        'ㄜ' => Some("e"),
        'ㄝ' => Some("e"),
        'ㄞ' => Some("ai"),
        'ㄟ' => Some("ei"),
        'ㄠ' => Some("ao"),
        'ㄡ' => Some("ou"),
        'ㄢ' => Some("an"),
        'ㄣ' => Some("en"),
        'ㄤ' => Some("ang"),
        'ㄥ' => Some("eng"),
        'ㄦ' => Some("er"),
        _ => None,
    }
}

fn convert_zhuyin_syllable_to_pinyin(syll: &str) -> String {
    let cleaned = strip_zhuyin_tone(syll);
    let mut out = String::new();
    for ch in cleaned.chars() {
        if let Some(frag) = zhuyin_char_to_pinyin_fragment(ch) {
            out.push_str(frag);
        }
    }

    if out.starts_with('i') && out.len() >= 2 {
        let rest = &out[1..];
        if rest.starts_with('a')
            || rest.starts_with('o')
            || rest.starts_with('e')
            || rest.starts_with('u')
            || rest.starts_with('i')
        {
            out = format!("y{}", rest);
        }
    }
    if out.starts_with('u') && out.len() >= 2 {
        let rest = &out[1..];
        if rest.starts_with('a')
            || rest.starts_with('o')
            || rest.starts_with('e')
            || rest.starts_with('i')
        {
            out = format!("w{}", rest);
        }
    }
    if out.starts_with('v') {
        let rest = &out[1..];
        out = format!("yu{}", rest);
    }

    out
}

fn convert_zhuyin_key_to_pinyin(key: &str) -> String {
    let parts: Vec<&str> = key.split('\'').collect();
    let mut out_parts: Vec<String> = Vec::new();
    for p in parts.iter() {
        let p_trim = p.trim();
        if p_trim.is_empty() {
            continue;
        }
        out_parts.push(convert_zhuyin_syllable_to_pinyin(p_trim));
    }
    out_parts.join("'")
}

fn normalize_pinyin_syllable(s: &str) -> String {
    let mut s = s.to_string();
    if s == "iou" {
        s = "iu".to_string();
    }
    if s == "uei" {
        s = "ui".to_string();
    }
    if s == "uen" {
        s = "un".to_string();
    }
    s
}

fn main() -> Result<()> {
    let data_dir = Path::new("data");
    let zhuyin_dir = Path::new("data/zhuyin");
    let out_dir = Path::new("data/converted");

    let opencc = OpenCC::from_config(BuiltinConfig::S2twp)
        .context("load OpenCC S2twp (Taiwan traditional)")?;

    // 1) Simplified pinyin
    let simplified_tables = [
        ("gb_char", data_dir.join("gb_char.table")),
        ("merged", data_dir.join("merged.table")),
        ("opengram", data_dir.join("opengram.table")),
        ("punct", data_dir.join("punct.table")),
    ];
    let mainland_unigrams = data_dir.join("interpolation2.text");
    let zhuyin_unigrams = zhuyin_dir.join("interpolation2.text");

    println!("Building simplified lexicon...");
    build_lexicon(
        &simplified_tables,
        &out_dir.join("simplified"),
        "original",
        &[mainland_unigrams.as_path()],
    )?;

    // 2) Traditional pinyin: s2twp(simp) ∪ tsi with TW freqs
    let tsi_path = zhuyin_dir.join("tsi.table");
    println!("Building traditional lexicon (s2twp + tsi freq overlay)...");
    build_traditional_hybrid(
        &simplified_tables,
        &tsi_path,
        &out_dir.join("traditional"),
        &opencc,
        &[zhuyin_unigrams.as_path(), mainland_unigrams.as_path()],
    )?;

    // 3) Zhuyin traditional (raw zhuyin keys)
    let zhuyin_tables = [("tsi", tsi_path.clone())];
    println!("Building zhuyin_traditional lexicon...");
    build_lexicon(
        &zhuyin_tables,
        &out_dir.join("zhuyin_traditional"),
        "zhuyin",
        &[zhuyin_unigrams.as_path()],
    )?;

    // 4) Emoji
    if data_dir.join("emoji.table").exists() {
        let emoji_tables = [("emoji", data_dir.join("emoji.table"))];
        println!("Building emoji lexicon...");
        // Emoji table weights are intentional; do not overlay phrase unigrams.
        build_lexicon(&emoji_tables, &out_dir.join("emoji"), "original", &[])?;
    } else {
        println!("Skipping emoji (emoji.table not found)");
    }

    // 5) Addon dictionaries (simplified + traditional s2twp)
    let addon_dir = data_dir.join("addon");
    if addon_dir.is_dir() {
        println!("\nBuilding addon dictionaries...");
        let addon_out_dir = out_dir.join("addon");
        let trad_addon_out = out_dir.join("traditional").join("addon");
        for entry in std::fs::read_dir(&addon_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("table") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let tables = [("addon", path.clone())];
                println!("  Building addon (simp): {}", name);
                build_lexicon(
                    &tables,
                    &addon_out_dir.join(&name),
                    "original",
                    &[mainland_unigrams.as_path()],
                )?;
                println!("  Building addon (trad s2twp): {}", name);
                build_addon_traditional(&path, &trad_addon_out.join(&name), &opencc)?;
            }
        }
    }

    println!("\nDone!");
    println!("Next: generate word bigrams (or run scripts/build_data.sh):");
    println!("  cargo run -p gen_word_bigrams --release -- \\");
    println!("    data/interpolation2.text data/converted/simplified");
    println!("  cargo run -p gen_word_bigrams --release -- \\");
    println!("    data/zhuyin/interpolation2.text data/converted/traditional");
    Ok(())
}
