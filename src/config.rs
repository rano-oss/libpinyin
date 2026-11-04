//! Pinyin-specific configuration (corrections, incomplete syllables, fuzzy defaults).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinyinConfig {
    /// Base configuration fields (fuzzy, weights, sorting, etc.)
    #[serde(flatten)]
    pub base: crate::core::Config,

    /// Incomplete syllable matching (e.g. "zh", "ch", "sh" without finals)
    pub pinyin_incomplete: bool,

    pub correct_ue_ve: bool,
    pub correct_v_u: bool,
    pub correct_uen_un: bool,
    pub correct_gn_ng: bool,
    pub correct_mg_ng: bool,
    pub correct_iou_iu: bool,
    pub correct_uei_ui: bool,
    pub correct_on_ong: bool,

    /// Double pinyin scheme (e.g. "Microsoft", "ZiRanMa", "XiaoHe")
    pub double_pinyin_scheme: Option<String>,
}

impl Default for PinyinConfig {
    /// Full IME profile: incomplete syllables, all corrections, full fuzzy set.
    fn default() -> Self {
        Self {
            base: crate::core::Config {
                fuzzy: default_fuzzy_rules(),
                ..crate::core::Config::default()
            },
            pinyin_incomplete: true,
            correct_ue_ve: true,
            correct_v_u: true,
            correct_uen_un: true,
            correct_gn_ng: true,
            correct_mg_ng: true,
            correct_iou_iu: true,
            correct_uei_ui: true,
            correct_on_ong: true,
            double_pinyin_scheme: None,
        }
    }
}

impl PinyinConfig {
    pub fn corrections_enabled(&self) -> bool {
        self.correct_ue_ve
            || self.correct_v_u
            || self.correct_uen_un
            || self.correct_gn_ng
            || self.correct_mg_ng
            || self.correct_iou_iu
            || self.correct_uei_ui
            || self.correct_on_ong
    }

    /// Convert into the base config for use with `Model::new()`.
    pub fn into_base(self) -> crate::core::Config {
        self.base
    }

    pub fn base(&self) -> &crate::core::Config {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut crate::core::Config {
        &mut self.base
    }
}

/// Default fuzzy equivalence rules (full AMB set used by typical full-pinyin IMEs).
fn default_fuzzy_rules() -> Vec<String> {
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
        "f=h:1.0",
        "k=g:1.0",
        "l=n:1.0",
        "l=r:1.0",
        "an=ang:1.0",
        "en=eng:1.0",
        "in=ing:1.0",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}
