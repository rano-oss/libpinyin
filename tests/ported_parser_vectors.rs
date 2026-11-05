#![allow(clippy::needless_collect)]
// Ported segmentation test vectors (extracted / inspired by upstream libpinyin tests)
//
// Purpose:
// - Provide a larger set of deterministic segmentation unit tests to exercise
//   the parser DP, apostrophe handling, unknown fallback and basic fuzzy placeholder.
// - These are correctness-focused vectors to be used while iterating toward
//   parity with upstream libpinyin's parser behavior.
//
// Notes:
// - The Parser here is the workspace's pinyin parser skeleton (trie + DP).
// - Tests seed the parser with the minimal syllable set required for each case.
// - Once we import upstream generated tables (parser table + distances) we can
//   expand tests to exercise subtle distance-based tie-breakers.
//
use libpinyin::parser::Parser;

fn texts_from_seg(seg: Vec<libpinyin::parser::Syllable>) -> Vec<String> {
    seg.into_iter().map(|s| s.text).collect()
}

#[test]
fn ported_simple_nihao() {
    let parser = Parser::with_syllables(&["ni", "hao"]);
    let seg = parser.segment_best("nihao", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["ni".to_string(), "hao".to_string()]);
}

#[test]
fn ported_zhongguo() {
    let parser = Parser::with_syllables(&["zhong", "guo"]);
    let seg = parser.segment_best("zhongguo", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["zhong".to_string(), "guo".to_string()]);
}

#[test]
fn ported_xiexie() {
    let parser = Parser::with_syllables(&["xie"]);
    let seg = parser.segment_best("xiexie", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["xie".to_string(), "xie".to_string()]);
}

#[test]
fn ported_wo_ai() {
    let parser = Parser::with_syllables(&["wo", "ai"]);
    let seg = parser.segment_best("woai", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["wo".to_string(), "ai".to_string()]);
}

#[test]
fn ported_sheng_ri() {
    let parser = Parser::with_syllables(&["sheng", "ri"]);
    let seg = parser.segment_best("shengri", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["sheng".to_string(), "ri".to_string()]);
}

#[test]
fn ported_shang_hai() {
    let parser = Parser::with_syllables(&["shang", "hai"]);
    let seg = parser.segment_best("shanghai", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["shang".to_string(), "hai".to_string()]);
}

#[test]
fn ported_chang_cheng() {
    let parser = Parser::with_syllables(&["chang", "cheng"]);
    let seg = parser.segment_best("changcheng", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["chang".to_string(), "cheng".to_string()]);
}

#[test]
fn ported_zheng_fu() {
    let parser = Parser::with_syllables(&["zheng", "fu"]);
    let seg = parser.segment_best("zhengfu", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["zheng".to_string(), "fu".to_string()]);
}

#[test]
fn ported_apostrophe_split_zhe_yang() {
    // Apostrophe should act as an enforced separator.
    // Seed syllables used by this test.
    let parser = Parser::with_syllables(&["zhe", "yang"]);
    // include both forms explicitly to ensure exact matching across the separator
    let seg = parser.segment_best("zhe'yang", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["zhe".to_string(), "yang".to_string()]);
}

#[test]
fn ported_unknown_fallback_single_char() {
    // Mixed known + unknown: "ni" is known, 'x' is unknown -> fallback to single-char token
    let parser = Parser::with_syllables(&["ni"]);
    let seg = parser.segment_best("nix", false);
    let texts = texts_from_seg(seg);
    assert_eq!(texts, vec!["ni".to_string(), "x".to_string()]);
}

#[test]
fn ported_fuzzy_alternative_presence() {
    // The fuzzy map contains default pairs like zh<->z, ch<->c, sh<->s, l<->n.
    // We assert alternatives exist; this is not a scoring parity test.
    let mut p = Parser::default();
    p.insert_syllable("zh");
    p.insert_syllable("z");
    // Use the public test API to get fuzzy alternatives.
    let alts = p.fuzzy_alternatives("zh");
    assert!(alts.contains(&"z".to_string()));
    let alts2 = p.fuzzy_alternatives("z");
    assert!(alts2.contains(&"zh".to_string()));
}

#[test]
fn ported_multi_syllable_variety() {
    // A combined test covering multiple syllables in a single model.
    let parser = Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "xie", "sheng", "ri", "shang", "hai",
    ]);
    let cases = vec![
        ("nihao", vec!["ni", "hao"]),
        ("zhongguo", vec!["zhong", "guo"]),
        ("woai", vec!["wo", "ai"]),
        ("xiexie", vec!["xie", "xie"]),
        ("shengri", vec!["sheng", "ri"]),
        ("shanghai", vec!["shang", "hai"]),
    ];

    for (input, expect) in cases {
        let seg = parser.segment_best(input, false);
        let texts = texts_from_seg(seg);
        let expected: Vec<String> = expect.into_iter().map(|s| s.to_string()).collect();
        assert_eq!(texts, expected, "input: {}", input);
    }
}
