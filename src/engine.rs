//! Pinyin input method engine
//!
//! Provides a high-level Engine that combines parser, model, and fuzzy matching
//! into a simple `input(text) -> Vec<Candidate>` API with caching optimization.
//!
//! This is now a thin wrapper around the generic core::Engine<Parser>.

use std::error::Error;
use std::ops::{Deref, DerefMut};

use crate::core::{EmojiDict, EnglishDict, Lexicon, Model, UserDict, WordBigram};
use crate::parser::Parser;

/// Public engine for libpinyin.
///
/// This wraps the generic core::Engine<Parser> with pinyin-specific loading logic.
/// All actual IME logic (segmentation, fuzzy matching, caching, scoring) is in core.
///
pub struct Engine {
    inner: crate::core::Engine<Parser>,
}

/// All standard pinyin syllables (without tone markers).
/// This list includes all valid pinyin syllables in Mandarin Chinese.
pub const PINYIN_SYLLABLES: &[&str] = &[
    "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei", "ben", "beng", "bi",
    "bian", "biao", "bie", "bin", "bing", "bo", "bu", "ca", "cai", "can", "cang", "cao", "ce",
    "cen", "ceng", "cha", "chai", "chan", "chang", "chao", "che", "chen", "cheng", "chi", "chong",
    "chou", "chu", "chuai", "chuan", "chuang", "chui", "chun", "chuo", "ci", "cong", "cou", "cu",
    "cuan", "cui", "cun", "cuo", "da", "dai", "dan", "dang", "dao", "de", "dei", "deng", "di",
    "dia", "dian", "diao", "die", "ding", "diu", "dong", "dou", "du", "duan", "dui", "dun", "duo",
    "e", "ei", "en", "er", "fa", "fan", "fang", "fei", "fen", "feng", "fo", "fou", "fu", "ga",
    "gai", "gan", "gang", "gao", "ge", "gei", "gen", "geng", "gong", "gou", "gu", "gua", "guai",
    "guan", "guang", "gui", "gun", "guo", "ha", "hai", "han", "hang", "hao", "he", "hei", "hen",
    "heng", "hong", "hou", "hu", "hua", "huai", "huan", "huang", "hui", "hun", "huo", "ji", "jia",
    "jian", "jiang", "jiao", "jie", "jin", "jing", "jiong", "jiu", "ju", "juan", "jue", "jun",
    "ka", "kai", "kan", "kang", "kao", "ke", "ken", "keng", "kong", "kou", "ku", "kua", "kuai",
    "kuan", "kuang", "kui", "kun", "kuo", "la", "lai", "lan", "lang", "lao", "le", "lei", "leng",
    "li", "lia", "lian", "liang", "liao", "lie", "lin", "ling", "liu", "lo", "long", "lou", "lu",
    "luan", "lun", "luo", "lv", "lve", "ma", "mai", "man", "mang", "mao", "me", "mei", "men",
    "meng", "mi", "mian", "miao", "mie", "min", "ming", "miu", "mo", "mou", "mu", "na", "nai",
    "nan", "nang", "nao", "ne", "nei", "nen", "neng", "ng", "ni", "nian", "niang", "niao", "nie",
    "nin", "ning", "niu", "nong", "nou", "nu", "nuan", "nuo", "nv", "nve", "o", "ou", "pa", "pai",
    "pan", "pang", "pao", "pei", "pen", "peng", "pi", "pian", "piao", "pie", "pin", "ping", "po",
    "pou", "pu", "qi", "qia", "qian", "qiang", "qiao", "qie", "qin", "qing", "qiong", "qiu", "qu",
    "quan", "que", "qun", "ran", "rang", "rao", "re", "ren", "reng", "ri", "rong", "rou", "ru",
    "ruan", "rui", "run", "ruo", "sa", "sai", "san", "sang", "sao", "se", "sen", "seng", "sha",
    "shai", "shan", "shang", "shao", "she", "shei", "shen", "sheng", "shi", "shou", "shu", "shua",
    "shuai", "shuan", "shuang", "shui", "shun", "shuo", "si", "song", "sou", "su", "suan", "sui",
    "sun", "suo", "ta", "tai", "tan", "tang", "tao", "te", "teng", "ti", "tian", "tiao", "tie",
    "ting", "tong", "tou", "tu", "tuan", "tui", "tun", "tuo", "wa", "wai", "wan", "wang", "wei",
    "wen", "weng", "wo", "wu", "xi", "xia", "xian", "xiang", "xiao", "xie", "xin", "xing", "xiong",
    "xiu", "xu", "xuan", "xue", "xun", "ya", "yan", "yang", "yao", "ye", "yi", "yin", "ying", "yo",
    "yong", "you", "yu", "yuan", "yue", "yun", "za", "zai", "zan", "zang", "zao", "ze", "zei",
    "zen", "zeng", "zha", "zhai", "zhan", "zhang", "zhao", "zhe", "zhen", "zheng", "zhi", "zhong",
    "zhou", "zhu", "zhua", "zhuai", "zhuan", "zhuang", "zhui", "zhun", "zhuo", "zi", "zong", "zou",
    "zu", "zuan", "zui", "zun", "zuo",
];

impl Engine {
    /// Construct an Engine from a pre-built `Model` and a `Parser`.
    ///
    /// Uses standard pinyin fuzzy rules configured in the parser.
    pub fn new(model: Model) -> Self {
        let parser = Parser::with_syllables(PINYIN_SYLLABLES);
        Self {
            inner: crate::core::Engine::new(model, parser),
        }
    }

