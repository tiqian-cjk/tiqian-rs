// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/AnnotationGeometryStage.kt

use crate::common::HashMap;

use super::super::clreq::bopomofo_reading::{BopomofoTone, bopomofo_parser};
use super::super::clreq::clreq_profile::ClreqProfile;
use super::super::core::geometry::{Rect, ScalarOffset, TextRange};
use super::super::core::int_range::IntRange;
use super::super::core::layout_model::{
    AutoSpaceDecisionInfo, BopomofoDecisionInfo, BopomofoGlyphPlacement, BopomofoGlyphRole,
    Cluster, ClusterGeometryDecisionInfo, DecorationDecisionInfo, DecorationSegmentInfo, Glyph,
    InlineObjectDecisionInfo, LineBox, RubyDecisionInfo,
};
use super::super::core::text::Text;
use super::super::core::text_model::{
    DecorationKind, DecorationSpan, InlineObjectBoundaryAdjustment, InlineObjectSpan, LayoutInput,
    RubyKind, RubySpan, TextStyle,
};
use super::super::font::font_policy::{FallbackResolver, FontRequest, FontRole};
use super::super::shaping::text_shaper::{ShapingInput, TextShaper};
use super::line_break_planning_stage::CJK_FACE_DESCENT_FALLBACK_EM;
use super::line_geometry_stage::ClusterMetricDecision;
use super::line_optimization::LineSolution;

#[derive(Clone, Debug, PartialEq)]
pub struct RubyFontGeometry {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub required_extent: f32,
    pub glyphs: Vec<Glyph>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationGeometryStageResult {
    pub inline_object_decisions: Vec<InlineObjectDecisionInfo>,
    pub decoration_decisions: Vec<DecorationDecisionInfo>,
    pub decoration_segments: Vec<DecorationSegmentInfo>,
    pub ruby_decisions: Vec<RubyDecisionInfo>,
    pub bopomofo_decisions: Vec<BopomofoDecisionInfo>,
}

pub struct AnnotationGeometryRequest<'a> {
    pub input: &'a LayoutInput,
    pub font_size: f32,
    pub inline_object_by_cluster_index: &'a HashMap<i32, InlineObjectSpan>,
    pub line_solution: &'a LineSolution,
    pub clreq_profile: &'a ClreqProfile,
    pub geometry_decisions: &'a [ClusterGeometryDecisionInfo],
    pub auto_space_decisions: &'a [AutoSpaceDecisionInfo],
    pub visible_line_ranges: &'a [IntRange],
    pub lines: &'a [LineBox],
    pub final_clusters: &'a [Cluster],
    pub cluster_roles: &'a [FontRole],
    pub justify_delta_by_cluster: &'a HashMap<i32, f32>,
    pub ruby_and_bopomofo_spread: &'a HashMap<i32, f32>,
    pub metric_decisions: &'a [ClusterMetricDecision],
    pub pinyin_spans: &'a [RubySpan],
    pub natural_clusters: &'a [Cluster],
    pub ruby_font_geometry_by_span: &'a HashMap<RubySpan, RubyFontGeometry>,
    pub ruby_stack_gap: f32,
    pub base_ascent: f32,
    pub ruby_font_size: f32,
    pub ruby_font_weight: i32,
    pub base_descent: f32,
    pub bopomofo_font_weight_at: &'a dyn Fn(ScalarOffset) -> i32,
    pub fallback_resolver: &'a dyn FallbackResolver,
    pub text_shaper: &'a dyn TextShaper,
}

