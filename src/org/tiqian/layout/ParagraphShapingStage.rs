// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ParagraphShapingStage.kt

use super::super::clreq::ClreqProfile::{
    ClreqPunctuationGlyphSubstitutor, clreq_punctuation_policies,
};
use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::{
    BreakOpportunityDecisionInfo, Cluster, EmergencyTrackingEligibilityDecisionInfo, Glyph,
    ShapingDecisionInfo,
};
use super::super::core::SourceInteractionBoundaries::source_grapheme_boundaries;
use super::super::core::Text::Text;
use super::super::core::TextModel::{InlineObjectSpan, LayoutInput, LineBreakPolicy, TextStyle};
use super::super::font::FontPolicy::{FontDecision, FontRole};
use super::super::linebreak::Hyphenation::Hyphenator;
use super::super::shaping::TextShaper::{
    ShapingInput, ShapingResult, TextShaper, UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE,
};
use super::ClusterRoleResolution::ResolvedClusterRange;
use super::ProgressiveBreakDecisions::{ProgressiveBreakOpportunity, ProgressiveBreakTier};
use icu_properties::{CodePointMapData, props::GeneralCategory};
use crate::common::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphShapingStageResult {
    pub shaping_results: Vec<ShapingResult>,
    pub hyphen_offsets: HashSet<i32>,
    pub hyphen_advance: f32,
    pub hyphen_glyphs: Vec<Glyph>,
    pub substitution_rollbacks: HashMap<TextRange, String>,
    pub break_opportunity_decisions: Vec<BreakOpportunityDecisionInfo>,
    pub emergency_tracking_eligibility_decisions: Vec<EmergencyTrackingEligibilityDecisionInfo>,
    pub progressive_break_offsets: HashMap<i32, ProgressiveBreakOpportunity>,
    pub segment_shaping_cache: HashMap<TextRange, ShapingResult>,
}

