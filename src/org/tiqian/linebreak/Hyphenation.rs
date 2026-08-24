// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/linebreak/Hyphenation.kt

use std::collections::HashMap;

/**
 * 查找单个 Western word 内可插入 soft hyphen 的 offset。offset `k` 表示在 `word[k - 1]` 与
 * `word[k]` 之间断行；行尾在 `word[k - 1]` 之后绘制连字符。offset 是 code point index，按升序
 * 排列，且已经排除左右 margin。
 *
 * 这是混排西文中 CLREQ「可使用连字符处」（§换行与断词连字）：通用规则仅允许在这些位置拆分
 * Western word。真实 hyphenation 是 platform/data capability，参见无数据的 [`NoHyphenator`] 与
 * 携带 TeX patterns 的 `EnglishHyphenation`。每个实例绑定一种语言。
 */
pub trait Hyphenator {
    fn hyphenate(&self, word: &str) -> Vec<i32>;
}

/**
 * 不提供任何 hyphenation opportunity。没有接入真实 hyphenator 时的默认值（core 保持 data-free）：
 * Western word 只会在既有 break character/word boundary 断开，绝不从词中断开。
 */
#[derive(Clone, Copy, Debug, Default)]
pub struct NoHyphenator;

impl Hyphenator for NoHyphenator {
    fn hyphenate(&self, _word: &str) -> Vec<i32> {
        Vec::new()
    }
}

/**
 * Frank Liang 的 hyphenation algorithm，也是 TeX、LibreOffice 与浏览器使用的算法。`patterns` 将
 * pattern key（小写字母，可带 word-boundary marker `.`）映射到 inter-letter level array，其长度为
 * `key` 长度加一。`exceptions` 将 lowercased 完整单词映射到显式 break offset；不含 break 的 entry
 * 禁止对该词断词。当合并后的 level 为奇数且位于 `left_min`/`right_min` margin 外时，允许断行。
 */
#[derive(Clone, Debug)]
pub struct LiangHyphenator {
    patterns: HashMap<String, Vec<i32>>,
    exceptions: HashMap<String, Vec<i32>>,
    left_min: i32,
    right_min: i32,
}

impl LiangHyphenator {
    pub fn new(patterns: HashMap<String, Vec<i32>>) -> Self {
        Self::with_options(patterns, HashMap::new(), 2, 3)
    }

    pub fn with_exceptions(
        patterns: HashMap<String, Vec<i32>>,
        exceptions: HashMap<String, Vec<i32>>,
    ) -> Self {
        Self::with_options(patterns, exceptions, 2, 3)
    }

    pub fn with_margins(
        patterns: HashMap<String, Vec<i32>>,
        left_min: i32,
        right_min: i32,
    ) -> Self {
        Self::with_options(patterns, HashMap::new(), left_min, right_min)
    }

    pub fn with_options(
        patterns: HashMap<String, Vec<i32>>,
        exceptions: HashMap<String, Vec<i32>>,
        left_min: i32,
        right_min: i32,
    ) -> Self {
        Self {
            patterns,
            exceptions,
            left_min,
            right_min,
        }
    }
}

impl Hyphenator for LiangHyphenator {
    fn hyphenate(&self, word: &str) -> Vec<i32> {
        let word_chars: Vec<char> = word.chars().collect();
        let word_length = word_chars.len() as i32;
        if word_length < self.left_min + self.right_min {
            return Vec::new();
        }
        let lower = word.to_lowercase();
        if let Some(explicit) = self.exceptions.get(&lower) {
            return explicit
                .iter()
                .copied()
                .filter(|offset| {
                    *offset >= self.left_min && *offset <= word_length - self.right_min
                })
                .collect();
        }

        // levels[p] 是 work[p] 前 gap 中合并后的 hyphenation level，work = ".<word>."，
        // 两个点均为 pattern boundary marker。
        let work: Vec<char> = format!(".{lower}.").chars().collect();
        let mut levels = vec![0_i32; work.len() + 1];
        for i in 0..work.len() {
            let mut key = String::new();
            for character in work.iter().skip(i) {
                key.push(*character);
                let Some(pattern) = self.patterns.get(&key) else {
                    continue;
                };
                for (k, value) in pattern.iter().enumerate() {
                    if *value > levels[i + k] {
                        levels[i + k] = *value;
                    }
                }
            }
        }

        // word 第 m 个字符之后的断点（offset m + 1）位于 work[m + 2] 前的 gap，
        // 因为 work 含前置 `.`。奇数 level 表示允许断行。
        let mut result = Vec::new();
        for m in 0..word_length - 1 {
            let offset = m + 1;
            if offset < self.left_min || offset > word_length - self.right_min {
                continue;
            }
            if levels[(m + 2) as usize] % 2 == 1 {
                result.push(offset);
            }
        }
        result
    }
}

/**
 * 将 TeX `hyph-*.tex` pattern file 中的 `\patterns{…}` 以及可选 `\hyphenation{…}` exception block
 * 解析为 [`LiangHyphenator`] 可用的 `(patterns, exceptions)`；会剥除以 `%` 开始的 comment。
 */
pub fn parse_tex_hyphenation_patterns(
    tex: &str,
) -> (HashMap<String, Vec<i32>>, HashMap<String, Vec<i32>>) {
    let no_comments = tex
        .lines()
        .map(|line| line.split_once('%').map_or(line, |(before, _)| before))
        .collect::<Vec<_>>()
        .join("\n");
    let block = |macro_name: &str| -> &str {
        let Some(start) = no_comments.find(macro_name) else {
            return "";
        };
        let Some(open_relative) = no_comments[start..].find('{') else {
            return "";
        };
        let open = start + open_relative;
        let Some(close_relative) = no_comments[open + 1..].find('}') else {
            return "";
        };
        &no_comments[open + 1..open + 1 + close_relative]
    };

    let mut patterns = HashMap::new();
    for token in block("\\patterns").split_whitespace() {
        let mut key = String::new();
        let mut levels = vec![0_i32];
        for character in token.chars() {
            if character.is_ascii_digit() {
                let last = levels
                    .last_mut()
                    .expect("hyphenation pattern levels must contain an initial gap");
                *last = character
                    .to_digit(10)
                    .expect("ASCII digit must have a radix-10 value")
                    as i32;
            } else {
                key.push(character);
                levels.push(0);
            }
        }
        patterns.insert(key, levels);
    }

    let mut exceptions = HashMap::new();
    for token in block("\\hyphenation").split_whitespace() {
        let mut offsets = Vec::new();
        let mut position = 0_i32;
        for character in token.chars() {
            if character == '-' {
                offsets.push(position);
            } else {
                position += 1;
            }
        }
        exceptions.insert(token.replace('-', "").to_lowercase(), offsets);
    }
    (patterns, exceptions)
}
