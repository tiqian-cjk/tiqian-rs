// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/PreparedParagraph.kt

use crate::common::{HashMap, HashSet};

use super::super::core::geometry::TextRange;
use super::super::core::layout_model::LayoutResult;
use super::super::core::layout_queries::positioned_clusters;
use super::super::core::text_model::{DecorationKind, TextStyle};

/**
 * 供 build-time snapshot 与 browser exact-font fallback 共用的规范纯段落 render plan。
 * lowering 与 [`LayoutResult`] 同处，避免两个 Web 入口各自生成不一致的 DOM geometry。
 */
/// Emits the prepared paragraph plan with Kotlin's optional render evidence.
pub fn to_prepared_paragraph_json(
    result: &LayoutResult,
    render_evidence: bool,
) -> String {
    let mut natural_width: HashMap<TextRange, f32> = HashMap::new();
    let mut open_type_features: HashMap<TextRange, Vec<String>> = HashMap::new();
    let mut render_font_family: HashMap<TextRange, String> = HashMap::new();
    let mut glyph_ids_by_range: HashMap<TextRange, Vec<u32>> = HashMap::new();
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
            if let Some(render_font_key) = &glyph.render_font_key {
                render_font_family.insert(glyph.cluster_range, render_font_key.clone());
            }
            glyph_ids_by_range
                .entry(glyph.cluster_range)
                .or_default()
                .push(glyph.id);
        }
    }
    let zero_width_breaks: HashSet<TextRange> = result
        .debug
        .zero_width_break_decisions
        .iter()
        .map(|decision| decision.range)
        .collect();
    let shaping_decision_by_range: HashMap<_, _> = result
        .debug
        .shaping_decisions
        .iter()
        .map(|decision| (decision.range, decision))
        .collect();
    let punctuation_decision_by_range: HashMap<_, _> = result
        .debug
        .punctuation_decisions
        .iter()
        .map(|decision| (decision.range, decision))
        .collect();
    let mut inline_start_by_offset: HashMap<i32, f32> = HashMap::new();
    let mut inline_end_by_offset: HashMap<i32, f32> = HashMap::new();
    for inline_box in &result.input.inline_boxes {
        if inline_box.inline_start != 0.0 {
            *inline_start_by_offset
                .entry(inline_box.range.start())
                .or_insert(0.0) += inline_box.inline_start;
        }
        if inline_box.inline_end != 0.0 {
            *inline_end_by_offset
                .entry(inline_box.range.end())
                .or_insert(0.0) += inline_box.inline_end;
        }
    }
    let inline_object_advance_by_range: HashMap<_, _> = result
        .input
        .inline_objects
        .iter()
        .map(|inline_object| (inline_object.range, inline_object.advance))
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
                    !cluster.display_text.is_empty()
                        || zero_width_breaks.contains(&cluster.range)
                        || (render_evidence
                            && inline_object_advance_by_range.contains_key(&cluster.range))
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
            if render_evidence {
                append_cell_render_evidence(
                    &mut out,
                    result,
                    cluster,
                    natural_width.get(&cluster.range).copied(),
                    render_font_family.get(&cluster.range),
                    glyph_ids_by_range.get(&cluster.range),
                    shaping_decision_by_range.get(&cluster.range).copied(),
                    punctuation_decision_by_range.get(&cluster.range).copied(),
                    inline_object_advance_by_range.get(&cluster.range).copied(),
                );
            }
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push(']');
    if render_evidence {
        append_paragraph_render_evidence(
            &mut out,
            result,
            &inline_start_by_offset,
            &inline_end_by_offset,
        );
    }
    out.push('}');
    out
}