pub fn resolve_annotation_geometry(
    request: AnnotationGeometryRequest<'_>,
) -> AnnotationGeometryStageResult {
    let mut object_entries: Vec<_> = request.inline_object_by_cluster_index.iter().collect();
    object_entries.sort_by_key(|(index, _)| **index);
    let inline_object_decisions = object_entries
        .into_iter()
        .map(|(cluster_index, object)| {
            let leading = &object.leading_boundary;
            let trailing = &object.trailing_boundary;
            InlineObjectDecisionInfo::builder(
                object.range,
                object.advance,
                object.ascent,
                object.descent,
                *cluster_index,
                request
                    .line_solution
                    .lines
                    .iter()
                    .position(|line| line.cluster_range.contains(*cluster_index))
                    .map_or(-1, |index| index as i32),
            )
            .leading_uniform_stretch(leading.participates_in_uniform_stretch)
            .leading_preferred_stretch_kind(
                leading
                    .preferred_stretch
                    .map(|stretch| format!("{:?}", stretch.kind)),
            )
            .leading_preferred_stretch_natural_width(
                leading
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.natural_width),
            )
            .leading_preferred_stretch_target_width(
                leading
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.target_width),
            )
            .leading_preferred_stretch_capacity(
                leading
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.capacity()),
            )
            .leading_prevents_line_break(leading.prevents_line_break)
            .leading_shrink_capacity(leading.shrink_capacity)
            .leading_line_end_discardable_advance(leading.line_end_discardable_advance)
            .trailing_uniform_stretch(trailing.participates_in_uniform_stretch)
            .trailing_preferred_stretch_kind(
                trailing
                    .preferred_stretch
                    .map(|stretch| format!("{:?}", stretch.kind)),
            )
            .trailing_preferred_stretch_natural_width(
                trailing
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.natural_width),
            )
            .trailing_preferred_stretch_target_width(
                trailing
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.target_width),
            )
            .trailing_preferred_stretch_capacity(
                trailing
                    .preferred_stretch
                    .map_or(0.0, |stretch| stretch.capacity()),
            )
            .trailing_prevents_line_break(trailing.prevents_line_break)
            .trailing_shrink_capacity(trailing.shrink_capacity)
            .trailing_line_end_discardable_advance(trailing.line_end_discardable_advance)
            .reason(
                if *leading != InlineObjectBoundaryAdjustment::FIXED
                    || *trailing != InlineObjectBoundaryAdjustment::FIXED
                {
                    "AdjustableInlineObject".to_owned()
                } else {
                    "MeasurableOpaqueInlineObject".to_owned()
                },
            )
            .build()
        })
        .collect();
    let geometry_by_range: HashMap<_, _> = request
        .geometry_decisions
        .iter()
        .map(|decision| (decision.range, decision))
        .collect();
    let leading_gap_ranges: Vec<_> = request
        .auto_space_decisions
        .iter()
        .filter(|decision| decision.side == "leading")
        .map(|decision| decision.cluster_range)
        .collect();
    let trailing_gap_ranges: Vec<_> = request
        .auto_space_decisions
        .iter()
        .filter(|decision| decision.side == "trailing")
        .map(|decision| decision.cluster_range)
        .collect();
    let decoration_decisions = compute_decoration_decisions(
        request.input,
        request.visible_line_ranges,
        request.lines,
        request.final_clusters,
        request.cluster_roles,
        request.justify_delta_by_cluster,
        request.ruby_and_bopomofo_spread,
        request.metric_decisions,
        request.font_size,
    );
    let decoration_segments = compute_decoration_segments(
        &request.input.decorations,
        request.visible_line_ranges,
        request.lines,
        request.final_clusters,
        request.justify_delta_by_cluster,
        &geometry_by_range,
        &leading_gap_ranges,
        &trailing_gap_ranges,
        request.clreq_profile.auto_space.gap_em * request.font_size,
        request.font_size,
    );
    let ruby_decisions = compute_ruby_decisions(
        request.pinyin_spans,
        request.visible_line_ranges,
        request.lines,
        request.final_clusters,
        request.natural_clusters,
        request.metric_decisions,
        request.ruby_font_geometry_by_span,
        request.ruby_stack_gap,
        request.base_ascent,
        request.ruby_font_size,
        request.ruby_font_weight,
        &request.input.text_style.locale,
    );
    let bopomofo_spans: Vec<_> = request
        .input
        .ruby_spans
        .iter()
        .filter(|ruby| ruby.kind == RubyKind::Bopomofo)
        .collect();
    let bopomofo_decisions = compute_bopomofo_decisions(
        &bopomofo_spans,
        request.visible_line_ranges,
        request.lines,
        request.final_clusters,
        request.natural_clusters,
        request.base_ascent,
        request.base_descent,
        request.font_size,
        request.bopomofo_font_weight_at,
        &request.input.text_style,
        request.fallback_resolver,
        request.text_shaper,
    );
    AnnotationGeometryStageResult {
        inline_object_decisions,
        decoration_decisions,
        decoration_segments,
        ruby_decisions,
        bopomofo_decisions,
    }
}