/// 宽度相关的 shaping stage：先解析 display substitution 与西文 token 断点，再返回保持 source range 的 cluster/glyph evidence。
#[allow(clippy::too_many_arguments)]
pub fn shape_paragraph(
    text_shaper: &dyn TextShaper,
    hyphenator: &dyn Hyphenator,
    input: &LayoutInput,
    text: &Text,
    font_size: f32,
    measure: f32,
    cluster_ranges: &[ResolvedClusterRange],
    font_decision_by_range: &HashMap<TextRange, FontDecision>,
    inline_object_by_range: &HashMap<TextRange, InlineObjectSpan>,
    punctuation_glyph_substitutor: &ClreqPunctuationGlyphSubstitutor,
    style_at: &dyn Fn(i32) -> TextStyle,
    emphasis_italic_at: &dyn Fn(i32) -> bool,
    rejected_technical_tiers_by_span: &HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    cached_segment_shaping: &HashMap<TextRange, ShapingResult>,
    cached_substitution_rollbacks: &HashMap<TextRange, String>,
) -> ParagraphShapingStageResult {
    let mut segment_cache = cached_segment_shaping.clone();
    let mut rollbacks = cached_substitution_rollbacks.clone();
    let mut shape_segment = |decision: &FontDecision, range: TextRange| -> ShapingResult {
        if let Some(cached) = segment_cache.get(&range) {
            return cached.clone();
        }
        let source = text.slice_text(range);
        let substitution = punctuation_glyph_substitutor.substitute(&source);
        let base = style_at(range.start());
        let mut style = base.clone();
        if decision.role == FontRole::LatinText && emphasis_italic_at(range.start()) {
            style.italic = true;
        }
        let shaped = text_shaper.shape(
            &ShapingInput::builder(text.clone(), range, style.clone(), decision.clone())
                .display_text(substitution.display_text.clone())
                .open_type_features(cjk_punctuation_full_width_features(
                    decision.role,
                    &substitution.display_text,
                ))
                .build(),
        );
        let rollback = if substitution.display_text == source {
            None
        } else if shaped.decisions.iter().any(|it| {
            it.capability_issue.as_deref() == Some(UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE)
        }) {
            Some("SubstitutionRollbackOnUnverifiedGlyphCoverage")
        } else if shaped.decisions.iter().any(|it| it.missing_glyphs > 0) {
            Some("SubstitutionRollbackOnMissingGlyph")
        } else if dash_ink_coverage_deficient(&shaped, &substitution.display_text, style.font_size)
        {
            Some("DashSubstitutionInkCoverageRollback")
        } else {
            None
        };
        let result = if let Some(cause) = rollback {
            rollbacks.insert(range, cause.to_owned());
            text_shaper.shape(
                &ShapingInput::builder(text.clone(), range, style, decision.clone())
                    .display_text(source)
                    .open_type_features(cjk_punctuation_full_width_features(
                        decision.role,
                        &text.slice_text(range),
                    ))
                    .build(),
            )
        } else {
            shaped
        };
        segment_cache.insert(range, result.clone());
        result
    };
    let mut hyphen_offsets = HashSet::new();
    let mut hyphen_advance = None;
    let mut hyphen_glyphs = Vec::new();
    let mut decisions = Vec::new();
    let mut emergency = Vec::new();
    let mut progressive: HashMap<i32, ProgressiveBreakOpportunity> = HashMap::new();
    let mut progressive_span_advance_cache: HashMap<TextRange, f32> = HashMap::new();
    let mut cuts_by_segment: HashMap<TextRange, Vec<i32>> = HashMap::new();
    for resolved in cluster_ranges {
        if let Some(object) = inline_object_by_range.get(&resolved.range) {
            decisions.shrink_to_fit();
            let _ = object;
            continue;
        }
        if resolved.mandatory_break || resolved.zero_width_soft_break {
            continue;
        }
        let decision = font_decision_by_range
            .get(&resolved.range)
            .expect("shapeable range must have font decision");
        for segment in shaping_segments(decision, text) {
            let shaped = shape_segment(decision, segment);
            let word = text.slice_text(segment);
            let latin = decision.role == FontRole::LatinText && !segment.is_empty();
            let progressive_span = input.content.line_break_spans.iter().find(|span| {
                span.policy == LineBreakPolicy::ProgressiveTechnical
                    && segment.start() >= span.range.start()
                    && segment.end() <= span.range.end()
            });
            let word_length = word.utf16_len();
            let all_letters = latin && word.chars().all(char::is_alphabetic);
            let all_caps =
                all_letters && word_length >= 2 && word.chars().all(|c| !c.is_lowercase());
            let abbreviation = all_caps && word_length < LATIN_OPAQUE_TOKEN_MIN_LENGTH;
            let camel = all_letters
                && !all_caps
                && !abbreviation
                && word.chars().skip(1).any(char::is_uppercase);
            let token_advance: f32 = shaped.clusters.iter().map(|c| c.advance).sum();
            let strong = if latin {
                strong_non_lexical_reason(&word)
            } else {
                None
            };
            let mut syllable = if all_letters
                && !abbreviation
                && !camel
                && !word.contains('-')
                && strong.is_none()
            {
                hyphenator.hyphenate(&word)
            } else {
                Vec::new()
            };
            syllable.sort();
            syllable.dedup();
            let longest = syllable_bounds(&syllable, word.utf16_len())
                .windows(2)
                .map(|bounds| bounds[1] - bounds[0])
                .max()
                .unwrap_or(0);
            let long_letters =
                all_letters && !abbreviation && !camel && longest >= LATIN_OPAQUE_TOKEN_MIN_LENGTH;
            let long_opaque = strong.is_some()
                || long_letters
                || (latin && !all_letters && word.utf16_len() >= LATIN_OPAQUE_TOKEN_MIN_LENGTH);
            let structural = if progressive_span.is_some() && latin {
                progressive_structural_cuts(text, segment)
            } else {
                Vec::new()
            };
            let technical_syllable = if progressive_span.is_some() && latin {
                technical_syllable_cuts(text, segment, &structural, hyphenator)
            } else {
                Vec::new()
            };
            let technical_emergency = if let Some(span) = progressive_span.filter(|_| latin) {
                let rejected = rejected_technical_tiers_by_span
                    .get(&span.range)
                    .cloned()
                    .unwrap_or_default();
                let span_advance = if let Some(cached) =
                    progressive_span_advance_cache.get(&span.range)
                {
                    *cached
                } else {
                    let mut total = 0.;
                    for range in cluster_ranges {
                        if range.mandatory_break
                            || range.zero_width_soft_break
                            || inline_object_by_range.contains_key(&range.range)
                        {
                            continue;
                        }
                        let candidate_decision = font_decision_by_range
                            .get(&range.range)
                            .expect("shapeable range must have font decision");
                        for candidate in shaping_segments(candidate_decision, text) {
                            let start = candidate.start().max(span.range.start());
                            let end = candidate.end().min(span.range.end());
                            if start < end {
                                total +=
                                    shape_segment(candidate_decision, TextRange::new(start, end))
                                        .clusters
                                        .iter()
                                        .map(|cluster| cluster.advance)
                                        .sum::<f32>();
                            }
                        }
                    }
                    progressive_span_advance_cache.insert(span.range, total);
                    total
                };
                let mut cuts = Vec::new();
                let mut bounds = vec![segment.start()];
                bounds.extend_from_slice(&structural);
                bounds.extend_from_slice(&technical_syllable);
                bounds.push(segment.end());
                bounds.sort();
                bounds.dedup();
                for pair in bounds.windows(2) {
                    let piece = TextRange::new(pair[0], pair[1]);
                    let piece_advance: f32 = shape_segment(decision, piece)
                        .clusters
                        .iter()
                        .map(|cluster| cluster.advance)
                        .sum();
                    if !rejected.is_empty() || piece_advance > measure || span_advance > measure {
                        cuts.extend(
                            source_grapheme_boundaries(text, piece)
                                .into_iter()
                                .filter(|offset| *offset > piece.start() && *offset < piece.end()),
                        );
                    }
                }
                if rejected.contains(&ProgressiveBreakTier::Structural) {
                    cuts.extend(structural.iter().copied())
                }
                if rejected.contains(&ProgressiveBreakTier::Syllable) {
                    cuts.extend(technical_syllable.iter().copied())
                }
                cuts.sort();
                cuts.dedup();
                cuts
            } else {
                Vec::new()
            };
            if let Some(span) = progressive_span {
                for (tier, offsets) in [
                    (ProgressiveBreakTier::Structural, structural.clone()),
                    (ProgressiveBreakTier::Syllable, technical_syllable.clone()),
                    (ProgressiveBreakTier::Emergency, technical_emergency.clone()),
                ] {
                    if rejected_technical_tiers_by_span
                        .get(&span.range)
                        .is_some_and(|set| set.contains(&tier))
                    {
                        continue;
                    }
                    if !offsets.is_empty() {
                        decisions.push(BreakOpportunityDecisionInfo::with_tier(
                            segment,
                            word.clone(),
                            offsets.clone(),
                            if tier == ProgressiveBreakTier::Emergency
                                && rejected_technical_tiers_by_span
                                    .get(&span.range)
                                    .is_some_and(|s| !s.is_empty())
                            {
                                "CurrentLineTechnicalEmergencyBreak".to_owned()
                            } else {
                                "ProgressiveTechnicalBreak".to_owned()
                            },
                            Some(format!("{:?}", tier)),
                        ));
                    }
                    for offset in offsets {
                        let candidate = ProgressiveBreakOpportunity::new(tier, span.range);
                        if progressive
                            .get(&offset)
                            .is_none_or(|old| candidate.tier.priority() < old.tier.priority())
                        {
                            progressive.insert(offset, candidate);
                        }
                    }
                }
                let tier = if segment.start() > span.range.start()
                    && text
                        .code_point_before(segment.start())
                        .is_some_and(|c| char::from_u32(c as u32).is_some_and(char::is_whitespace))
                {
                    ProgressiveBreakTier::Whitespace
                } else {
                    ProgressiveBreakTier::WholeToken
                };
                if !rejected_technical_tiers_by_span
                    .get(&span.range)
                    .is_some_and(|set| set.contains(&tier))
                {
                    if progressive
                        .get(&segment.start())
                        .is_none_or(|current| tier.priority() < current.tier.priority())
                    {
                        progressive.insert(
                            segment.start(),
                            ProgressiveBreakOpportunity::new(tier, span.range),
                        );
                    }
                    decisions.push(BreakOpportunityDecisionInfo::with_tier(
                        segment,
                        word.clone(),
                        vec![segment.start()],
                        if tier == ProgressiveBreakTier::Whitespace {
                            "ProgressiveTechnicalWhitespaceBreak".to_owned()
                        } else {
                            "ProgressiveTechnicalWholeTokenWrap".to_owned()
                        },
                        Some(format!("{:?}", tier)),
                    ));
                }
                if !technical_emergency.is_empty()
                    && !emergency.iter().any(
                        |decision: &EmergencyTrackingEligibilityDecisionInfo| {
                            decision.range == span.range
                                && decision.reason
                                    == if let Some(rejected) = rejected_technical_tiers_by_span
                                        .get(&span.range)
                                        .filter(|it| !it.is_empty())
                                    {
                                        let mut names: Vec<_> = rejected.iter().copied().collect();
                                        names.sort_by_key(|tier| tier.priority());
                                        format!(
                                            "CurrentLineTechnicalTierRejection:{}",
                                            names
                                                .iter()
                                                .map(|tier| format!("{:?}", tier))
                                                .collect::<Vec<_>>()
                                                .join("+")
                                        )
                                    } else {
                                        "ProgressiveTechnicalSpan".to_owned()
                                    }
                        },
                    )
                {
                    emergency.push(EmergencyTrackingEligibilityDecisionInfo {
                        range: span.range,
                        source_text: text.slice_text(span.range),
                        reason: if let Some(rejected) = rejected_technical_tiers_by_span
                            .get(&span.range)
                            .filter(|it| !it.is_empty())
                        {
                            let mut names: Vec<_> = rejected.iter().copied().collect();
                            names.sort_by_key(|tier| tier.priority());
                            format!(
                                "CurrentLineTechnicalTierRejection:{}",
                                names
                                    .iter()
                                    .map(|tier| format!("{:?}", tier))
                                    .collect::<Vec<_>>()
                                    .join("+")
                            )
                        } else {
                            "ProgressiveTechnicalSpan".to_owned()
                        },
                    })
                }
            }
            let clean = if progressive_span.is_some() {
                [
                    structural.clone(),
                    technical_syllable.clone(),
                    technical_emergency.clone(),
                ]
                .concat()
            } else if !latin {
                Vec::new()
            } else if word.contains('-') {
                [
                    existing_hyphen_cuts(text, segment),
                    latin_separator_cuts(&word, segment, token_advance, measure, long_opaque),
                ]
                .concat()
            } else if camel {
                camel_case_cuts(text, segment)
            } else if !all_letters {
                latin_separator_cuts(&word, segment, token_advance, measure, long_opaque)
            } else {
                Vec::new()
            };
            if progressive_span.is_none() && latin && (word.contains('-') || !all_letters) {
                let locator = bibliographic_numeric_locator_break_offsets(&word);
                if !locator.is_empty() {
                    decisions.push(BreakOpportunityDecisionInfo::new(
                        segment,
                        word.clone(),
                        locator
                            .into_iter()
                            .map(|offset| segment.start() + offset)
                            .collect(),
                        "BibliographicNumericLocatorBreak".to_owned(),
                    ));
                }
            }
            let hyphen = if progressive_span.is_none()
                && all_letters
                && !abbreviation
                && !camel
                && !long_letters
                && !word.contains('-')
                && clean.is_empty()
            {
                latin_word_cuts(
                    text,
                    segment,
                    &syllable,
                    measure,
                    &mut shape_segment,
                    decision,
                )
            } else {
                Vec::new()
            };
            let opaque = if progressive_span.is_none()
                && latin
                && (!all_letters || long_letters)
                && (token_advance > measure || long_opaque)
            {
                opaque_hard_cuts(
                    text,
                    segment,
                    &clean,
                    measure,
                    long_opaque,
                    &mut shape_segment,
                    decision,
                )
            } else {
                Vec::new()
            };
            if progressive_span.is_none() && !opaque.is_empty() {
                let mut clean_bounds = vec![segment.start()];
                clean_bounds.extend_from_slice(&clean);
                clean_bounds.push(segment.end());
                clean_bounds.sort();
                clean_bounds.dedup();
                for pair in clean_bounds.windows(2) {
                    if opaque
                        .iter()
                        .any(|offset| *offset > pair[0] && *offset < pair[1])
                    {
                        let piece = text.slice_text(TextRange::new(pair[0], pair[1]));
                        if let Some(reason) = strong_non_lexical_reason(&piece) {
                            let range = TextRange::new(pair[0], pair[1]);
                            if !emergency.iter().any(
                                |decision: &EmergencyTrackingEligibilityDecisionInfo| {
                                    decision.range == range && decision.reason == reason
                                },
                            ) {
                                emergency.push(EmergencyTrackingEligibilityDecisionInfo {
                                    range,
                                    source_text: piece,
                                    reason: reason.to_owned(),
                                });
                            }
                        }
                    }
                }
            }
            let mut all_cuts = [clean, hyphen.clone(), opaque].concat();
            all_cuts.sort();
            all_cuts.dedup();
            cuts_by_segment.insert(segment, all_cuts);
            if !hyphen.is_empty() {
                for offset in &hyphen {
                    hyphen_offsets.insert(*offset);
                }
                if hyphen_advance.is_none() {
                    let h = text_shaper.shape(
                        &ShapingInput::builder(
                            Text::from("-"),
                            TextRange::new(0, 1),
                            input.text_style.clone(),
                            decision.clone(),
                        )
                        .display_text(Text::from("-"))
                        .build(),
                    );
                    hyphen_advance = Some(if h.clusters.len() == 1 {
                        h.clusters[0].advance
                    } else {
                        0.5 * font_size
                    });
                    hyphen_glyphs = h
                        .glyph_runs
                        .into_iter()
                        .flat_map(|run| run.glyphs)
                        .collect();
                }
            }
        }
    }
    let mut shaping_results = Vec::new();
    for resolved in cluster_ranges {
        if let Some(object) = inline_object_by_range.get(&resolved.range) {
            shaping_results.push(inline_object_shaping_result(text, object));
            continue;
        }
        if resolved.mandatory_break {
            shaping_results.push(mandatory_break_shaping_result(text, resolved.range));
            continue;
        }
        if resolved.zero_width_soft_break {
            shaping_results.push(zero_width_soft_break_shaping_result(text, resolved.range));
            continue;
        }
        let decision = font_decision_by_range
            .get(&resolved.range)
            .expect("shapeable range must have font decision");
        for segment in shaping_segments(decision, text) {
            let cuts = cuts_by_segment.get(&segment).cloned().unwrap_or_default();
            if cuts.is_empty() {
                shaping_results.push(shape_segment(decision, segment));
            } else {
                let bounds = [vec![segment.start()], cuts, vec![segment.end()]].concat();
                for pair in bounds.windows(2) {
                    for range in point_mark_prefixed_ranges(text, TextRange::new(pair[0], pair[1]))
                    {
                        shaping_results.push(shape_segment(decision, range));
                    }
                }
            }
        }
    }
    ParagraphShapingStageResult {
        shaping_results,
        hyphen_offsets,
        hyphen_advance: hyphen_advance.unwrap_or(0.),
        hyphen_glyphs,
        substitution_rollbacks: rollbacks,
        break_opportunity_decisions: decisions,
        emergency_tracking_eligibility_decisions: emergency,
        progressive_break_offsets: progressive,
        segment_shaping_cache: segment_cache,
    }
}
fn cjk_punctuation_full_width_features(role: FontRole, text: &Text) -> Vec<String> {
    if role == FontRole::CjkPunctuation
        && text
            .chars()
            .any(|c| matches!(c, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}'))
    {
        vec!["fwid=1".to_owned()]
    } else {
        Vec::new()
    }
}
fn dash_ink_coverage_deficient(result: &ShapingResult, display: &Text, size: f32) -> bool {
    if !display.contains('\u{2E3A}') {
        return false;
    }
    let glyphs: Vec<_> = result
        .glyph_runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .collect();
    let Some(glyph) = glyphs.first().filter(|_| glyphs.len() == 1) else {
        return false;
    };
    let Some(ink) = glyph.bounds else {
        return false;
    };
    ink.right - ink.left < DASH_SUBSTITUTION_TARGET_EM * size * DASH_SUBSTITUTION_MIN_INK_COVERAGE
}
fn shaping_segments(decision: &FontDecision, text: &Text) -> Vec<TextRange> {
    if decision.role != FontRole::LatinText {
        return vec![decision.range];
    }
    let mut out = Vec::new();
    let mut start = decision.range.start();
    let mut in_space = text.code_point_at_or_none(start) == Some(' ' as i32);
    for offset in utf16_offsets_after(text, decision.range) {
        let space = text.code_point_at_or_none(offset) == Some(' ' as i32);
        if space != in_space {
            out.push(TextRange::new(start, offset));
            start = offset;
            in_space = space;
        }
    }
    out.push(TextRange::new(start, decision.range.end()));
    out
}
fn utf16_offsets_after(text: &Text, range: TextRange) -> Vec<i32> {
    let mut out = Vec::new();
    let mut offset = range.start();
    while let Some(code) = text.code_point_at_or_none(offset) {
        let width = if code > 0xFFFF { 2 } else { 1 };
        offset += width;
        if offset < range.end() {
            out.push(offset)
        } else {
            break;
        }
    }
    out
}
fn point_mark_prefixed_ranges(text: &Text, range: TextRange) -> Vec<TextRange> {
    let mut end = range.start();
    while end < range.end()
        && text
            .code_point_at_or_none(end)
            .and_then(|code_point| char::from_u32(code_point as u32))
            .is_some_and(clreq_punctuation_policies::is_ascii_point_mark)
    {
        end += 1
    }
    if end > range.start() && end < range.end() {
        vec![
            TextRange::new(range.start(), end),
            TextRange::new(end, range.end()),
        ]
    } else {
        vec![range]
    }
}
fn latin_word_cuts(
    text: &Text,
    range: TextRange,
    syllable: &[i32],
    measure: f32,
    shape: &mut dyn FnMut(&FontDecision, TextRange) -> ShapingResult,
    decision: &FontDecision,
) -> Vec<i32> {
    let mut cuts: HashSet<_> = syllable.iter().map(|x| range.start() + x).collect();
    for pair in syllable_bounds(syllable, range.length()).windows(2) {
        let piece = TextRange::new(range.start() + pair[0], range.start() + pair[1]);
        let shaped = shape(decision, piece);
        if shaped.clusters.len() == 1 && shaped.clusters[0].advance > measure {
            let lo = pair[0] + HYPHEN_MIN_LEFT;
            let hi = pair[1] - HYPHEN_MIN_RIGHT;
            if lo <= hi {
                for x in lo..=hi {
                    cuts.insert(range.start() + x);
                }
            } else {
                for x in pair[0] + 1..pair[1] {
                    cuts.insert(range.start() + x);
                }
            }
        }
    }
    let mut out: Vec<_> = cuts.into_iter().collect();
    out.sort();
    let _ = text;
    out
}
fn syllable_bounds(syllable: &[i32], length: i32) -> Vec<i32> {
    let mut out = vec![0];
    out.extend_from_slice(syllable);
    out.push(length);
    out.sort();
    out.dedup();
    out
}
fn existing_hyphen_cuts(text: &Text, range: TextRange) -> Vec<i32> {
    let chars: Vec<_> = text.slice(range).chars().collect();
    let mut out = Vec::new();
    let mut offset = range.start();
    for (i, c) in chars.iter().enumerate() {
        if *c == '-'
            && chars[..i]
                .iter()
                .rev()
                .take_while(|x| x.is_alphabetic())
                .count()
                >= 2
            && chars[i + 1..]
                .iter()
                .take_while(|x| x.is_alphabetic())
                .count()
                >= 2
        {
            out.push(offset + 1)
        }
        offset += c.len_utf16() as i32;
    }
    out
}
fn camel_case_cuts(text: &Text, range: TextRange) -> Vec<i32> {
    let chars: Vec<_> = text.slice(range).chars().collect();
    let humps: Vec<_> = (1..chars.len())
        .filter(|index| {
            chars[*index].is_uppercase()
                && (chars[*index - 1].is_lowercase()
                    || (chars[*index - 1].is_uppercase()
                        && chars
                            .get(*index + 1)
                            .is_some_and(|character| character.is_lowercase())))
        })
        .collect();
    let mut bounds = Vec::with_capacity(humps.len() + 2);
    bounds.push(0);
    bounds.extend(humps.iter().copied());
    bounds.push(chars.len());
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    offsets.push(range.start());
    for character in &chars {
        offsets.push(offsets.last().copied().unwrap() + character.len_utf16() as i32)
    }
    humps
        .into_iter()
        .filter(|hump| {
            hump - bounds
                .iter()
                .copied()
                .rfind(|bound| *bound < *hump)
                .unwrap()
                >= 2
                && bounds.iter().copied().find(|bound| *bound > *hump).unwrap() - hump >= 2
        })
        .map(|hump| offsets[hump])
        .collect()
}
fn progressive_structural_cuts(text: &Text, range: TextRange) -> Vec<i32> {
    let mut out = camel_case_cuts(text, range);
    let chars: Vec<_> = text.slice(range).chars().collect();
    let mut offset = range.start();
    for (i, c) in chars.iter().enumerate() {
        if i + 1 < chars.len()
            && (PROGRESSIVE_TECHNICAL_BREAK_AFTER_CHARS.contains(c)
                || ((chars[i].is_alphabetic() && is_decimal_digit(chars[i + 1]))
                    || (is_decimal_digit(chars[i]) && chars[i + 1].is_alphabetic())))
        {
            out.push(offset + c.len_utf16() as i32)
        }
        offset += c.len_utf16() as i32;
    }
    out.sort();
    out.dedup();
    out
}
fn technical_syllable_cuts(
    text: &Text,
    range: TextRange,
    structural: &[i32],
    hyphenator: &dyn Hyphenator,
) -> Vec<i32> {
    let mut bounds = vec![range.start()];
    bounds.extend_from_slice(structural);
    bounds.push(range.end());
    let mut out = Vec::new();
    for pair in bounds.windows(2) {
        let mut offset = pair[0];
        while offset < pair[1] {
            while offset < pair[1]
                && text
                    .code_point_at_or_none(offset)
                    .is_some_and(|c| char::from_u32(c as u32).is_none_or(|x| !x.is_alphabetic()))
            {
                offset += 1
            }
            let start = offset;
            while offset < pair[1]
                && text
                    .code_point_at_or_none(offset)
                    .is_some_and(|c| char::from_u32(c as u32).is_some_and(char::is_alphabetic))
            {
                offset += 1
            }
            if offset > start {
                let word = text.slice_text(TextRange::new(start, offset));
                if strong_non_lexical_reason(&word).is_none() {
                    out.extend(
                        hyphenator
                            .hyphenate(&word)
                            .into_iter()
                            .filter(|x| *x > 0 && *x < offset - start)
                            .map(|x| start + x),
                    );
                }
            } else {
                offset += 1
            }
        }
    }
    out.retain(|x| !structural.contains(x));
    out.sort();
    out.dedup();
    out
}
fn latin_separator_cuts(
    word: &Text,
    range: TextRange,
    advance: f32,
    measure: f32,
    force: bool,
) -> Vec<i32> {
    let url = word.contains("://") || word.to_lowercase().starts_with("www.") || domain_like(word);
    let opaque = word.chars().any(|c| !c.is_alphabetic());
    let solidus = breakable_latin_solidus(word);
    let bibliographic = bibliographic_numeric_locator_break_offsets(word);
    let opaque_separator_mode = url || (opaque && (advance > measure || force));
    if !solidus && !opaque_separator_mode && bibliographic.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<_> = bibliographic
        .into_iter()
        .map(|offset| range.start() + offset)
        .collect();
    let chars: Vec<_> = word.chars().collect();
    let mut offset = range.start();
    for (index, character) in chars.iter().enumerate() {
        let break_after = (solidus && !url && *character == '/')
            || (opaque_separator_mode
                && match *character {
                    '/' => !(advance <= measure) || chars.get(index.wrapping_sub(1)) != Some(&':'),
                    '.' | '-' | '_' | '?' | '&' | '=' | '#' | '%' | '~' => true,
                    _ => false,
                });
        if index + 1 < chars.len() && break_after {
            out.push(offset + character.len_utf16() as i32)
        }
        offset += character.len_utf16() as i32;
    }
    out
}
fn opaque_hard_cuts(
    text: &Text,
    range: TextRange,
    clean: &[i32],
    measure: f32,
    force: bool,
    shape: &mut dyn FnMut(&FontDecision, TextRange) -> ShapingResult,
    decision: &FontDecision,
) -> Vec<i32> {
    let mut bounds = vec![range.start()];
    bounds.extend_from_slice(clean);
    bounds.push(range.end());
    let mut out = Vec::new();
    for pair in bounds.windows(2) {
        let piece = TextRange::new(pair[0], pair[1]);
        let shaped = shape(decision, piece);
        let piece_advance = if shaped.clusters.len() == 1 {
            shaped.clusters[0].advance
        } else {
            0.
        };
        if piece.length() > 1
            && (piece_advance > measure
                || (force && piece.length() >= LATIN_OPAQUE_TOKEN_MIN_LENGTH))
        {
            out.extend(
                source_grapheme_boundaries(text, piece)
                    .into_iter()
                    .filter(|x| *x > piece.start() && *x < piece.end()),
            );
        }
    }
    out.sort();
    out.dedup();
    out
}
fn domain_like(text: &Text) -> bool {
    let chars: Vec<_> = text.chars().collect();
    chars.iter().enumerate().any(|(index, character)| {
        *character == '.'
            && index > 0
            && index + 2 < chars.len()
            && chars[index - 1].is_alphanumeric()
            && chars[index + 1].is_alphanumeric()
            && chars[index + 1..]
                .iter()
                .take_while(|character| character.is_alphabetic())
                .count()
                >= 2
    })
}
fn breakable_latin_solidus(text: &Text) -> bool {
    let chars: Vec<_> = text.chars().collect();
    chars.iter().enumerate().any(|(index, character)| {
        *character == '/'
            && index > 0
            && index + 1 < chars.len()
            && chars[index - 1].is_alphanumeric()
            && chars[index + 1].is_alphanumeric()
    })
}
fn bibliographic_numeric_locator_break_offsets(text: &Text) -> Vec<i32> {
    let chars: Vec<_> = text.chars().collect();
    let Some(open) = chars.iter().position(|character| *character == '(') else {
        return Vec::new();
    };
    if open == 0 || !is_decimal_digit(chars[0]) {
        return Vec::new();
    }
    let Some(close) = chars
        .iter()
        .enumerate()
        .skip(open + 1)
        .find_map(|(index, character)| (*character == ')').then_some(index))
    else {
        return Vec::new();
    };
    if close <= open + 1 {
        return Vec::new();
    }
    let Some(colon) = chars
        .iter()
        .enumerate()
        .skip(close + 1)
        .find_map(|(index, character)| (*character == ':').then_some(index))
    else {
        return Vec::new();
    };
    if colon != close + 1 || colon >= chars.len() - 1 {
        return Vec::new();
    }
    let volume = &chars[..open];
    let issue = &chars[open + 1..close];
    let mut pages = &chars[colon + 1..];
    if pages.last() == Some(&'.') {
        pages = &pages[..pages.len() - 1]
    }
    if volume.is_empty()
        || issue.is_empty()
        || pages.is_empty()
        || !volume.iter().all(|character| is_decimal_digit(*character))
        || !issue.iter().all(|character| is_decimal_digit(*character))
    {
        return Vec::new();
    }
    let separator = pages
        .iter()
        .position(|character| matches!(character, '-' | '–' | '—'));
    let numeric = if let Some(separator) = separator {
        separator > 0
            && separator + 1 < pages.len()
            && pages[..separator]
                .iter()
                .all(|character| is_decimal_digit(*character))
            && pages[separator + 1..]
                .iter()
                .all(|character| is_decimal_digit(*character))
    } else {
        pages.iter().all(|character| is_decimal_digit(*character))
    };
    if !numeric {
        return Vec::new();
    }
    let utf16_at = |index: usize| {
        chars[..index]
            .iter()
            .map(|character| character.len_utf16() as i32)
            .sum()
    };
    vec![utf16_at(open), utf16_at(colon + 1)]
}
fn strong_non_lexical_reason(text: &Text) -> Option<&'static str> {
    let chars: Vec<_> = text.chars().collect();
    if (text.utf16_len() as usize) < EMERGENCY_TRACKING_TOKEN_MIN_LENGTH {
        return None;
    }
    if chars.iter().all(|c| c.is_alphabetic())
        && chars
            .iter()
            .all(|c| c.to_lowercase().eq(chars[0].to_lowercase()))
    {
        Some("LongRepeatedLetterRun")
    } else if chars.iter().any(|c| c.is_alphabetic())
        && chars
            .iter()
            .all(|c| is_decimal_digit(*c) || matches!(c.to_ascii_lowercase(), 'a'..='f'))
    {
        Some("LongHexIdentityRun")
    } else {
        let transitions = chars
            .windows(2)
            .filter(|p| {
                (p[0].is_alphabetic() && is_decimal_digit(p[1]))
                    || (is_decimal_digit(p[0]) && p[1].is_alphabetic())
            })
            .count();
        (transitions >= 2).then_some("LongMixedAlphaNumericIdentifier")
    }
}
fn is_decimal_digit(character: char) -> bool {
    CodePointMapData::<GeneralCategory>::new().get(character) == GeneralCategory::DecimalNumber
}
fn mandatory_break_shaping_result(text: &Text, range: TextRange) -> ShapingResult {
    ShapingResult::new(
        vec![Cluster::with_display_text(
            range,
            text.slice_text(range),
            Text::new(),
            "mandatory-break".to_owned(),
            0.,
        )],
        Vec::new(),
    )
}
fn zero_width_soft_break_shaping_result(text: &Text, range: TextRange) -> ShapingResult {
    let source = text.slice_text(range);
    ShapingResult::with_decisions(
        vec![Cluster::with_display_text(
            range,
            source.clone(),
            Text::new(),
            "zero-width-space".to_owned(),
            0.,
        )],
        Vec::new(),
        vec![
            ShapingDecisionInfo::builder(
                range,
                source,
                Text::new(),
                "zero-width-space".to_owned(),
                0,
                0.,
                "StructuralControl".to_owned(),
                "ZeroWidthSpaceSoftBreakNoShape".to_owned(),
            )
            .build(),
        ],
    )
}
fn inline_object_shaping_result(text: &Text, object: &InlineObjectSpan) -> ShapingResult {
    let source = text.slice_text(object.range);
    ShapingResult::with_decisions(
        vec![Cluster::with_display_text(
            object.range,
            source.clone(),
            Text::new(),
            "inline-object".to_owned(),
            object.advance,
        )],
        Vec::new(),
        vec![
            ShapingDecisionInfo::builder(
                object.range,
                source,
                Text::new(),
                "inline-object".to_owned(),
                0,
                object.advance,
                "InlineObject".to_owned(),
                "MeasurableOpaqueInlineObject:no-font-shaping".to_owned(),
            )
            .build(),
        ],
    )
}
pub fn is_mandatory_break_cluster(cluster: &Cluster) -> bool {
    cluster.font_key == "mandatory-break" && cluster.display_text.is_empty()
}
pub fn is_zero_width_soft_break_cluster(cluster: &Cluster) -> bool {
    cluster.font_key == "zero-width-space" && cluster.display_text.is_empty()
}
pub fn is_inline_object_cluster(cluster: &Cluster) -> bool {
    cluster.font_key == "inline-object"
}
pub fn map_to_cluster_range(glyphs: &[Glyph], cluster: &Cluster) -> Vec<Glyph> {
    let sum: f32 = glyphs.iter().map(|glyph| glyph.advance).sum();
    glyphs
        .iter()
        .cloned()
        .map(|mut glyph| {
            if sum <= 0. {
                glyph.advance = cluster.advance / glyphs.len().max(1) as f32;
            }
            glyph.cluster_range = cluster.range;
            glyph
        })
        .collect()
}
const DASH_SUBSTITUTION_MIN_INK_COVERAGE: f32 = 0.85;
const DASH_SUBSTITUTION_TARGET_EM: f32 = 2.;
const HYPHEN_MIN_LEFT: i32 = 2;
const HYPHEN_MIN_RIGHT: i32 = 3;
const LATIN_OPAQUE_TOKEN_MIN_LENGTH: i32 = 24;
const EMERGENCY_TRACKING_TOKEN_MIN_LENGTH: usize = 12;
const PROGRESSIVE_TECHNICAL_BREAK_AFTER_CHARS: &[char] = &[
    '/', '\\', '.', '-', '_', ':', ';', ',', '?', '&', '=', '#', '%', '~', '+', '*', '|', ')', ']',
    '}',
];
