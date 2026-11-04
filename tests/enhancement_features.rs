mod common;

/// Tests for parser enhancement features:
/// - Partial pinyin (incomplete syllables)
/// - Pinyin corrections (ue/ve, v/u)
///
/// These are unit tests for the parser correction methods,
/// demonstrating the new enhancement features work correctly.
use libpinyin::parser::Parser;

#[test]
fn parser_find_syllable_completion_basic() {
    use libpinyin::PINYIN_SYLLABLES;
    let parser = Parser::with_syllables(PINYIN_SYLLABLES);

    let n = parser
        .find_syllable_completion("n")
        .expect("n should complete");
    assert!(n.starts_with('n') && n.len() >= 2, "got {n}");

    let zh = parser
        .find_syllable_completion("zh")
        .expect("zh should complete");
    assert!(zh.starts_with("zh") && zh.len() > 2, "got {zh}");

    let z = parser
        .find_syllable_completion("z")
        .expect("z should complete");
    assert!(z.starts_with('z') && z.len() >= 2, "got {z}");
}

#[test]
fn parser_apply_corrections_ue_ve() {
    // Test ue <-> ve correction
    let parser = Parser::default();

    // "nue" should suggest "nve" as correction
    let corrections = parser.apply_corrections("nue");
    assert!(
        corrections.contains(&"nve".to_string()),
        "Expected 'nve' in corrections for 'nue', got: {:?}",
        corrections
    );

    // "lve" should suggest "lue" as correction
    let corrections = parser.apply_corrections("lve");
    assert!(
        corrections.contains(&"lue".to_string()),
        "Expected 'lue' in corrections for 'lve', got: {:?}",
        corrections
    );

    // "xue" should suggest "xve" as correction
    let corrections = parser.apply_corrections("xue");
    assert!(
        corrections.contains(&"xve".to_string()),
        "Expected 'xve' in corrections for 'xue', got: {:?}",
        corrections
    );
}

#[test]
fn parser_apply_corrections_v_u() {
    // Test v <-> u after n, l
    let parser = Parser::default();

    // "nv" should suggest "nu" as correction
    let corrections = parser.apply_corrections("nv");
    assert!(
        corrections.contains(&"nu".to_string()),
        "Expected 'nu' in corrections for 'nv', got: {:?}",
        corrections
    );

    // "nu" should suggest "nv" as correction
    let corrections = parser.apply_corrections("nu");
    assert!(
        corrections.contains(&"nv".to_string()),
        "Expected 'nv' in corrections for 'nu', got: {:?}",
        corrections
    );

    // "lv" should suggest "lu" as correction
    let corrections = parser.apply_corrections("lv");
    assert!(
        corrections.contains(&"lu".to_string()),
        "Expected 'lu' in corrections for 'lv', got: {:?}",
        corrections
    );

    // "lu" should suggest "lv" as correction
    let corrections = parser.apply_corrections("lu");
    assert!(
        corrections.contains(&"lv".to_string()),
        "Expected 'lv' in corrections for 'lu', got: {:?}",
        corrections
    );
}

#[test]
fn parser_apply_corrections_no_corrections() {
    // Test that normal syllables don't get spurious corrections
    let parser = Parser::default();

    // "wo" shouldn't have corrections
    let corrections = parser.apply_corrections("wo");
    assert!(
        corrections.is_empty(),
        "Expected no corrections for 'wo', got: {:?}",
        corrections
    );

    // "ai" shouldn't have corrections
    let corrections = parser.apply_corrections("ai");
    assert!(
        corrections.is_empty(),
        "Expected no corrections for 'ai', got: {:?}",
        corrections
    );

    // "zha" shouldn't have corrections (no ng, ue, ve, v, u, uen, un, iou, iu)
    let corrections = parser.apply_corrections("zha");
    assert!(
        corrections.is_empty(),
        "Expected no corrections for 'zha', got: {:?}",
        corrections
    );
}

#[test]
fn parser_corrections_are_bidirectional() {
    // Verify corrections work in both directions
    let parser = Parser::default();

    // ue -> ve and ve -> ue
    let from_ue = parser.apply_corrections("nue");
    let from_ve = parser.apply_corrections("nve");
    assert!(
        from_ue.contains(&"nve".to_string()),
        "ue should correct to ve"
    );
    assert!(
        from_ve.contains(&"nue".to_string()),
        "ve should correct to ue"
    );

    // v -> u and u -> v (after n, l)
    let from_v = parser.apply_corrections("nv");
    let from_u = parser.apply_corrections("nu");
    assert!(from_v.contains(&"nu".to_string()), "v should correct to u");
    assert!(from_u.contains(&"nv".to_string()), "u should correct to v");
}