#[allow(clippy::too_many_arguments)]
fn append_cell_render_evidence(
    out: &mut String,
    result: &LayoutResult,
    cluster: &super::super::core::layout_model::Cluster,
    natural_width: Option<f32>,
    render_font_family: Option<&String>,
    glyph_ids: Option<&Vec<u32>>,
    shaping_decision: Option<&super::super::core::layout_model::ShapingDecisionInfo>,
    punctuation_decision: Option<&super::super::core::layout_model::PunctuationDecisionInfo>,
    inline_object_advance: Option<f32>,
) {
    if let Some(inline_object_advance) = inline_object_advance {
        out.push_str(",\"inlineObject\":");
        append_json_number(out, inline_object_advance);
    }
    let glyph_width = inline_object_advance.or(natural_width).unwrap_or(cluster.advance);
    if cluster.advance != glyph_width {
        out.push_str(",\"advance\":");
        append_json_number(out, cluster.advance);
    }
    if let Some(render_font_family) = render_font_family {
        out.push_str(",\"renderFontFamily\":");
        append_json_string(out, render_font_family);
    }
    if let Some(strategy) = shaping_decision.and_then(|decision| decision.strategy.as_ref()) {
        out.push_str(",\"dashStrategy\":");
        append_json_string(out, strategy);
        if let Some(language) = shaping_decision.and_then(|decision| decision.language.as_ref()) {
            out.push_str(",\"shapingLanguage\":");
            append_json_string(out, language);
        }
        if let Some(resolved_face) = shaping_decision.and_then(|decision| decision.resolved_face.as_ref()) {
            out.push_str(",\"resolvedFace\":");
            append_json_string(out, resolved_face);
        }
        if let Some(glyph_ids) = glyph_ids.filter(|glyph_ids| !glyph_ids.is_empty()) {
            out.push_str(",\"glyphIds\":");
            append_json_string(out, &glyph_ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","));
        }
        out.push_str(",\"shapingEvidence\":");
        append_json_string(out, &shaping_decision.expect("strategy has a shaping decision").reason);
    }
    if let Some(punctuation_decision) = punctuation_decision
        && punctuation_decision.ink_containment_applied
        && let Some(floor) = punctuation_decision.ink_containment_body_floor
    {
        out.push_str(",\"punctuationInkFloor\":");
        append_json_number(out, floor);
        out.push_str(",\"punctuationBodyWidth\":");
        append_json_number(out, punctuation_decision.body_width);
    }
    let latin = result.debug.font_decisions.iter().any(|decision| {
        cluster.range.start() >= decision.range.start()
            && cluster.range.end() <= decision.range.end()
            && decision.role == "LatinText"
    });
    if latin {
        out.push_str(",\"latin\":true");
    }
    let cluster_style = style_at(result, cluster.range.start());
    if cluster_style != &result.input.text_style {
        out.push_str(",\"style\":{");
        let mut field_count = 0;
        if cluster_style.font_size != result.input.text_style.font_size {
            out.push_str("\"fontSize\":");
            append_json_number(out, cluster_style.font_size);
            field_count += 1;
        }
        if cluster_style.font_weight != result.input.text_style.font_weight {
            if field_count > 0 {
                out.push(',');
            }
            out.push_str("\"fontWeight\":");
            out.push_str(&cluster_style.font_weight.to_string());
            field_count += 1;
        }
        if cluster_style.italic != result.input.text_style.italic {
            if field_count > 0 {
                out.push(',');
            }
            out.push_str("\"italic\":");
            out.push_str(&cluster_style.italic.to_string());
        }
        out.push('}');
    }
}

fn style_at(result: &LayoutResult, offset: i32) -> &TextStyle {
    result
        .input
        .content
        .spans
        .iter()
        .rev()
        .find(|span| offset >= span.range.start() && offset < span.range.end())
        .map_or(&result.input.text_style, |span| &span.style)
}