fn compute_decoration_decisions(
    input: &LayoutInput,
    line_ranges: &[IntRange],
    lines: &[LineBox],
    final_clusters: &[Cluster],
    cluster_roles: &[FontRole],
    justify_delta_by_cluster: &HashMap<i32, f32>,
    ruby_spread_by_cluster: &HashMap<i32, f32>,
    metric_decisions: &[ClusterMetricDecision],
    font_size: f32,
) -> Vec<DecorationDecisionInfo> {
    let mut decisions = Vec::new();
    for span in input
        .decorations
        .iter()
        .filter(|span| span.kind == DecorationKind::Emphasis)
    {
        for (line_index, cluster_range) in line_ranges.iter().enumerate() {
            let mut x = lines[line_index].indent;
            for index in *cluster_range {
                let cluster = &final_clusters[index as usize];
                if contains_range(span.range, cluster.range) {
                    let role = cluster_roles[index as usize];
                    let applied = role == FontRole::CjkText;
                    let glyph_advance = cluster.advance
                        - justify_delta_by_cluster.get(&index).copied().unwrap_or(0.0)
                        - ruby_spread_by_cluster.get(&index).copied().unwrap_or(0.0);
                    let metric = metric_decisions
                        .iter()
                        .find(|metric| contains_range(metric.range, cluster.range));
                    let cluster_em = metric.map_or(font_size, |metric| metric.request.font_size);
                    let face_descent = metric
                        .map_or(cluster_em * CJK_FACE_DESCENT_FALLBACK_EM, |metric| {
                            metric.layout_metrics.descent
                        });
                    let candidate_dot_diameter = cluster_em * EMPHASIS_DOT_DIAMETER_EM;
                    decisions.push(
                        DecorationDecisionInfo::builder(
                            cluster.range,
                            cluster.text.clone(),
                            format!("{:?}", span.kind),
                            applied,
                            if applied {
                                "EmphasisDotOnHanText".to_owned()
                            } else if role == FontRole::CjkPunctuation {
                                "clreq-no-dot-on-punctuation".to_owned()
                            } else {
                                "no-dot-on-non-han".to_owned()
                            },
                        )
                        .anchor_x(x + glyph_advance / 2.0)
                        .anchor_y(
                            lines[line_index].baseline
                                + cluster.baseline_shift
                                + face_descent
                                + cluster_em * input.paragraph_style.emphasis_dot_gap_em
                                + candidate_dot_diameter / 2.0,
                        )
                        .dot_diameter(if applied { candidate_dot_diameter } else { 0.0 })
                        .build(),
                    );
                }
                x += cluster.advance;
            }
        }
    }
    decisions
}

