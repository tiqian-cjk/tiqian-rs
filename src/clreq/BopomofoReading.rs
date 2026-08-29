use super::super::core::Text::Text;

// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/clreq/BopomofoReading.kt

/**
 * 注音声调（ADR 0033）。作者只需填写读音字符串，由引擎推导声调；手工标记声调不可行。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BopomofoTone {
    /// 阴平（1 声）：不绘制调号，但保留与其他声调相同的调号空间。
    Yinping,
    /// 阳平（2 声）`ˊ`：属于平上去组，绘制在最后一个符号的右上方。
    Yangping,
    /// 上声（3 声）`ˇ`：属于平上去组。
    Shang,
    /// 去声（4 声）`ˋ`：属于平上去组。
    Qu,
    /// 轻声 `˙`：前置，绘制在符号列顶部。
    Neutral,
    /// 入声（方音）：绘制在右下方。几何已支持，但 v1 解析器不会产生此值。
    Ru,
}

/// 解析后的注音读音：1–3 个 ㄅㄆㄇ 符号与声调（ADR 0033）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BopomofoReading {
    pub symbols: Vec<Text>,
    pub tone: BopomofoTone,
}

/**
 * 将注音字符串解析为符号与声调：
 * - 前置 `˙`（U+02D9）表示轻声，其余内容为符号；
 * - 后置 `ˊ`、`ˇ`、`ˋ` 表示阳平、上声、去声并从符号中移除；后置 `ˉ`（U+02C9）
 *   表示显式阴平，同样移除；
 * - 其他情况均为阴平，不绘制调号。
 *
 * 剩余每个字符（U+3105–U+312F Bopomofo）各自构成一个符号，读音最多携带 3 个符号。
 * v1 不解析入声（方音），因为普通话注音没有入声。
 */
pub mod bopomofo_parser {
    use super::*;

    pub fn parse(reading: &Text) -> BopomofoReading {
        if reading.is_empty() {
            return BopomofoReading {
                symbols: Vec::new(),
                tone: BopomofoTone::Yinping,
            };
        }

        if reading.starts_with(NEUTRAL) {
            return BopomofoReading {
                symbols: symbols_of(&reading.as_str()[NEUTRAL.len_utf8()..]),
                tone: BopomofoTone::Neutral,
            };
        }

        let last = reading
            .chars()
            .next_back()
            .expect("非空注音读音必须包含最后一个字符");
        let tone = match last {
            YANGPING => BopomofoTone::Yangping,
            SHANG => BopomofoTone::Shang,
            QU => BopomofoTone::Qu,
            _ => BopomofoTone::Yinping,
        };
        let has_suffix_mark = matches!(last, YANGPING | SHANG | QU | YINPING_MACRON);
        let body = if has_suffix_mark {
            &reading.as_str()[..reading.len() - last.len_utf8()]
        } else {
            reading.as_str()
        };
        BopomofoReading {
            symbols: symbols_of(body),
            tone,
        }
    }

    fn symbols_of(body: &str) -> Vec<Text> {
        body.chars()
            .map(|character| Text::from(character.to_string()))
            .collect()
    }

    const NEUTRAL: char = '˙';
    const YANGPING: char = 'ˊ';
    const SHANG: char = 'ˇ';
    const QU: char = 'ˋ';
    const YINPING_MACRON: char = 'ˉ';
}
