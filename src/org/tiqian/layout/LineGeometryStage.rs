// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineGeometryStage.kt

use std::collections::HashMap;

use super::super::core::IntRange::IntRange;
use super::super::core::LayoutModel::{
    Cluster, Glyph, InlineObjectLineHeightDecisionInfo, LineBox, LineDebugInfo, LineEndReason,
    MaxLinesDecisionInfo, RubyLineHeightDecisionInfo,
};
use super::super::core::TextModel::{
    InlineObjectSpan, LastLineAlignment, LayoutInput, RubyLineHeightMode, RubySpan,
};
use super::super::font::FontMetrics::{FontMetricsRequest, MetricBox};
use super::super::font::FontPolicy::{LayoutFontMetrics, RawFontMetrics};
use super::AnnotationGeometryStage::RubyFontGeometry;
use super::Justifier::JustificationPlan;
use super::LineOptimization::LineSolution;
use super::LineOptimization::RepairOption;
use super::ParagraphShapingStage::is_inline_object_cluster;
use super::PunctuationGeometryLedger::cluster_index_range_for;

const EMPTY_PARAGRAPH_BASELINE_RATIO: f32 = 0.75;

#[derive(Clone, Debug, PartialEq)]
pub struct LineBoxStageResult {
    pub laid_out_lines: Vec<LineBox>,
    pub visible_lines: Vec<LineBox>,
    pub max_lines_decision: Option<MaxLinesDecisionInfo>,
    pub visible_line_ranges: Vec<IntRange>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LineVerticalGeometryStageResult {
    pub ruby_line_height_decision: Option<RubyLineHeightDecisionInfo>,
    pub inline_object_line_height_decision: Option<InlineObjectLineHeightDecisionInfo>,
    pub line_baseline: Vec<f32>,
    pub line_top: Vec<f32>,
    pub line_bottom: Vec<f32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterMetricDecision {
    pub range: super::super::core::Geometry::TextRange,
    pub source_text: String,
    pub request: FontMetricsRequest,
    pub raw_metrics: RawFontMetrics,
    pub layout_metrics: LayoutFontMetrics,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedLineMetrics {
    pub baseline: f32,
    pub height: f32,
    pub extra_leading: f32,
}

pub fn build_line_boxes(
    input: &LayoutInput,
    line_solution: &LineSolution,
    trimmed_clusters: &[Cluster],
    final_clusters: &[Cluster],
    first_line_indent: f32,
    block_indent: f32,
    measure: f32,
    grid_body_offset: f32,
    line_baseline: &[f32],
    line_top: &[f32],
    line_bottom: &[f32],
    line_hyphen_advance_at: &dyn Fn(i32) -> f32,
    hyphen_glyphs: &[Glyph],
    justification_plans: &[Option<JustificationPlan>],
) -> LineBoxStageResult {
    let laid_out_lines: Vec<_> = line_solution
        .lines
        .iter()
        .enumerate()
        .map(|(line_index, candidate)| {
            // Kotlin's sumOf starts from +0f. Rust's f32::sum yields -0.0
            // for an empty iterator, so add +0.0 to retain the same
            // observable width for empty mandatory-break lines.
            let adjusted_width: f32 = candidate
                .cluster_range
                .into_iter()
                .filter(|index| !candidate.hanging_cluster_indices.contains(index))
                .map(|index| trimmed_clusters[index as usize].advance)
                .sum::<f32>()
                + 0.0;
            let visual_width: f32 = candidate
                .cluster_range
                .into_iter()
                .map(|index| final_clusters[index as usize].advance)
                .sum::<f32>()
                + 0.0;
            let hanging_punctuation_advance: f32 = candidate
                .hanging_cluster_indices
                .iter()
                .map(|index| final_clusters[*index as usize].advance)
                .sum::<f32>()
                + 0.0;
            let drawable = !candidate.cluster_range.is_empty()
                && candidate
                    .cluster_range
                    .into_iter()
                    .any(|index| !final_clusters[index as usize].display_text.is_empty());
            let base_indent = if !drawable {
                0.0
            } else if candidate.cluster_range.first() == 0 {
                first_line_indent
            } else {
                block_indent
            };
            let hyphen_advance = line_hyphen_advance_at(line_index as i32);
            let limit = measure - base_indent;
            let alignment_inset = if candidate.end_reason == LineEndReason::AutoWrap {
                0.0
            } else {
                match input.paragraph_style.last_line_alignment {
                    LastLineAlignment::Start => 0.0,
                    LastLineAlignment::Center => ((limit - visual_width) / 2.0).max(0.0),
                    LastLineAlignment::End => (limit - visual_width).max(0.0),
                }
            };
            let repair = candidate.repair.as_ref().map(|repair| {
                let kind = match repair {
                    RepairOption::PushIn { .. } => "PushIn",
                    RepairOption::Hang { .. } => "Hang",
                    RepairOption::CarryPrevious { .. } => "CarryPrevious",
                    RepairOption::CarryNext { .. } => "CarryNext",
                    RepairOption::LeaveRagged { .. } => "LeaveRagged",
                };
                format!("{kind}:{}", repair.reason())
            });
            let range_note = if candidate.cluster_range.is_empty() {
                format!("line:{line_index}:clusters=empty")
            } else {
                format!(
                    "line:{line_index}:clusters={}-{}",
                    candidate.cluster_range.first(),
                    candidate.cluster_range.last()
                )
            };
            let mut notes = vec![
                range_note,
                format!("end:{:?}", candidate.end_reason),
                format!(
                    "natural={},adjusted={},visual={visual_width}",
                    candidate.natural_width, candidate.adjusted_width
                ),
            ];
            if let Some(fallback) = justification_plans
                .get(line_index)
                .and_then(Option::as_ref)
                .and_then(|plan| plan.fallback_reason.as_deref())
            {
                notes.push(format!("justify-fallback:{fallback}"));
            }
            LineBox::builder(
                candidate.source_range,
                candidate.cluster_range,
                line_baseline[line_index],
                line_top[line_index],
                line_bottom[line_index],
                candidate.natural_width,
                adjusted_width,
                visual_width,
            )
            .hanging_punctuation_advance(hanging_punctuation_advance)
            .indent(grid_body_offset + base_indent + alignment_inset)
            .end_reason(candidate.end_reason)
            .hyphen_advance(hyphen_advance)
            .hyphen_glyphs(if hyphen_advance > 0.0 {
                hyphen_glyphs.to_vec()
            } else {
                Vec::new()
            })
            .debug(LineDebugInfo::with_all(repair, notes))
            .build()
        })
        .collect();
    let visible_count = (input.constraints.max_lines() as usize).min(laid_out_lines.len());
    let visible_lines = laid_out_lines[..visible_count].to_vec();
    let max_lines_decision = (visible_lines.len() < laid_out_lines.len()).then(|| {
        MaxLinesDecisionInfo::new(laid_out_lines.len() as i32, visible_lines.len() as i32)
    });
    let visible_line_ranges = line_solution
        .lines
        .iter()
        .take(visible_lines.len())
        .map(|line| line.cluster_range)
        .collect();
    LineBoxStageResult {
        laid_out_lines,
        visible_lines,
        max_lines_decision,
        visible_line_ranges,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_line_vertical_geometry(
    input: &LayoutInput,
    font_size: f32,
    pinyin_spans: &[RubySpan],
    natural_clusters: &[Cluster],
    line_solution: &LineSolution,
    ruby_font_geometry_by_span: &HashMap<RubySpan, RubyFontGeometry>,
    existing_interline_space: f32,
    base_line_metrics: ResolvedLineMetrics,
    base_face_height: f32,
    ruby_extent: f32,
    inline_object_by_cluster_index: &HashMap<i32, InlineObjectSpan>,
    base_ascent: f32,
    base_descent: f32,
) -> LineVerticalGeometryStageResult {
    let pinyin_ranges: Vec<_> = pinyin_spans
        .iter()
        .filter_map(|ruby| {
            cluster_index_range_for(natural_clusters, ruby.base_range).map(|range| (ruby, range))
        })
        .collect();
    let per_line_ruby_extent: Vec<f32> = line_solution
        .lines
        .iter()
        .map(|line| {
            pinyin_ranges
                .iter()
                .filter_map(|(ruby, (first, last))| {
                    (*first <= line.cluster_range.last() && *last >= line.cluster_range.first())
                        .then(|| {
                            ruby_font_geometry_by_span
                                .get(*ruby)
                                .map(|geometry| geometry.required_extent)
                        })
                        .flatten()
                })
                .fold(0.0, f32::max)
        })
        .collect();
    let per_line_ruby_deficit: Vec<f32> = per_line_ruby_extent
        .iter()
        .map(|extent| (extent - existing_interline_space).max(0.0))
        .collect();
    let paragraph_ruby_deficit = per_line_ruby_deficit.iter().copied().fold(0.0, f32::max);
    let line_ruby_top_extra = match input.paragraph_style.ruby_line_height_mode {
        RubyLineHeightMode::PerLine => per_line_ruby_deficit,
        RubyLineHeightMode::UniformParagraph => {
            vec![paragraph_ruby_deficit; line_solution.lines.len()]
        }
    };
    let line_ruby_interline_demand = match input.paragraph_style.ruby_line_height_mode {
        RubyLineHeightMode::PerLine => per_line_ruby_extent.clone(),
        RubyLineHeightMode::UniformParagraph => {
            vec![
                per_line_ruby_extent.iter().copied().fold(0.0, f32::max);
                line_solution.lines.len()
            ]
        }
    };
    let ruby_line_height_decision =
        (!pinyin_spans.is_empty()).then(|| RubyLineHeightDecisionInfo {
            mode: format!("{:?}", input.paragraph_style.ruby_line_height_mode),
            base_line_height: base_line_metrics.height,
            base_face_height,
            ruby_extent,
            available_interline_space: existing_interline_space,
            max_extra: line_ruby_top_extra.iter().copied().fold(0.0, f32::max),
            line_extras: line_ruby_top_extra.clone(),
            expanded_line_indices: line_ruby_top_extra
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (*value > 0.0).then_some(index as i32))
                .collect(),
            reason: if line_ruby_top_extra.iter().any(|value| *value > 0.0) {
                "ConditionalRubyLineHeight".to_owned()
            } else {
                "ExistingInterlineSpaceFitsRuby".to_owned()
            },
        });
    let line_object_ascent: Vec<f32> = line_solution
        .lines
        .iter()
        .map(|line| {
            line.cluster_range
                .into_iter()
                .filter_map(|index| {
                    inline_object_by_cluster_index
                        .get(&index)
                        .map(|object| object.ascent)
                })
                .fold(0.0, f32::max)
        })
        .collect();
    let line_object_descent: Vec<f32> = line_solution
        .lines
        .iter()
        .map(|line| {
            line.cluster_range
                .into_iter()
                .filter_map(|index| {
                    inline_object_by_cluster_index
                        .get(&index)
                        .map(|object| object.descent)
                })
                .fold(0.0, f32::max)
        })
        .collect();
    let top_intrusion: Vec<f32> = line_object_ascent
        .iter()
        .map(|ascent| (ascent - base_ascent).max(0.0))
        .collect();
    let bottom_intrusion: Vec<f32> = line_object_descent
        .iter()
        .map(|descent| (descent - base_descent).max(0.0))
        .collect();
    let minimum_clearance = input.paragraph_style.inline_object_minimum_clearance_em * font_size;
    let base_top = base_line_metrics.baseline;
    let base_bottom = base_line_metrics.height - base_line_metrics.baseline;
    let combined_extra: Vec<f32> = (0..line_solution.lines.len())
        .map(|index| {
            if index == 0 {
                line_ruby_top_extra[index].max((line_object_ascent[index] - base_top).max(0.0))
            } else {
                let top_demand = line_ruby_interline_demand[index].max(top_intrusion[index]);
                let intrudes = bottom_intrusion[index - 1] > 0.0
                    || (top_intrusion[index] > 0.0
                        && top_intrusion[index] >= line_ruby_interline_demand[index]);
                (bottom_intrusion[index - 1]
                    + top_demand
                    + if intrudes { minimum_clearance } else { 0.0 }
                    - existing_interline_space)
                    .max(0.0)
            }
        })
        .collect();
    let object_extra: Vec<f32> = combined_extra
        .iter()
        .zip(&line_ruby_top_extra)
        .map(|(combined, ruby)| (combined - ruby).max(0.0))
        .collect();
    let mut line_baseline = vec![0.0; line_solution.lines.len()];
    if !line_baseline.is_empty() {
        line_baseline[0] = base_line_metrics.baseline + combined_extra[0];
        for index in 1..line_baseline.len() {
            line_baseline[index] =
                line_baseline[index - 1] + base_line_metrics.height + combined_extra[index];
        }
    }
    let mut line_top = vec![0.0; line_solution.lines.len()];
    let mut line_bottom = vec![0.0; line_solution.lines.len()];
    let mut boundary_shifts_after = vec![0.0; line_solution.lines.len().saturating_sub(1)];
    for index in 0..line_solution.lines.len().saturating_sub(1) {
        let boundary_extent = resolve_inline_object_line_boundary_extent(
            base_bottom,
            base_descent.max(line_object_descent[index]),
            line_baseline[index + 1] - line_baseline[index],
            base_ascent.max(line_object_ascent[index + 1]),
        );
        let nominal = line_baseline[index] + base_bottom;
        let boundary = line_baseline[index] + boundary_extent;
        line_bottom[index] = boundary;
        line_top[index + 1] = boundary;
        boundary_shifts_after[index] = boundary - nominal;
    }
    let trailing_extra = line_object_descent
        .last()
        .map_or(0.0, |descent| (descent - base_bottom).max(0.0));
    if let Some(last) = line_bottom.len().checked_sub(1) {
        line_bottom[last] = line_baseline[last] + base_bottom + trailing_extra;
    }
    let inline_object_line_height_decision =
        (!inline_object_by_cluster_index.is_empty()).then(|| {
            let expanded: Vec<_> = object_extra
                .iter()
                .enumerate()
                .filter_map(|(index, value)| (*value > 0.0).then_some(index as i32))
                .collect();
            InlineObjectLineHeightDecisionInfo {
                base_line_height: base_line_metrics.height,
                base_face_ascent: base_ascent,
                base_face_descent: base_descent,
                available_interline_space: existing_interline_space,
                minimum_clearance,
                line_ascents: line_object_ascent,
                line_descents: line_object_descent,
                line_extras: object_extra,
                boundary_shifts_after,
                trailing_extra,
                expanded_line_indices: expanded.clone(),
                reason: if expanded.is_empty() && trailing_extra == 0.0 {
                    "ExistingInterlineSpaceFitsInlineObjects".to_owned()
                } else {
                    "InlineObjectInterlineCollision".to_owned()
                },
            }
        });
    LineVerticalGeometryStageResult {
        ruby_line_height_decision,
        inline_object_line_height_decision,
        line_baseline,
        line_top,
        line_bottom,
    }
}

pub fn line_metrics(
    decisions: &[ClusterMetricDecision],
    explicit_line_height: Option<f32>,
    default_line_height: f32,
    spacing_floor: f32,
) -> ResolvedLineMetrics {
    if decisions.is_empty() {
        let height = explicit_line_height.unwrap_or(default_line_height);
        return ResolvedLineMetrics {
            baseline: height * EMPTY_PARAGRAPH_BASELINE_RATIO,
            height,
            extra_leading: 0.0,
        };
    }
    let ideographic: Vec<_> = decisions
        .iter()
        .filter(|decision| decision.layout_metrics.metric_box == MetricBox::IdeographicEmBox)
        .collect();
    let source: Vec<_> = if ideographic.is_empty() {
        decisions.iter().collect()
    } else {
        ideographic
    };
    let ascent = source
        .iter()
        .map(|decision| decision.layout_metrics.ascent)
        .fold(f32::NEG_INFINITY, f32::max);
    let descent = source
        .iter()
        .map(|decision| decision.layout_metrics.descent)
        .fold(f32::NEG_INFINITY, f32::max);
    let natural_height = ascent + descent;
    let height = explicit_line_height
        .unwrap_or(default_line_height)
        .max(natural_height + spacing_floor);
    let extra_leading = (height - natural_height).max(0.0);
    ResolvedLineMetrics {
        baseline: extra_leading / 2.0 + ascent,
        height,
        extra_leading,
    }
}

pub fn renderable_glyph_run_clusters(
    clusters: &[Cluster],
    open_type_features_by_cluster_range: &HashMap<
        super::super::core::Geometry::TextRange,
        Vec<String>,
    >,
) -> Vec<Vec<Cluster>> {
    let mut groups: Vec<Vec<Cluster>> = Vec::new();
    for cluster in clusters
        .iter()
        .filter(|cluster| !cluster.display_text.is_empty() && !is_inline_object_cluster(cluster))
    {
        if let Some(current) = groups.last_mut()
            && current.last().is_some_and(|previous| {
                previous.font_key == cluster.font_key
                    && previous.range.end() == cluster.range.start()
                    && open_type_features_by_cluster_range
                        .get(&previous.range)
                        .map(Vec::as_slice)
                        .unwrap_or(&[])
                        == open_type_features_by_cluster_range
                            .get(&cluster.range)
                            .map(Vec::as_slice)
                            .unwrap_or(&[])
            })
        {
            current.push(cluster.clone());
        } else {
            groups.push(vec![cluster.clone()]);
        }
    }
    groups
}

pub fn resolve_inline_object_line_boundary_extent(
    nominal_boundary_extent: f32,
    current_content_bottom_extent: f32,
    baseline_distance: f32,
    next_content_top_extent: f32,
) -> f32 {
    nominal_boundary_extent.clamp(
        current_content_bottom_extent,
        current_content_bottom_extent.max(baseline_distance - next_content_top_extent),
    )
}