#[test]
fn parser_apply_corrections_uen_un() {
    // Test uen <-> un correction (PINYIN_CORRECT_UEN_UN)
    let parser = Parser::default();

    // "juen" should suggest "jun" as correction
    let corrections = parser.apply_corrections("juen");
    assert!(
        corrections.contains(&"jun".to_string()),
        "Expected 'jun' in corrections for 'juen', got: {:?}",
        corrections
    );

    // "jun" should suggest "juen" as correction
    let corrections = parser.apply_corrections("jun");
    assert!(
        corrections.contains(&"juen".to_string()),
        "Expected 'juen' in corrections for 'jun', got: {:?}",
        corrections
    );

    // "chuen" should suggest "chun" as correction
    let corrections = parser.apply_corrections("chuen");
    assert!(
        corrections.contains(&"chun".to_string()),
        "Expected 'chun' in corrections for 'chuen', got: {:?}",
        corrections
    );
}

#[test]
fn parser_apply_corrections_gn_ng() {
    // Test gn <-> ng correction (PINYIN_CORRECT_GN_NG)
    let parser = Parser::default();

    // "bagn" should suggest "bang" as correction
    let corrections = parser.apply_corrections("bagn");
    assert!(
        corrections.contains(&"bang".to_string()),
        "Expected 'bang' in corrections for 'bagn', got: {:?}",
        corrections
    );

    // "bang" should suggest "bagn" as correction
    let corrections = parser.apply_corrections("bang");
    assert!(
        corrections.contains(&"bagn".to_string()),
        "Expected 'bagn' in corrections for 'bang', got: {:?}",
        corrections
    );

    // "hegn" should suggest "heng" as correction
    let corrections = parser.apply_corrections("hegn");
    assert!(
        corrections.contains(&"heng".to_string()),
        "Expected 'heng' in corrections for 'hegn', got: {:?}",
        corrections
    );
}

#[test]
fn parser_apply_corrections_mg_ng() {
    // Test mg <-> ng correction (PINYIN_CORRECT_MG_NG)
    let parser = Parser::default();

    // "bamg" should suggest "bang" as correction
    let corrections = parser.apply_corrections("bamg");
    assert!(
        corrections.contains(&"bang".to_string()),
        "Expected 'bang' in corrections for 'bamg', got: {:?}",
        corrections
    );

    // "bang" should suggest "bamg" as correction (via ng → gn → mg chain)
    let corrections = parser.apply_corrections("bang");
    // Note: "bang" generates both "bagn" and "bamg" corrections
    // We check for either to be present
    assert!(
        corrections.contains(&"bamg".to_string()) || corrections.contains(&"bagn".to_string()),
        "Expected 'bamg' or 'bagn' in corrections for 'bang', got: {:?}",
        corrections
    );

    // "hemg" should suggest "heng" as correction
    let corrections = parser.apply_corrections("hemg");
    assert!(
        corrections.contains(&"heng".to_string()),
        "Expected 'heng' in corrections for 'hemg', got: {:?}",
        corrections
    );
}

#[test]
fn parser_apply_corrections_iou_iu() {
    // Test iou <-> iu correction (PINYIN_CORRECT_IOU_IU)
    let parser = Parser::default();

    // "liou" should suggest "liu" as correction
    let corrections = parser.apply_corrections("liou");
    assert!(
        corrections.contains(&"liu".to_string()),
        "Expected 'liu' in corrections for 'liou', got: {:?}",
        corrections
    );

    // "liu" should suggest "liou" as correction
    let corrections = parser.apply_corrections("liu");
    assert!(
        corrections.contains(&"liou".to_string()),
        "Expected 'liou' in corrections for 'liu', got: {:?}",
        corrections
    );

    // "jiou" should suggest "jiu" as correction
    let corrections = parser.apply_corrections("jiou");
    assert!(
        corrections.contains(&"jiu".to_string()),
        "Expected 'jiu' in corrections for 'jiou', got: {:?}",
        corrections
    );
}

#[test]
fn incomplete_zh_sh_segment_as_units() {
    use common::c_options::{self, PINYIN_INCOMPLETE, PINYIN_OPTIONS_STRICT};
    use libpinyin::{Parser, PINYIN_SYLLABLES};
    let cfg = c_options::from_c_options(PINYIN_OPTIONS_STRICT | PINYIN_INCOMPLETE);
    let p = Parser::with_syllables_and_config(PINYIN_SYLLABLES, &cfg);
    assert!(p.find_syllable_completion("zh").is_some());
    assert!(p.find_syllable_completion("sh").is_some());
    let zh: Vec<_> = p
        .segment_best("zh", true)
        .into_iter()
        .map(|s| s.text)
        .collect();
    let sh: Vec<_> = p
        .segment_best("sh", true)
        .into_iter()
        .map(|s| s.text)
        .collect();
    assert_eq!(zh, vec!["zh".to_string()]);
    assert_eq!(sh, vec!["sh".to_string()]);
}
