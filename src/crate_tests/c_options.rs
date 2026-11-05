//! Upstream libpinyin option bits (`pinyin_custom2.h`) for C↔Rust parity tests only.
#![allow(dead_code)]

use crate::config::PinyinConfig;

pub const IS_PINYIN: u32 = 1 << 1;
pub const PINYIN_INCOMPLETE: u32 = 1 << 3;
pub const USE_TONE: u32 = 1 << 5;

pub const PINYIN_AMB_C_CH: u32 = 1 << 10;
pub const PINYIN_AMB_S_SH: u32 = 1 << 11;
pub const PINYIN_AMB_Z_ZH: u32 = 1 << 12;
pub const PINYIN_AMB_F_H: u32 = 1 << 13;
pub const PINYIN_AMB_G_K: u32 = 1 << 14;
pub const PINYIN_AMB_L_N: u32 = 1 << 15;
pub const PINYIN_AMB_L_R: u32 = 1 << 16;
pub const PINYIN_AMB_AN_ANG: u32 = 1 << 17;
pub const PINYIN_AMB_EN_ENG: u32 = 1 << 18;
pub const PINYIN_AMB_IN_ING: u32 = 1 << 19;
pub const PINYIN_AMB_ALL: u32 = 0x3FF << 10;

pub const PINYIN_CORRECT_GN_NG: u32 = 1 << 21;
pub const PINYIN_CORRECT_MG_NG: u32 = 1 << 22;
pub const PINYIN_CORRECT_IOU_IU: u32 = 1 << 23;
pub const PINYIN_CORRECT_UEI_UI: u32 = 1 << 24;
pub const PINYIN_CORRECT_UEN_UN: u32 = 1 << 25;
pub const PINYIN_CORRECT_UE_VE: u32 = 1 << 26;
pub const PINYIN_CORRECT_V_U: u32 = 1 << 27;
pub const PINYIN_CORRECT_ON_ONG: u32 = 1 << 28;
pub const PINYIN_CORRECT_ALL: u32 = 0xFF << 21;

/// Typical full-pinyin IME profile: incomplete + all fuzzy + all corrections.
pub const PINYIN_OPTIONS_FULL: u32 =
    IS_PINYIN | USE_TONE | PINYIN_INCOMPLETE | PINYIN_AMB_ALL | PINYIN_CORRECT_ALL;

/// Strict full-pinyin parse (no incomplete / fuzzy / corrections).
pub const PINYIN_OPTIONS_STRICT: u32 = IS_PINYIN | USE_TONE;

/// Build a [`PinyinConfig`] that mirrors upstream `pinyin_set_options` bits.
pub fn from_c_options(options: u32) -> PinyinConfig {
    PinyinConfig {
        base: crate::Config {
            fuzzy: fuzzy_rules_for_amb_options(options),
            ..crate::Config::default()
        },
        pinyin_incomplete: options & PINYIN_INCOMPLETE != 0,
        correct_ue_ve: options & PINYIN_CORRECT_UE_VE != 0,
        correct_v_u: options & PINYIN_CORRECT_V_U != 0,
        correct_uen_un: options & PINYIN_CORRECT_UEN_UN != 0,
        correct_gn_ng: options & PINYIN_CORRECT_GN_NG != 0,
        correct_mg_ng: options & PINYIN_CORRECT_MG_NG != 0,
        correct_iou_iu: options & PINYIN_CORRECT_IOU_IU != 0,
        correct_uei_ui: options & PINYIN_CORRECT_UEI_UI != 0,
        correct_on_ong: options & PINYIN_CORRECT_ON_ONG != 0,
    }
}

/// Encode a config back to upstream-style option bits (pinyin subset).
pub fn to_c_options(cfg: &PinyinConfig) -> u32 {
    let mut options = IS_PINYIN | USE_TONE;
    if cfg.pinyin_incomplete {
        options |= PINYIN_INCOMPLETE;
    }
    if cfg.correct_gn_ng {
        options |= PINYIN_CORRECT_GN_NG;
    }
    if cfg.correct_mg_ng {
        options |= PINYIN_CORRECT_MG_NG;
    }
    if cfg.correct_iou_iu {
        options |= PINYIN_CORRECT_IOU_IU;
    }
    if cfg.correct_uei_ui {
        options |= PINYIN_CORRECT_UEI_UI;
    }
    if cfg.correct_uen_un {
        options |= PINYIN_CORRECT_UEN_UN;
    }
    if cfg.correct_ue_ve {
        options |= PINYIN_CORRECT_UE_VE;
    }
    if cfg.correct_v_u {
        options |= PINYIN_CORRECT_V_U;
    }
    if cfg.correct_on_ong {
        options |= PINYIN_CORRECT_ON_ONG;
    }
    options |= amb_options_from_fuzzy_rules(&cfg.base.fuzzy);
    options
}