fn compute_decoration_segments(
    decorations: &[DecorationSpan],
    line_ranges: &[IntRange],
    lines: &[LineBox],
    final_clusters: &[Cluster],
    justify_delta_by_cluster: &HashMap<i32, f32>,
    geometry_by_range: &HashMap<TextRange, &ClusterGeometryDecisionInfo>,
    leading_gap_ranges: &[TextRange],
    trailing_gap_ranges: &[TextRange],
    auto_space_gap_px: f32,
    font_size: f32,
) -> Vec<DecorationSegmentInfo> {
    let mut segments = Vec::new();
    for span in decorations.iter().filter(|span| {
        matches!(
            span.kind,
            DecorationKind::Mourning | DecorationKind::ProperNoun | DecorationKind::BookTitle
        )
    }) {
        let mut span_segments = Vec::new();
        for (line_index, cluster_range) in line_ranges.iter().enumerate() {
            let mut x = lines[line_index].indent;
            let mut left = None;
            let mut right = 0.0;
            let mut segment_start = None;
            let mut segment_end = None;
            for index in *cluster_range {
                let cluster = &final_clusters[index as usize];
                if contains_range(span.range, cluster.range) {
                    if left.is_none() {
                        let geometry = geometry_by_range.get(&cluster.range).copied();
                        let blank = geometry.map_or(0.0, |geometry| {
                            geometry.leading_glue_natural - geometry.leading_glue_consumed
                        }) + if leading_gap_ranges.contains(&cluster.range)
                            && index != cluster_range.first()
                        {
                            auto_space_gap_px
                        } else {
                            0.0
                        };
                        left = Some(x + blank);
                        segment_start = Some(cluster.range.start());
                    }
                    let geometry = geometry_by_range.get(&cluster.range).copied();
                    let blank = geometry.map_or(0.0, |geometry| {
                        geometry.trailing_glue_natural - geometry.trailing_glue_consumed
                    }) + if trailing_gap_ranges.contains(&cluster.range)
                        && index != cluster_range.last()
                    {
                        auto_space_gap_px
                    } else {
                        0.0
                    };
                    right = x + cluster.advance
                        - justify_delta_by_cluster.get(&index).copied().unwrap_or(0.0)
                        - blank;
                    segment_end = Some(cluster.range.end());
                }
                x += cluster.advance;
            }
            let (Some(left), Some(segment_start), Some(segment_end)) =
                (left, segment_start, segment_end)
            else {
                continue;
            };
            let line = &lines[line_index];
            let interlinear = span.kind != DecorationKind::Mourning;
            let line_y = line.baseline
                + font_size
                    * if span.kind == DecorationKind::BookTitle {
                        BOOK_TITLE_WAVE_LINE_Y_EM
                    } else {
                        INTERLINEAR_LINE_Y_EM
                    };
            span_segments.push(DecorationSegmentInfo {
                source_range: TextRange::new(segment_start, segment_end),
                kind: format!("{:?}", span.kind),
                line_index: line_index as i32,
                left,
                top: if interlinear {
                    line_y
                } else {
                    line.baseline - font_size * MOURNING_FRAME_FACE_ASCENT_EM
                },
                right,
                bottom: if interlinear {
                    line_y
                } else {
                    line.baseline + font_size * MOURNING_FRAME_FACE_DESCENT_EM
                },
                open_start: segment_start > span.range.start(),
                open_end: segment_end < span.range.end(),
                reason: String::new(),
            });
        }
        let reason = if span.kind == DecorationKind::Mourning && span_segments.len() <= 1 {
            "MourningSpanKeptUnbroken"
        } else if span.kind == DecorationKind::Mourning {
            "mourning-span-split-across-lines"
        } else {
            "InterlinearLinePerAnnotatedItem"
        };
        segments.extend(span_segments.into_iter().map(|mut segment| {
            segment.reason = reason.to_owned();
            segment
        }));
    }
    shorten_adjacent_interlinear_lines(segments, font_size)
}

fn shorten_adjacent_interlinear_lines(
    mut segments: Vec<DecorationSegmentInfo>,
    font_size: f32,
) -> Vec<DecorationSegmentInfo> {
    let mut indices_by_line: HashMap<i32, Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        if matches!(segment.kind.as_str(), "ProperNoun" | "BookTitle") {
            indices_by_line
                .entry(segment.line_index)
                .or_default()
                .push(index);
        }
    }
    let mut line_indices: Vec<_> = indices_by_line.into_iter().collect();
    line_indices.sort_by_key(|(line_index, _)| *line_index);
    for (_, mut indices) in line_indices {
        indices.sort_by(|left, right| segments[*left].left.total_cmp(&segments[*right].left));
        for pair in indices.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if segments[right].left - segments[left].right > ADJACENT_LINE_EPSILON * font_size {
                continue;
            }
            let pullback = font_size * ADJACENT_LINE_SHORTEN_EM;
            segments[left].right -= pullback;
            segments[left].reason = format!(
                "{};AdjacentInterlinearLineShortening",
                segments[left].reason
            );
            segments[right].left += pullback;
            segments[right].reason = format!(
                "{};AdjacentInterlinearLineShortening",
                segments[right].reason
            );
        }
    }
    segments
}

