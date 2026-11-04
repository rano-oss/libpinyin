//! Mandarin tone lookup for Mac-style Tab tone filter.

use std::collections::HashMap;

/// Maps Han characters to primary Mandarin tone (1–4). Neutral/unknown omitted.
#[derive(Debug, Clone)]
pub struct ToneMap {
    tones: HashMap<char, u8>,
}

impl Default for ToneMap {
    fn default() -> Self {
        Self {
            tones: builtin_tones(),
        }
    }
}

impl ToneMap {
    pub fn tone_of(&self, ch: char) -> Option<u8> {
        self.tones.get(&ch).copied()
    }

    /// Tone of the first Han character in `text`, if known.
    pub fn first_han_tone(&self, text: &str) -> Option<u8> {
        text.chars()
            .find(|c| {
                let u = *c as u32;
                (0x4E00..=0x9FFF).contains(&u) || (0x3400..=0x4DBF).contains(&u)
            })
            .and_then(|c| self.tone_of(c))
    }
}

fn builtin_tones() -> HashMap<char, u8> {
    // Compact seed covering common singles used in filtering demos / tests.
    // Unknown chars are kept when filtering (see ImeEngine).
    let pairs: &[(char, u8)] = &[
        // ma
        ('妈', 1),
        ('麻', 2),
        ('马', 3),
        ('骂', 4),
        ('吗', 1),
        // yi
        ('一', 1),
        ('移', 2),
        ('以', 3),
        ('意', 4),
        ('亿', 4),
        // shi
        ('师', 1),
        ('时', 2),
        ('使', 3),
        ('是', 4),
        ('事', 4),
        ('市', 4),
        // bei / jing
        ('杯', 1),
        ('北', 3),
        ('被', 4),
        ('备', 4),
        ('京', 1),
        ('经', 1),
        ('精', 1),
        ('景', 3),
        ('静', 4),
        // ni / hao
        ('妮', 1),
        ('泥', 2),
        ('你', 3),
        ('逆', 4),
        ('蒿', 1),
        ('毫', 2),
        ('好', 3),
        ('号', 4),
        // zhong / guo
        ('中', 1),
        ('忠', 1),
        ('种', 3),
        ('重', 4),
        ('众', 4),
        ('锅', 1),
        ('国', 2),
        ('果', 3),
        ('过', 4),
        // wo / men
        ('窝', 1),
        ('我', 3),
        ('卧', 4),
        ('闷', 1),
        ('门', 2),
        ('们', 2),
        ('焖', 4),
        // ai / ren
        ('埃', 1),
        ('癌', 2),
        ('矮', 3),
        ('爱', 4),
        ('人', 2),
        ('忍', 3),
        ('认', 4),
        // common particles
        ('的', 1),
        ('了', 1),
        ('在', 4),
        ('有', 3),
        ('和', 2),
        ('不', 4),
        ('这', 4),
        ('那', 4),
        ('个', 4),
        ('上', 4),
        ('大', 4),
        ('来', 2),
        ('说', 1),
        ('出', 1),
        ('会', 4),
        ('可', 3),
        ('她', 1),
        ('他', 1),
        ('它', 1),
        ('天', 1),
        ('年', 2),
        ('日', 4),
        ('月', 4),
        ('东', 1),
        ('南', 2),
        ('西', 1),
        ('方', 1),
        ('学', 2),
        ('生', 1),
        ('工', 1),
        ('作', 4),
        ('成', 2),
        ('功', 1),
        ('应', 1),
        ('该', 1),
        ('朋', 2),
        ('友', 3),
        ('谢', 4),
        ('上', 4),
        ('海', 3),
        ('伤', 1),
        ('方', 1),
        ('房', 2),
        ('访', 3),
        ('放', 4),
        ('行', 2),
        ('很', 3),
        ('高', 1),
    ];
    pairs.iter().copied().collect()
}
