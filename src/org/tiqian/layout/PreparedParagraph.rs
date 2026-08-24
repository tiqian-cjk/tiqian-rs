// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/PreparedParagraph.kt

use std::collections::{HashMap, HashSet};

use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::LayoutResult;
use super::super::core::LayoutQueries::positioned_clusters;

/**
 * 供 build-time snapshot 与 browser exact-font fallback 共用的规范纯段落 render plan。
 * lowering 与 [`LayoutResult`] 同处，避免两个 Web 入口各自生成不一致的 DOM geometry。
 */
pub fn to_prepared_paragraph_json(result: &LayoutResult) -> String {
    let mut natural_width: HashMap<TextRange, f32> = HashMap::new();
    let mut open_type_features: HashMap<TextRange, Vec<String>> = HashMap::new();
    for run in &result.glyph_runs {
        for glyph in &run.glyphs {
            *natural_width.entry(glyph.cluster_range).or_insert(0.0) += glyph.advance;
            if !run.open_type_features.is_empty() {
                let features = open_type_features.entry(glyph.cluster_range).or_default();
                for feature in &run.open_type_features {
                    if !features.contains(feature) {
                        features.push(feature.clone());
                    }
                }
            }
        }
    }
    let zero_width_breaks: HashSet<TextRange> = result
        .debug
        .zero_width_break_decisions
        .iter()
        .map(|decision| decision.range)
        .collect();
    let positioned = positioned_clusters(result);
    let mut out = String::from("{\"schema\":1,\"layoutRevision\":\"tiqian-layout-v2\",\"width\":");
    append_json_number(&mut out, result.input.constraints.max_width());
    out.push_str(",\"height\":");
    append_json_number(&mut out, result.size.height);
    out.push_str(",\"lines\":[");
    for (line_index, line) in result.lines.iter().enumerate() {
        if line_index > 0 {
            out.push(',');
        }
        let cells: Vec<_> = positioned
            .iter()
            .filter(|position| {
                position.line_index == line_index as i32 && {
                    let cluster = &result.clusters[position.cluster_index as usize];
                    !cluster.display_text.is_empty() || zero_width_breaks.contains(&cluster.range)
                }
            })
            .collect();
        out.push_str("{\"rangeStart\":");
        out.push_str(&line.range.start().to_string());
        out.push_str(",\"rangeEnd\":");
        out.push_str(&line.range.end().to_string());
        out.push_str(",\"top\":");
        append_json_number(&mut out, line.top);
        out.push_str(",\"bottom\":");
        append_json_number(&mut out, line.bottom);
        out.push_str(",\"baseline\":");
        append_json_number(&mut out, line.baseline);
        out.push_str(",\"indent\":");
        append_json_number(&mut out, line.indent);
        out.push_str(",\"visualWidth\":");
        append_json_number(&mut out, line.visual_width);
        out.push_str(",\"hyphenAdvance\":");
        append_json_number(&mut out, line.hyphen_advance);
        out.push_str(",\"endReason\":");
        append_json_string(&mut out, &format!("{:?}", line.end_reason));
        out.push_str(",\"cells\":[");
        for (cell_index, position) in cells.iter().enumerate() {
            if cell_index > 0 {
                out.push(',');
            }
            let cluster = &result.clusters[position.cluster_index as usize];
            out.push_str("{\"rangeStart\":");
            out.push_str(&cluster.range.start().to_string());
            out.push_str(",\"rangeEnd\":");
            out.push_str(&cluster.range.end().to_string());
            out.push_str(",\"source\":");
            append_json_string(&mut out, &cluster.text);
            out.push_str(",\"display\":");
            append_json_string(&mut out, &cluster.display_text);
            out.push_str(",\"drawX\":");
            append_json_number(&mut out, position.draw_x);
            out.push_str(",\"naturalWidth\":");
            append_json_number(
                &mut out,
                *natural_width
                    .get(&cluster.range)
                    .unwrap_or(&cluster.advance),
            );
            out.push_str(",\"leadingLayoutAdvance\":");
            append_json_number(&mut out, cluster.leading_layout_advance);
            // MultiCodeUnitShapingBoundary：Latin word、URL、emoji 等多 unit cluster 已在 core 独立
            // shaping；DOM 不得合并相邻 cluster 后重新 shaping 成不同、更宽的 run。
            if cluster.range.end() - cluster.range.start() > 1 {
                out.push_str(",\"shapingBoundary\":true");
            }
            if let Some(features) = open_type_features
                .get(&cluster.range)
                .filter(|features| !features.is_empty())
            {
                out.push_str(",\"openTypeFeatures\":[");
                for (feature_index, feature) in features.iter().enumerate() {
                    if feature_index > 0 {
                        out.push(',');
                    }
                    append_json_string(&mut out, feature);
                }
                out.push(']');
            }
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// Kotlin/JS、JVM 与 Native 的 Float.toString 不同；此处将 Float 归一到 ECMAScript Number::toString 计划格式。
fn append_json_number(out: &mut String, value: f32) {
    if value == 0.0 {
        out.push('0');
    } else {
        out.push_str(&ecma_json_number(value));
    }
}

fn ecma_json_number(value: f32) -> String {
    let raw = value as f64;
    let negative = raw.is_sign_negative();
    let mut body = raw.abs().to_string();
    let exponent_at = body.find(['e', 'E']);
    let exponent = exponent_at.map_or(0, |at| {
        body[at + 1..]
            .parse::<i32>()
            .expect("Rust float exponent is decimal")
    });
    if let Some(at) = exponent_at {
        body.truncate(at);
    }
    let dot_at = body.find('.');
    let integer_part = dot_at.map_or(body.as_str(), |at| &body[..at]);
    let fraction_part = dot_at.map_or("", |at| &body[at + 1..]);
    let mut digits = if integer_part.chars().any(|character| character != '0') {
        format!("{integer_part}{fraction_part}")
    } else {
        fraction_part.to_owned()
    };
    let mut decimal_exponent = if integer_part.chars().any(|character| character != '0') {
        integer_part.len() as i32
    } else {
        0
    } + exponent;
    let first = digits.find(|character: char| character != '0');
    let Some(first) = first else {
        return "0".to_owned();
    };
    if first > 0 {
        digits = digits[first..].to_owned();
        decimal_exponent -= first as i32;
    }
    let last = digits
        .rfind(|character: char| character != '0')
        .expect("nonzero digit exists");
    if last + 1 < digits.len() {
        digits.truncate(last + 1);
    }
    let digits = canonical_tie_break(&digits, value);
    let magnitude = if negative { "-" } else { "" };
    let k = digits.len() as i32;
    let n = decimal_exponent;
    if k <= n && n <= 21 {
        format!("{magnitude}{digits}{}", "0".repeat((n - k) as usize))
    } else if n > 0 && n <= 21 {
        format!(
            "{magnitude}{}.{}",
            &digits[..n as usize],
            &digits[n as usize..]
        )
    } else if n > -6 && n <= 0 {
        format!("{magnitude}0.{}{}", "0".repeat((-n) as usize), digits)
    } else {
        let mantissa = if k > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        let exponent_value = n - 1;
        format!(
            "{magnitude}{mantissa}e{}{}",
            if exponent_value < 0 { "-" } else { "+" },
            exponent_value.abs()
        )
    }
}

/// dtoa 在末位精确半值时可能采用不同取整；Float 的 exact decimal expansion 有限，按 half-to-even 归一。
fn canonical_tie_break(digits: &str, value: f32) -> String {
    let bits = value.to_bits() & 0x7fff_ffff;
    let biased_exponent = (bits >> 23) & 0xff;
    let mut mantissa = bits & 0x7f_ffff;
    if mantissa == 0 && biased_exponent == 0 {
        return digits.to_owned();
    }
    
    let exponent: i32 = if biased_exponent == 0 {
        -149
    } else {
        mantissa |= 0x80_0000;
        biased_exponent as i32 - 150
    };
    let mut exact = mantissa.to_string();
    for _ in 0..exponent.max(0) {
        exact = times_small(&exact, 2);
    }
    for _ in 0..(-exponent).max(0) {
        exact = times_small(&exact, 5);
    }
    let stripped = exact.trim_end_matches('0');
    if stripped.len() <= digits.len() {
        return digits.to_owned();
    }
    let keep = &stripped[..digits.len()];
    let remainder = &stripped[digits.len()..];
    let past_half = remainder.len() > 1 && remainder[1..].chars().any(|character| character != '0');
    let round_up = match remainder.as_bytes()[0] {
        b'6'..=b'9' => true,
        b'0'..=b'4' => false,
        _ => past_half || !(keep.as_bytes()[keep.len() - 1] - b'0').is_multiple_of(2),
    };
    let canonical = if round_up {
        increment_decimal(keep)
    } else {
        keep.to_owned()
    };
    if canonical.trim_end_matches('0').len() == digits.len() {
        canonical
    } else {
        digits.to_owned()
    }
}

fn times_small(digits: &str, factor: u32) -> String {
    let mut out = Vec::new();
    let mut carry = 0_u32;
    for character in digits.bytes().rev() {
        let product = (character - b'0') as u32 * factor + carry;
        out.push((b'0' + (product % 10) as u8) as char);
        carry = product / 10;
    }
    while carry > 0 {
        out.push((b'0' + (carry % 10) as u8) as char);
        carry /= 10;
    }
    out.into_iter().rev().collect()
}

fn increment_decimal(digits: &str) -> String {
    let mut chars = digits.as_bytes().to_vec();
    for index in (0..chars.len()).rev() {
        if chars[index] < b'9' {
            chars[index] += 1;
            return String::from_utf8(chars).expect("decimal digits are UTF-8");
        }
        chars[index] = b'0';
    }
    format!(
        "1{}",
        String::from_utf8(chars).expect("decimal digits are UTF-8")
    )
}

fn append_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character <= '\u{1f}' => {
                out.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => out.push(character),
        }
    }
    out.push('"');
}