fn compute_ruby_decisions(
    ruby_spans: &[RubySpan],
    line_ranges: &[IntRange],
    lines: &[LineBox],
    final_clusters: &[Cluster],
    natural_clusters: &[Cluster],
    metric_decisions: &[ClusterMetricDecision],
    ruby_font_geometry_by_span: &HashMap<RubySpan, RubyFontGeometry>,
    ruby_stack_gap: f32,
    fallback_base_ascent: f32,
    ruby_font_size: f32,
    ruby_font_weight: i32,
    base_locale: &str,
) -> Vec<RubyDecisionInfo> {
    let mut decisions = Vec::new();
    for ruby in ruby_spans {
        let geometry = ruby_font_geometry_by_span
            .get(ruby)
            .expect("pinyin ruby span must have measured geometry");
        for (line_index, cluster_range) in line_ranges.iter().enumerate() {
            let mut x = lines[line_index].indent;
            let mut base_left = None;
            let mut content_width = 0.0;
            let mut base_face_top = f32::INFINITY;
            for index in *cluster_range {
                let cluster = &final_clusters[index as usize];
                if contains_range(ruby.base_range, cluster.range) {
                    base_left.get_or_insert(x);
                    content_width += natural_clusters[index as usize].advance;
                    let ascent = metric_decisions
                        .iter()
                        .find(|metric| contains_range(metric.range, cluster.range))
                        .map_or(fallback_base_ascent, |metric| metric.layout_metrics.ascent);
                    base_face_top = base_face_top
                        .min(lines[line_index].baseline + cluster.baseline_shift - ascent);
                }
                x += cluster.advance;
            }
            if let Some(base_left) = base_left {
                decisions.push(
                    RubyDecisionInfo::builder(
                        ruby.base_range,
                        ruby.text.clone(),
                        line_index as i32,
                        base_left + content_width / 2.0,
                        base_face_top - ruby_stack_gap - geometry.descent,
                        ruby_font_size,
                        ((geometry.width - content_width) / 2.0).max(0.0),
                    )
                    .ascent(geometry.ascent)
                    .descent(geometry.descent)
                    .width(geometry.width)
                    .font_families(ruby.font_families.clone())
                    .font_weight(ruby_font_weight)
                    .locale(
                        ruby.locale
                            .clone()
                            .unwrap_or_else(|| base_locale.to_owned()),
                    )
                    .glyphs(geometry.glyphs.clone())
                    .build(),
                );
            }
        }
    }
    decisions
}