    /// Consume this wrapper and return the core engine.
    pub fn into_inner(self) -> crate::core::Engine<Parser> {
        self.inner
    }

    /// Build an [`ImeEngine`](crate::core::ImeEngine) with the given candidate page size.
    pub fn into_ime(self, page_size: usize) -> crate::core::ImeEngine<Parser> {
        crate::core::ImeEngine::with_page_size(self.into_inner(), page_size)
    }

    /// Load an engine from a model directory containing runtime artifacts.
    ///
    /// Expected layout (data-dir):
    ///  - lexicon.fst + lexicon.dat        (mmap lexicon)
    ///  - word_bigram.dat + word_bigram_words.fst  (mmap word bigram)
    ///  - userdict.redb                    (persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Load lexicon (mmap)
        let fst_path = data_dir.join("lexicon.fst");
        let dat_path = data_dir.join("lexicon.dat");
        let lexicon = Lexicon::load(&fst_path, &dat_path)
            .map_err(|e| format!("failed to load lexicon from {:?}: {}", data_dir, e))?;

        // Load word bigram (mmap)
        let wb_dat_path = data_dir.join("word_bigram.dat");
        let wb_fst_path = data_dir.join("word_bigram_words.fst");
        let word_bigram = WordBigram::load(&wb_dat_path, &wb_fst_path)
            .map_err(|e| format!("failed to load word_bigram from {:?}: {}", data_dir, e))?;

        // Userdict: `LIBCHINESE_USERDICT` override (tests / nested), else beside an
        // explicit `PINYINWL_DATA_DIR` so a private lexicon copy does not still fight
        // the host lock on ~/.pinyin/userdict.redb, else ~/.pinyin/userdict.redb.
        let userdict = {
            let ud_path = if let Ok(p) = std::env::var("LIBCHINESE_USERDICT") {
                std::path::PathBuf::from(p)
            } else if std::env::var_os("PINYINWL_DATA_DIR").is_some() {
                data_dir.join("userdict.redb")
            } else {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                std::path::PathBuf::from(home)
                    .join(".pinyin")
                    .join("userdict.redb")
            };

            if let Some(parent) = ud_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            UserDict::new(&ud_path)?
        };

        let model = Model::new(
            lexicon,
            word_bigram,
            userdict,
            crate::core::Config::default(),
        );

        // Aux tables may live beside the lexicon (installed simplified/), in the
        // parent data/ dir, or in the source-tree grandparent (converted/simplified
        // → data/). Also check the user-local share path so English can be
        // installed without writing to /usr.
        let mut aux_dirs = vec![data_dir.to_path_buf()];
        if let Some(parent) = data_dir.parent() {
            aux_dirs.push(parent.to_path_buf());
            if let Some(grandparent) = parent.parent() {
                aux_dirs.push(grandparent.to_path_buf());
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let local = std::path::PathBuf::from(home).join(".local/share/libpinyin/data");
            aux_dirs.push(local.join("simplified"));
            aux_dirs.push(local.join("traditional"));
            aux_dirs.push(local);
        }
        // Dedup while preserving order
        {
            let mut seen = std::collections::HashSet::new();
            aux_dirs.retain(|d| seen.insert(d.clone()));
        }

        let mut core = {
            let parser = Parser::with_syllables(PINYIN_SYLLABLES);
            crate::core::Engine::new(model, parser)
        };

        let mut emoji_loaded = false;
        for dir in &aux_dirs {
            let emoji_path = dir.join("emoji.table");
            if emoji_path.exists() {
                if let Ok(emoji_dict) = EmojiDict::from_file(&emoji_path) {
                    log::info!("Loaded emoji dictionary with {} entries", emoji_dict.len());
                    core.set_emoji_dict(emoji_dict);
                    emoji_loaded = true;
                    break;
                }
            }
        }
        if !emoji_loaded {
            log::warn!(
                "emoji.table not found beside {:?}; emoji candidates will be unavailable",
                data_dir
            );
        }
        let mut english_loaded = false;
        for dir in &aux_dirs {
            let english_path = dir.join("english.wordlist");
            if english_path.exists() {
                if let Ok(english_dict) = EnglishDict::from_file(&english_path) {
                    log::info!(
                        "Loaded English dictionary with {} entries from {:?}",
                        english_dict.len(),
                        english_path
                    );
                    core.set_english_dict(english_dict);
                    english_loaded = true;
                    break;
                }
            }
        }
        if !english_loaded {
            log::warn!(
                "english.wordlist not found beside {:?}; English mixed-input will be unavailable",
                data_dir
            );
        }

        let mut engine = Self { inner: core };

        // Load addon dictionaries if available
        let addon_dir = data_dir.join("addon");
        if addon_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&addon_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let fst_path = path.join("lexicon.fst");
                        let dat_path = path.join("lexicon.dat");
                        if fst_path.exists() && dat_path.exists() {
                            if let Ok(lexicon) = Lexicon::load(&fst_path, &dat_path) {
                                let name = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                log::info!("Loaded addon dictionary: {}", name);
                                engine.add_addon(name, lexicon, false);
                            }
                        }
                    }
                }
            }
        }

        Ok(engine)
    }
}

impl Deref for Engine {
    type Target = crate::core::Engine<Parser>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Engine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