fn fuzzy_rules_for_amb_options(options: u32) -> Vec<String> {
    let mut rules = Vec::new();

    if options & PINYIN_AMB_C_CH != 0 {
        rules.extend(
            [
                "c=ch:1.0",
                "ci=chi:1.0",
                "ca=cha:1.0",
                "ce=che:1.0",
                "cu=chu:1.0",
                "cai=chai:1.0",
                "cao=chao:1.0",
                "cou=chou:1.0",
                "can=chan:1.0",
                "cen=chen:1.0",
                "cang=chang:1.0",
                "ceng=cheng:1.0",
                "cong=chong:1.0",
                "cuan=chuan:1.0",
                "cun=chun:1.0",
                "cui=chui:1.0",
                "cuo=chuo:1.0",
            ]
            .map(str::to_string),
        );
    }
    if options & PINYIN_AMB_S_SH != 0 {
        rules.extend(
            [
                "s=sh:1.0",
                "si=shi:1.0",
                "sa=sha:1.0",
                "se=she:1.0",
                "su=shu:1.0",
                "sai=shai:1.0",
                "sao=shao:1.0",
                "sou=shou:1.0",
                "san=shan:1.0",
                "sen=shen:1.0",
                "sang=shang:1.0",
                "seng=sheng:1.0",
                "song=shong:1.0",
                "suan=shuan:1.0",
                "sun=shun:1.0",
                "sui=shui:1.0",
                "suo=shuo:1.0",
            ]
            .map(str::to_string),
        );
    }
    if options & PINYIN_AMB_Z_ZH != 0 {
        rules.extend(
            [
                "z=zh:1.0",
                "zi=zhi:1.0",
                "za=zha:1.0",
                "ze=zhe:1.0",
                "zu=zhu:1.0",
                "zai=zhai:1.0",
                "zei=zhei:1.0",
                "zao=zhao:1.0",
                "zou=zhou:1.0",
                "zan=zhan:1.0",
                "zen=zhen:1.0",
                "zang=zhang:1.0",
                "zeng=zheng:1.0",
                "zong=zhong:1.0",
                "zuan=zhuan:1.0",
                "zun=zhun:1.0",
                "zui=zhui:1.0",
                "zuo=zhuo:1.0",
            ]
            .map(str::to_string),
        );
    }
    if options & PINYIN_AMB_F_H != 0 {
        rules.push("f=h:1.0".into());
    }
    if options & PINYIN_AMB_G_K != 0 {
        rules.push("k=g:1.0".into());
    }
    if options & PINYIN_AMB_L_N != 0 {
        rules.push("l=n:1.0".into());
    }
    if options & PINYIN_AMB_L_R != 0 {
        rules.push("l=r:1.0".into());
    }
    if options & PINYIN_AMB_AN_ANG != 0 {
        rules.push("an=ang:1.0".into());
        rules.extend(
            [
                "ban=bang:1.0",
                "pan=pang:1.0",
                "man=mang:1.0",
                "fan=fang:1.0",
                "dan=dang:1.0",
                "tan=tang:1.0",
                "nan=nang:1.0",
                "lan=lang:1.0",
                "gan=gang:1.0",
                "kan=kang:1.0",
                "han=hang:1.0",
                "ran=rang:1.0",
                "zan=zang:1.0",
                "can=cang:1.0",
                "san=sang:1.0",
                "zhan=zhang:1.0",
                "chan=chang:1.0",
                "shan=shang:1.0",
                "yan=yang:1.0",
                "wan=wang:1.0",
            ]
            .map(str::to_string),
        );
    }
    if options & PINYIN_AMB_EN_ENG != 0 {
        rules.push("en=eng:1.0".into());
        rules.extend(
            [
                "ben=beng:1.0",
                "pen=peng:1.0",
                "men=meng:1.0",
                "fen=feng:1.0",
                "den=deng:1.0",
                "ten=teng:1.0",
                "nen=neng:1.0",
                "len=leng:1.0",
                "gen=geng:1.0",
                "ken=keng:1.0",
                "hen=heng:1.0",
                "ren=reng:1.0",
                "zen=zeng:1.0",
                "cen=ceng:1.0",
                "sen=seng:1.0",
                "zhen=zheng:1.0",
                "chen=cheng:1.0",
                "shen=sheng:1.0",
                "wen=weng:1.0",
            ]
            .map(str::to_string),
        );
    }
    if options & PINYIN_AMB_IN_ING != 0 {
        rules.push("in=ing:1.0".into());
        rules.extend(
            [
                "bin=bing:1.0",
                "pin=ping:1.0",
                "min=ming:1.0",
                "din=ding:1.0",
                "tin=ting:1.0",
                "nin=ning:1.0",
                "lin=ling:1.0",
                "jin=jing:1.0",
                "qin=qing:1.0",
                "xin=xing:1.0",
                "yin=ying:1.0",
            ]
            .map(str::to_string),
        );
    }

    rules
}

fn amb_options_from_fuzzy_rules(rules: &[String]) -> u32 {
    let mut options = 0u32;
    let joined = rules.join(" ");
    if joined.contains("c=ch") || joined.contains("ci=chi") {
        options |= PINYIN_AMB_C_CH;
    }
    if joined.contains("s=sh") || joined.contains("si=shi") {
        options |= PINYIN_AMB_S_SH;
    }
    if joined.contains("z=zh") || joined.contains("zi=zhi") || joined.contains("zong=zhong") {
        options |= PINYIN_AMB_Z_ZH;
    }
    if joined.contains("f=h") {
        options |= PINYIN_AMB_F_H;
    }
    if joined.contains("k=g") || joined.contains("g=k") {
        options |= PINYIN_AMB_G_K;
    }
    if joined.contains("l=n") || joined.contains("n=l") {
        options |= PINYIN_AMB_L_N;
    }
    if joined.contains("l=r") || joined.contains("r=l") {
        options |= PINYIN_AMB_L_R;
    }
    if joined.contains("an=ang") || joined.contains("fan=fang") {
        options |= PINYIN_AMB_AN_ANG;
    }
    if joined.contains("en=eng") || joined.contains("fen=feng") {
        options |= PINYIN_AMB_EN_ENG;
    }
    if joined.contains("in=ing") || joined.contains("yin=ying") {
        options |= PINYIN_AMB_IN_ING;
    }
    options
}