#[allow(clippy::too_many_arguments)]
fn compute_bopomofo_decisions(
    ruby_spans: &[&RubySpan],
    line_ranges: &[IntRange],
    lines: &[LineBox],
    final_clusters: &[Cluster],
    natural_clusters: &[Cluster],
    base_ascent: f32,
    base_descent: f32,
    font_size: f32,
    bopomofo_font_weight_at: &dyn Fn(ScalarOffset) -> i32,
    base_text_style: &TextStyle,
    fallback_resolver: &dyn FallbackResolver,
    text_shaper: &dyn TextShaper,
) -> Vec<BopomofoDecisionInfo> {
    let h_unit = font_size / 30.0;
    let v_unit = (base_ascent + base_descent) / 30.0;
    let mut decisions = Vec::new();
    for ruby in ruby_spans {
        let locale = ruby
            .locale
            .clone()
            .unwrap_or_else(|| base_text_style.locale.clone());
        for (line_index, cluster_range) in line_ranges.iter().enumerate() {
            let mut x = lines[line_index].indent;
            let mut content_left = None;
            let mut content_width = 0.0;
            for index in *cluster_range {
                let cluster = &final_clusters[index as usize];
                if contains_range(ruby.base_range, cluster.range) {
                    content_left.get_or_insert(x);
                    content_width += natural_clusters[index as usize].advance;
                }
                x += cluster.advance;
            }
            let Some(content_left) = content_left else {
                continue;
            };
            let zone_left = content_left + content_width;
            let box_top = lines[line_index].baseline - base_ascent;
            let parsed = bopomofo_parser::parse(&ruby.text);
            let count = parsed.symbols.len().clamp(1, 3);
            let neutral = parsed.tone == BopomofoTone::Neutral;
            let mut placements = Vec::new();
            let mut place = |left_units: f32,
                             width_units: f32,
                             top_units: i32,
                             bottom_units: i32,
                             role: BopomofoGlyphRole,
                             text: Text| {
                placements.push(BopomofoGlyphPlacement::new(
                    text,
                    zone_left + left_units * h_unit,
                    box_top + top_units as f32 * v_unit,
                    width_units * h_unit,
                    (bottom_units - top_units) as f32 * v_unit,
                    role,
                ))
            };
            if neutral {
                let (top, bottom) = bopomofo_neutral_row(count);
                place(
                    1.0,
                    9.0,
                    top,
                    bottom,
                    BopomofoGlyphRole::Neutral,
                    Text::from("˙"),
                );
            }
            for (symbol, (top, bottom)) in parsed
                .symbols
                .iter()
                .take(3)
                .zip(bopomofo_symbol_rows(count, neutral))
            {
                place(
                    1.0,
                    9.0,
                    top,
                    bottom,
                    BopomofoGlyphRole::Symbol,
                    symbol.clone(),
                );
            }
            match parsed.tone {
                BopomofoTone::Yangping | BopomofoTone::Shang | BopomofoTone::Qu => {
                    let (top, bottom) = bopomofo_regular_tone_row(count);
                    place(
                        10.0,
                        5.0,
                        top,
                        bottom,
                        BopomofoGlyphRole::Tone,
                        Text::from(bopomofo_tone_glyph(parsed.tone)),
                    );
                }
                BopomofoTone::Ru => {
                    let (top, bottom) = bopomofo_ru_tone_row(count);
                    place(
                        10.0,
                        5.0,
                        top,
                        bottom,
                        BopomofoGlyphRole::Tone,
                        Text::from(bopomofo_tone_glyph(parsed.tone)),
                    );
                }
                BopomofoTone::Yinping | BopomofoTone::Neutral => {}
            }
            if placements.is_empty() {
                continue;
            }
            let weight = bopomofo_font_weight_at(ruby.base_range.start());
            let replay = placements
                .into_iter()
                .map(|placement| {
                    replay_bopomofo_placement(
                        placement,
                        ruby,
                        &locale,
                        weight,
                        font_size,
                        base_text_style,
                        fallback_resolver,
                        text_shaper,
                    )
                })
                .collect();
            decisions.push(
                BopomofoDecisionInfo::builder(
                    ruby.base_range,
                    ruby.text.clone(),
                    line_index as i32,
                    replay,
                )
                .font_families(ruby.font_families.clone())
                .font_weight(weight)
                .locale(locale.clone())
                .build(),
            );
        }
    }
    decisions
}

fn replay_bopomofo_placement(
    placement: BopomofoGlyphPlacement,
    ruby: &RubySpan,
    locale: &str,
    weight: i32,
    font_size: f32,
    base_text_style: &TextStyle,
    fallback_resolver: &dyn FallbackResolver,
    text_shaper: &dyn TextShaper,
) -> BopomofoGlyphPlacement {
    let replay_size = if placement.role == BopomofoGlyphRole::Neutral {
        placement.width
    } else {
        font_size * BOPOMOFO_ANNOTATION_FONT_EM
    };
    let range = TextRange::new(ScalarOffset::ZERO, placement.text.scalar_len());
    let decision = fallback_resolver.resolve(
        &placement.text,
        range,
        &FontRequest {
            preferred_families: ruby.font_families.clone(),
            locale: locale.to_owned(),
            role: FontRole::CjkText,
        },
    );
    let mut style = base_text_style.clone();
    style.font_size = replay_size;
    style.font_families = ruby.font_families.clone();
    style.font_weight = weight;
    style.italic = false;
    style.locale = locale.to_owned();
    let shaped = text_shaper.shape(
        &ShapingInput::builder(placement.text.clone(), range, style, decision)
            .display_text(placement.text.clone())
            .open_type_features(vec!["vert=1".to_owned()])
            .build(),
    );
    let glyphs: Vec<_> = shaped
        .glyph_runs
        .iter()
        .flat_map(|run| run.glyphs.iter().cloned())
        .collect();
    let advance: f32 = shaped.clusters.iter().map(|cluster| cluster.advance).sum();
    let ink = union_ink_bounds(&glyphs);
    let draw_x = match placement.role {
        BopomofoGlyphRole::Symbol | BopomofoGlyphRole::Neutral => {
            placement.left + (placement.width - advance) / 2.0
        }
        BopomofoGlyphRole::Tone => {
            placement.left + placement.width / 2.0
                - ((ink.map_or(0.0, |bounds| bounds.left)
                    + ink.map_or(advance, |bounds| bounds.right))
                    / 2.0)
        }
    };
    let baseline_y = match placement.role {
        BopomofoGlyphRole::Symbol => {
            placement.top + placement.height * BOPOMOFO_SYMBOL_BASELINE_FACTOR
        }
        BopomofoGlyphRole::Neutral | BopomofoGlyphRole::Tone => {
            placement.top + placement.height / 2.0
                - ((ink.map_or(0.0, |bounds| bounds.top) + ink.map_or(0.0, |bounds| bounds.bottom))
                    / 2.0)
        }
    };
    BopomofoGlyphPlacement::builder(
        placement.text,
        placement.left,
        placement.top,
        placement.width,
        placement.height,
        placement.role,
    )
    .glyphs(glyphs)
    .draw_x(draw_x)
    .baseline_y(baseline_y)
    .font_size(replay_size)
    .build()
}

