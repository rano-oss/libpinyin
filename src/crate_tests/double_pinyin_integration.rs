//! Integration tests for double pinyin (shuangpin) support
//!
//! Tests the complete workflow from double pinyin input to segmentation.

use crate::double_pinyin::{double_to_full_pinyin, get_scheme_data, DoublePinyinScheme};
use crate::parser::Parser;

#[test]
fn test_microsoft_scheme_basic_conversion() {
    // Test Microsoft scheme: "ui" -> "shi", "hf" -> "hen"
    let scheme = DoublePinyinScheme::Microsoft;
    let data = get_scheme_data(&scheme);

    // "ui" -> "shi" (u=sh, i=i)
    let result = double_to_full_pinyin('u', 'i', &data);
    assert_eq!(result, Some("shi".to_string()));

    // "hf" -> "hen" (h=h, f=en)
    let result = double_to_full_pinyin('h', 'f', &data);
    assert_eq!(result, Some("hen".to_string()));

    // "aa" -> "a" (special case)
    let result = double_to_full_pinyin('a', 'a', &data);
    assert_eq!(result, Some("a".to_string()));
}

#[test]
fn test_ziranma_scheme_basic_conversion() {
    // Test ZiRanMa scheme differences from Microsoft
    let scheme = DoublePinyinScheme::ZiRanMa;
    let data = get_scheme_data(&scheme);

    // C ZRM: u=sh, h=ang → shang; g=eng → sheng
    let result = double_to_full_pinyin('u', 'h', &data);
    assert_eq!(result, Some("shang".to_string()));
    let result = double_to_full_pinyin('u', 'g', &data);
    assert_eq!(result, Some("sheng".to_string()));
}

#[test]
fn test_invalid_combinations() {
    let scheme = DoublePinyinScheme::Microsoft;
    let data = get_scheme_data(&scheme);

    // Invalid combinations should return None
    // Numbers and special characters should fail
    let result = double_to_full_pinyin('1', '2', &data);
    assert_eq!(result, None);

    // Non-alpha characters
    let result = double_to_full_pinyin('#', '@', &data);
    assert_eq!(result, None);
}

#[test]
fn test_parser_integration_microsoft() {
    // Create parser with some common syllables
    let parser = Parser::with_syllables(&[
        "ni", "hao", "shi", "jie", "zhong", "guo", "a", "e", "hen", "bang",
    ]);

    // Test conversion through parser
    // Microsoft: "ui" = "shi", "hf" = "hen"
    let converted = parser.convert_double_pinyin("uihf", "microsoft");
    assert_eq!(converted, Some("shihen".to_string()));

    // Test with punctuation
    let converted = parser.convert_double_pinyin("ui,hf", "microsoft");
    assert_eq!(converted, Some("shi,hen".to_string()));
}

#[test]
fn test_parser_integration_ziranma() {
    let parser = Parser::with_syllables(&["ni", "hao", "shi", "jie", "zhong", "guo", "shang"]);

    // ZiRanMa: uh → shang (matches C double_pinyin_zrm_*)
    let converted = parser.convert_double_pinyin("uh", "ziranma");
    assert_eq!(converted, Some("shang".to_string()));
}

#[test]
fn test_segment_with_scheme_microsoft() {
    // Create parser with common syllables
    let mut parser = Parser::default();
    parser.insert_syllable("shi");
    parser.insert_syllable("hen");
    parser.insert_syllable("bang");

    // Test segment_with_scheme using Microsoft double pinyin
    // "uihfbh" in Microsoft = "shi" + "hen" + "bang"
    // ui=shi, hf=hen, bh=bang (b=b, h=ang)
    let segmentation = parser.segment_with_scheme("uihfbh", false, Some("microsoft"));

    // Should segment into syllables
    assert!(!segmentation.is_empty());
    // First syllable should be "shi"
    assert_eq!(segmentation[0].text, "shi");
}

#[test]
fn test_segment_with_scheme_fallback() {
    // Create parser with standard pinyin syllables
    let mut parser = Parser::default();
    parser.insert_syllable("ni");
    parser.insert_syllable("hao");

    // If double pinyin conversion fails, should fall back to standard parsing
    // "nihao" is valid standard pinyin
    let segmentation = parser.segment_with_scheme("nihao", false, Some("microsoft"));

    // Should still segment (either converted or fallback)
    assert!(!segmentation.is_empty());
}