fn append_paragraph_render_evidence(
    out: &mut String,
    result: &LayoutResult,
    inline_start_by_offset: &HashMap<i32, f32>,
    inline_end_by_offset: &HashMap<i32, f32>,
) {
    out.push_str(",\"fontSize\":");
    append_json_number(out, result.input.text_style.font_size);
    out.push_str(",\"overlayWidth\":");
    append_json_number(out, result.size.width);
    let emphasis_ranges: Vec<_> = result
        .input
        .decorations
        .iter()
        .filter(|span| span.kind == DecorationKind::Emphasis)
        .collect();
    if !emphasis_ranges.is_empty() {
        out.push_str(",\"emphasisRanges\":[");
        for (index, span) in emphasis_ranges.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push('[');
            out.push_str(&span.range.start().to_string());
            out.push(',');
            out.push_str(&span.range.end().to_string());
            out.push(']');
        }
        out.push(']');
    }
    if !inline_start_by_offset.is_empty() || !inline_end_by_offset.is_empty() {
        let mut offsets: Vec<_> = inline_start_by_offset
            .keys()
            .chain(inline_end_by_offset.keys())
            .copied()
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        out.push_str(",\"inlineEdges\":[");
        for (index, offset) in offsets.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"offset\":");
            out.push_str(&offset.to_string());
            if let Some(inline_start) = inline_start_by_offset.get(offset) {
                out.push_str(",\"inlineStart\":");
                append_json_number(out, *inline_start);
            }
            if let Some(inline_end) = inline_end_by_offset.get(offset) {
                out.push_str(",\"inlineEnd\":");
                append_json_number(out, *inline_end);
            }
            out.push('}');
        }
        out.push(']');
    }
    if !result.debug.ruby_decisions.is_empty() {
        out.push_str(",\"rubyDecisions\":[");
        for (index, ruby) in result.debug.ruby_decisions.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"baseRangeStart\":");
            out.push_str(&ruby.base_range.start().to_string());
            out.push_str(",\"baseRangeEnd\":");
            out.push_str(&ruby.base_range.end().to_string());
            out.push_str(",\"text\":");
            append_json_string(out, &ruby.text);
            out.push_str(",\"centerX\":");
            append_json_number(out, ruby.center_x);
            out.push_str(",\"baselineY\":");
            append_json_number(out, ruby.baseline_y);
            out.push_str(",\"fontSize\":");
            append_json_number(out, ruby.font_size);
            out.push_str(",\"ascent\":");
            append_json_number(out, ruby.ascent);
            out.push_str(",\"fontWeight\":");
            out.push_str(&ruby.font_weight.to_string());
            append_json_string_array(out, "fontFamilies", &ruby.font_families);
            out.push('}');
        }
        out.push(']');
    }
    if !result.debug.bopomofo_decisions.is_empty() {
        out.push_str(",\"bopomofoDecisions\":[");
        for (index, bopomofo) in result.debug.bopomofo_decisions.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"baseRangeStart\":");
            out.push_str(&bopomofo.base_range.start().to_string());
            out.push_str(",\"baseRangeEnd\":");
            out.push_str(&bopomofo.base_range.end().to_string());
            out.push_str(",\"text\":");
            append_json_string(out, &bopomofo.text);
            out.push_str(",\"fontWeight\":");
            out.push_str(&bopomofo.font_weight.to_string());
            append_json_string_array(out, "fontFamilies", &bopomofo.font_families);
            out.push_str(",\"placements\":[");
            for (placement_index, placement) in bopomofo.placements.iter().enumerate() {
                if placement_index > 0 {
                    out.push(',');
                }
                out.push_str("{\"text\":");
                append_json_string(out, &placement.text);
                out.push_str(",\"left\":");
                append_json_number(out, placement.left);
                out.push_str(",\"top\":");
                append_json_number(out, placement.top);
                out.push_str(",\"width\":");
                append_json_number(out, placement.width);
                out.push_str(",\"height\":");
                append_json_number(out, placement.height);
                out.push_str(",\"role\":");
                append_json_string(out, &format!("{:?}", placement.role));
                out.push('}');
            }
            out.push_str("]}");
        }
        out.push(']');
    }
    let decoration_segments: Vec<_> = result
        .debug
        .decoration_segments
        .iter()
        .filter(|segment| matches!(segment.kind.as_str(), "ProperNoun" | "BookTitle"))
        .collect();
    if !decoration_segments.is_empty() {
        out.push_str(",\"decorationSegments\":[");
        for (index, segment) in decoration_segments.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"kind\":");
            append_json_string(out, &segment.kind);
            out.push_str(",\"left\":");
            append_json_number(out, segment.left);
            out.push_str(",\"top\":");
            append_json_number(out, segment.top);
            out.push_str(",\"right\":");
            append_json_number(out, segment.right);
            out.push_str(",\"sourceRangeStart\":");
            out.push_str(&segment.source_range.start().to_string());
            out.push_str(",\"sourceRangeEnd\":");
            out.push_str(&segment.source_range.end().to_string());
            out.push('}');
        }
        out.push(']');
    }
    let emphasis_dots: Vec<_> = result
        .debug
        .decoration_decisions
        .iter()
        .filter(|decision| {
            decision.applied && decision.kind == "Emphasis" && decision.dot_diameter > 0.0
        })
        .collect();
    if !emphasis_dots.is_empty() {
        out.push_str(",\"emphasisDots\":[");
        for (index, dot) in emphasis_dots.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str("{\"clusterRangeStart\":");
            out.push_str(&dot.cluster_range.start().to_string());
            out.push_str(",\"anchorX\":");
            append_json_number(out, dot.anchor_x);
            out.push_str(",\"anchorY\":");
            append_json_number(out, dot.anchor_y);
            out.push_str(",\"dotDiameter\":");
            append_json_number(out, dot.dot_diameter);
            out.push('}');
        }
        out.push(']');
    }
}

fn append_json_string_array(out: &mut String, field: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(",\"");
    out.push_str(field);
    out.push_str("\":[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        append_json_string(out, value);
    }
    out.push(']');
}

/// Kotlin/JS、JVM 与 Native 的 Float.toString 不同；此处将 Float 归一到 ECMAScript Number::toString 计划格式。
fn append_json_number(out: &mut String, value: f32) {
    if value == 0.0 {
        out.push('0');
    } else {
        out.push_str(&ecma_json_number(value));
    }
}

pub fn ecma_json_number(value: f32) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        };
    }
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