fn union_ink_bounds(glyphs: &[Glyph]) -> Option<Rect> {
    let bounds: Vec<_> = glyphs
        .iter()
        .filter_map(|glyph| {
            glyph.bounds.map(|bounds| Rect {
                left: bounds.left + glyph.x,
                top: bounds.top + glyph.y,
                right: bounds.right + glyph.x,
                bottom: bounds.bottom + glyph.y,
            })
        })
        .collect();
    (!bounds.is_empty()).then(|| Rect {
        left: bounds
            .iter()
            .map(|bounds| bounds.left)
            .fold(f32::INFINITY, f32::min),
        top: bounds
            .iter()
            .map(|bounds| bounds.top)
            .fold(f32::INFINITY, f32::min),
        right: bounds
            .iter()
            .map(|bounds| bounds.right)
            .fold(f32::NEG_INFINITY, f32::max),
        bottom: bounds
            .iter()
            .map(|bounds| bounds.bottom)
            .fold(f32::NEG_INFINITY, f32::max),
    })
}

fn contains_range(outer: TextRange, inner: TextRange) -> bool {
    inner.start() >= outer.start() && inner.end() <= outer.end()
}
fn bopomofo_symbol_rows(count: usize, neutral: bool) -> Vec<(i32, i32)> {
    match count {
        0 | 1 => vec![(11, 20)],
        2 => vec![(6, 15), (17, 26)],
        _ if neutral => vec![(3, 12), (12, 21), (21, 30)],
        _ => vec![(2, 11), (11, 20), (20, 29)],
    }
}
fn bopomofo_neutral_row(count: usize) -> (i32, i32) {
    match count {
        1 => (8, 10),
        2 => (3, 5),
        _ => (0, 2),
    }
}
fn bopomofo_regular_tone_row(count: usize) -> (i32, i32) {
    match count {
        1 => (9, 14),
        2 => (15, 20),
        _ => (18, 23),
    }
}
fn bopomofo_ru_tone_row(count: usize) -> (i32, i32) {
    match count {
        1 => (16, 21),
        2 => (21, 26),
        _ => (24, 29),
    }
}
fn bopomofo_tone_glyph(tone: BopomofoTone) -> &'static str {
    match tone {
        BopomofoTone::Yangping => "ˊ",
        BopomofoTone::Shang => "ˇ",
        BopomofoTone::Qu => "ˋ",
        BopomofoTone::Neutral => "˙",
        _ => "",
    }
}

const EMPHASIS_DOT_DIAMETER_EM: f32 = 0.19;
const BOPOMOFO_ANNOTATION_FONT_EM: f32 = 0.3;
const BOPOMOFO_SYMBOL_BASELINE_FACTOR: f32 = 0.88;
const MOURNING_FRAME_FACE_ASCENT_EM: f32 = 0.88;
const MOURNING_FRAME_FACE_DESCENT_EM: f32 = 0.12;
const INTERLINEAR_LINE_Y_EM: f32 = 0.18;
const BOOK_TITLE_WAVE_LINE_Y_EM: f32 = 0.24;
const ADJACENT_LINE_SHORTEN_EM: f32 = 0.0625;
const ADJACENT_LINE_EPSILON: f32 = 0.01;