#[test]
fn test_segment_with_scheme_none() {
    // Create parser with standard pinyin syllables
    let mut parser = Parser::default();
    parser.insert_syllable("ni");
    parser.insert_syllable("hao");

    // With scheme=None, should use standard pinyin parsing
    let segmentation = parser.segment_with_scheme("nihao", false, None);

    assert_eq!(segmentation.len(), 2);
    assert_eq!(segmentation[0].text, "ni");
    assert_eq!(segmentation[1].text, "hao");
}

#[test]
fn test_invalid_scheme_fallback() {
    let parser = Parser::with_syllables(&["ni", "hao"]);

    // Invalid scheme name should return None and fall back to standard parsing
    let converted = parser.convert_double_pinyin("nihao", "invalid_scheme");
    assert_eq!(converted, None);
}

#[test]
fn test_special_vowels_microsoft() {
    let scheme = DoublePinyinScheme::Microsoft;
    let data = get_scheme_data(&scheme);

    // Special cases: aa, ee, oo, etc.
    assert_eq!(
        double_to_full_pinyin('a', 'a', &data),
        Some("a".to_string())
    );
    assert_eq!(
        double_to_full_pinyin('e', 'e', &data),
        Some("e".to_string())
    );
    assert_eq!(
        double_to_full_pinyin('o', 'o', &data),
        Some("o".to_string())
    );
}

#[test]
fn test_empty_input() {
    let parser = Parser::default();

    // Empty input should return empty conversion
    let converted = parser.convert_double_pinyin("", "microsoft");
    assert_eq!(converted, Some("".to_string()));
}

#[test]
fn test_mixed_content() {
    let parser = Parser::with_syllables(&["shi", "hen", "bang"]);

    // Test with numbers and punctuation mixed in
    let converted = parser.convert_double_pinyin("ui123hf!bh", "microsoft");
    // Should convert letters and pass through numbers/punctuation
    assert!(converted.is_some());
    let result = converted.unwrap();
    assert!(result.contains("shi"));
    assert!(result.contains("123"));
    assert!(result.contains("!"));
}

#[test]
fn test_xiaohe_scheme() {
    // Test XiaoHe scheme which has some differences from Microsoft
    let scheme = DoublePinyinScheme::XiaoHe;
    let data = get_scheme_data(&scheme);

    // XiaoHe: "ui" -> "shi", "w" maps to "ei" (different from Microsoft)
    let result = double_to_full_pinyin('u', 'i', &data);
    assert_eq!(result, Some("shi".to_string()));

    // Test a difference: w=ei in XiaoHe
    let result = double_to_full_pinyin('l', 'w', &data);
    assert_eq!(result, Some("lei".to_string()));
}

#[test]
fn test_abc_scheme() {
    // Test ABC scheme which has different shengmu mappings
    let scheme = DoublePinyinScheme::ABC;
    let data = get_scheme_data(&scheme);

    // ABC uses 'a' for zh, 'e' for ch, 'v' for sh (different from Microsoft)
    // "ai" -> "zhi" (a=zh, i=i)
    let result = double_to_full_pinyin('a', 'i', &data);
    assert_eq!(result, Some("zhi".to_string()));

    // "ei" -> "chi" (e=ch, i=i)
    let result = double_to_full_pinyin('e', 'i', &data);
    assert_eq!(result, Some("chi".to_string()));
}

#[test]
fn test_all_schemes_available() {
    // Verify all 6 schemes can be instantiated
    let schemes = vec![
        DoublePinyinScheme::Microsoft,
        DoublePinyinScheme::ZiRanMa,
        DoublePinyinScheme::ZiGuang,
        DoublePinyinScheme::ABC,
        DoublePinyinScheme::XiaoHe,
        DoublePinyinScheme::PinyinJiaJia,
    ];

    for scheme in schemes {
        let data = get_scheme_data(&scheme);
        // Verify scheme has name
        assert!(!data.name.is_empty());
        // Verify scheme has mappings
        assert!(!data.yunmu.is_empty());
    }
}
