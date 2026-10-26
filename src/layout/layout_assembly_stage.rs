// 结果装配阶段：把行调整阶段的产出装配成 `LayoutResult`。
//
// 本文件只做装配，不做调整决策。行调整阶段给出最终的 cluster、注音与装饰几何，
// 这里把它们整理进 `LayoutOutput`（运行时数据）与 `LayoutResult.debug`
// （诊断记录，由 `debug-info` feature 控制是否编译）。

use crate::common::HashMap;
use alloc::vec::Vec;

use super::super::core::geometry::Size;
#[cfg(feature = "debug-info")]
use super::super::core::layout_model::LayoutDebugInfo;
use super::super::core::layout_model::{
    AnnotationGeometry, Cluster, ClusterGeometryDecisionInfo, DecorationGeometry,
    EmphasisDotGeometry, InlineObjectGeometry, LayoutOutput, LayoutResult,
    LineEdgeTrimDecisionInfo, LineWhiteSpaceDecisionInfo, ResolvedClusterGeometry,
    ResolvedFontMetrics, ResolvedFontRole, WhiteSpaceGeometry,
};
use super::super::shaping::font_backend::FontBackend;
use super::annotation_geometry_stage::{
    AnnotationGeometryRequest, AnnotationGeometryStageResult, resolve_annotation_geometry,
};
#[cfg(feature = "debug-info")]
use super::layout_debug_assembly::{LayoutDebugStageInput, build_layout_debug_info};
use super::line_break_planning_stage::{LineBreakPlanningStageResult, ParagraphLayoutPrep};
use super::line_geometry_stage::{
    build_glyph_runs, build_line_boxes, resolve_line_vertical_geometry,
};
use super::line_optimization::LineSolution;
use super::white_space::LineWhiteSpaceAction;

/// 行调整阶段交给装配阶段的全部产出。
pub struct LayoutAssemblyInput {
    /// 裁边后、两端对齐前的 cluster。
    pub trimmed_clusters: Vec<Cluster>,
    /// 两端对齐后的最终 cluster。
    pub final_clusters: Vec<Cluster>,
    /// 两端对齐的 cluster 增量，注音几何需要。
    pub justify_delta_by_cluster: HashMap<i32, f32>,
    /// 行边缘 trim 决策，同时进入运行时数据与诊断记录。
    pub edge_trim_decisions: Vec<LineEdgeTrimDecisionInfo>,
    /// 最终几何，`resolved_advance` 已按 `final_clusters` 校正。
    pub resolved_clusters: Vec<ResolvedClusterGeometry>,
    /// 几何决策，`resolved_advance` 已按 `final_clusters` 校正。
    pub geometry_decisions: Vec<ClusterGeometryDecisionInfo>,
}

/// 装配一个段落结果。
pub struct LayoutAssemblyRequest<'a> {
    pub prep: &'a ParagraphLayoutPrep,
    pub plan: &'a LineBreakPlanningStageResult,
    pub line_solution: &'a LineSolution,
    pub input: LayoutAssemblyInput,
    /// 行高计算所需的亚像素行位置，按行索引取值。
    pub line_hyphen_advance_at: &'a dyn Fn(usize) -> f32,
    pub font_backend: &'a dyn FontBackend,
    /// 诊断装配中与行调整内部状态无关的输入。
    #[cfg(feature = "debug-info")]
    pub debug_stage: LayoutDebugStageInput<'a>,
}

/// 装配段落结果。
///
/// `debug-info` 关闭时 `debug` 字段不存在，装配过程也不会触碰诊断数据。
pub fn assemble_paragraph_layout(request: LayoutAssemblyRequest<'_>) -> LayoutResult {
    let LayoutAssemblyRequest {
        prep,
        plan,
        line_solution,
        input,
        line_hyphen_advance_at,
        font_backend,
        #[cfg(feature = "debug-info")]
        debug_stage,
    } = request;
    let LayoutAssemblyInput {
        trimmed_clusters,
        final_clusters,
        justify_delta_by_cluster,
        edge_trim_decisions,
        resolved_clusters,
        geometry_decisions,
    } = input;
    let glyph_runs = build_glyph_runs(
        &final_clusters,
        &prep.open_type_features_by_cluster_range,
        &prep.shaped_glyphs_by_cluster_range,
        &prep.atom_class_by_range,
        &prep.font_resolutions,
    );
    let vertical_geometry = resolve_line_vertical_geometry(
        &prep.input,
        prep.font_size,
        &prep.pinyin_spans,
        &prep.natural_clusters,
        line_solution,
        &prep.ruby_font_geometry_by_span,
        plan.existing_interline_space,
        plan.base_line_metrics,
        plan.base_face_height,
        plan.ruby_extent,
        &prep.inline_object_by_cluster_index,
        plan.base_ascent,
        plan.base_descent,
    );
    let line_boxes = build_line_boxes(
        &prep.input,
        line_solution,
        &trimmed_clusters,
        &final_clusters,
        plan.first_line_indent,
        plan.block_indent,
        prep.measure,
        prep.grid_body_offset,
        &vertical_geometry.line_baseline,
        &vertical_geometry.line_top,
        &vertical_geometry.line_bottom,
        &|index| line_hyphen_advance_at(index as usize),
        &prep.hyphen_glyphs,
    );
    let annotation = resolve_annotation_geometry(AnnotationGeometryRequest {
        input: &prep.input,
        font_size: prep.font_size,
        inline_object_by_cluster_index: &prep.inline_object_by_cluster_index,
        line_solution,
        clreq_profile: &prep.clreq_profile,
        geometry_decisions: &geometry_decisions,
        auto_space_decisions: &prep.auto_space_decisions,
        visible_line_ranges: &line_boxes.visible_line_ranges,
        lines: &line_boxes.visible_lines,
        final_clusters: &final_clusters,
        cluster_roles: &prep.cluster_roles,
        justify_delta_by_cluster: &justify_delta_by_cluster,
        ruby_and_bopomofo_spread: &prep.ruby_and_bopomofo_spread,
        metric_decisions: &plan.metric_decisions,
        pinyin_spans: &prep.pinyin_spans,
        natural_clusters: &prep.natural_clusters,
        ruby_font_geometry_by_span: &prep.ruby_font_geometry_by_span,
        ruby_stack_gap: prep.ruby_stack_gap,
        base_ascent: plan.base_ascent,
        ruby_font_size: prep.ruby_font_size,
        ruby_font_weight: prep.ruby_font_weight,
        base_descent: plan.base_descent,
        bopomofo_font_weight_at: prep.bopomofo_font_weight_at.as_ref(),
        font_backend,
    });
    let widest_line = line_boxes
        .visible_lines
        .iter()
        .map(|line| line.indent + line.visual_width + line.hyphen_advance)
        .fold(0.0, f32::max);
    let total_height = line_boxes
        .visible_lines
        .last()
        .map(|line| line.bottom)
        .unwrap_or_else(|| {
            if prep.text.is_empty() {
                0.0
            } else {
                plan.base_line_metrics.height
            }
        });
    let size = Size {
        width: widest_line.min(prep.input.constraints.max_width()),
        height: total_height,
    };
    let layout = build_layout_output(
        prep,
        plan,
        line_solution,
        &final_clusters,
        edge_trim_decisions,
        resolved_clusters,
        &annotation,
    );
    #[cfg(feature = "debug-info")]
    let debug = build_paragraph_debug(
        debug_stage,
        prep,
        line_solution,
        &final_clusters,
        &annotation,
        &line_boxes,
        &vertical_geometry,
        geometry_decisions,
        &layout.line_edge_trims,
    );
    let lines = line_boxes.visible_lines;
    build_layout_result(
        prep.input.clone(),
        size,
        final_clusters,
        glyph_runs,
        lines,
        layout,
        #[cfg(feature = "debug-info")]
        debug,
    )
}

/// 装配运行时数据。
///
/// 每个分组都从上游决策直接产出，不从诊断记录复制。
fn build_layout_output(
    prep: &ParagraphLayoutPrep,
    plan: &LineBreakPlanningStageResult,
    line_solution: &LineSolution,
    final_clusters: &[Cluster],
    edge_trim_decisions: Vec<LineEdgeTrimDecisionInfo>,
    resolved_clusters: Vec<ResolvedClusterGeometry>,
    annotation: &AnnotationGeometryStageResult,
) -> LayoutOutput {
    let mut resolutions: Vec<_> = prep.font_resolutions.values().collect();
    resolutions.sort_by_key(|resolution| (resolution.range.start(), resolution.range.end()));
    LayoutOutput {
        font_roles: resolutions
            .into_iter()
            .filter_map(|resolution| {
                // 与诊断记录保持一致：缺少选定候选证据的 resolution 不产出记录。
                resolution.selected_attempt()?;
                Some(ResolvedFontRole {
                    range: resolution.range,
                    role: resolution.role,
                    face: resolution.face.clone(),
                })
            })
            .collect(),
        font_metrics: plan
            .metric_decisions
            .iter()
            .map(|decision| ResolvedFontMetrics {
                range: decision.range,
                face: decision.request.face.clone(),
                role: decision.request.role,
                ascent: decision.layout_metrics.ascent,
                descent: decision.layout_metrics.descent,
                baseline_class: decision.layout_metrics.baseline_class,
                metric_box: decision.layout_metrics.metric_box,
                source: decision.layout_metrics.source,
            })
            .collect(),
        clusters: resolved_clusters,
        punctuation: prep.punctuation_atoms.to_vec(),
        spacing: prep.spacing_plan.adjustments.clone(),
        line_edge_trims: edge_trim_decisions,
        annotations: AnnotationGeometry {
            ruby: annotation.ruby_decisions.clone(),
            bopomofo: annotation.bopomofo_decisions.clone(),
        },
        decorations: DecorationGeometry {
            emphasis_dots: annotation
                .decoration_decisions
                .iter()
                .map(|decision| EmphasisDotGeometry {
                    cluster_range: decision.cluster_range,
                    kind: decision.kind,
                    id: decision.id,
                    applied: decision.applied,
                    anchor_x: decision.anchor_x,
                    anchor_y: decision.anchor_y,
                    dot_diameter: decision.dot_diameter,
                })
                .collect(),
            segments: annotation.decoration_segments.clone(),
        },
        inline_objects: annotation
            .inline_object_decisions
            .iter()
            .map(|decision| InlineObjectGeometry {
                range: decision.range,
                cluster_index: decision.cluster_index,
                line_index: decision.line_index,
                advance: decision.advance,
                ascent: decision.ascent,
                descent: decision.descent,
            })
            .collect(),
        white_space: WhiteSpaceGeometry {
            units: prep
                .natural_clusters
                .iter()
                .filter_map(|cluster| prep.white_space_units.get(&cluster.range).copied())
                .collect(),
            line_adjustments: resolve_line_white_space(&line_solution.lines, final_clusters),
            auto_space: prep.auto_space_decisions.to_vec(),
        },
    }
}

/// 逐行空白几何。运行时数据与诊断记录共用这一份计算。
pub fn resolve_line_white_space(
    lines: &[super::line_optimization::LineCandidate],
    final_clusters: &[Cluster],
) -> Vec<LineWhiteSpaceDecisionInfo> {
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        for adjustment in &line.white_space_adjustments {
            let mut adjustment = *adjustment;
            if adjustment.action == LineWhiteSpaceAction::ResolveTab {
                adjustment.occupied_advance =
                    final_clusters[adjustment.cluster_index as usize].advance;
            }
            if matches!(
                adjustment.action,
                LineWhiteSpaceAction::HangEnd | LineWhiteSpaceAction::ConditionallyHangEnd
            ) {
                continue;
            }
            out.push(LineWhiteSpaceDecisionInfo {
                line_index: index as i32,
                line_range: line.source_range,
                adjustment,
            });
        }
        if let Some(measurement) = &line.measurement {
            for adjustment in measurement.final_hanging_adjustments(
                line.cluster_range,
                line.end_reason,
                line.measurement_limit,
                final_clusters,
            ) {
                out.push(LineWhiteSpaceDecisionInfo {
                    line_index: index as i32,
                    line_range: line.source_range,
                    adjustment,
                });
            }
        }
    }
    out
}

/// 装配诊断记录。仅在 `debug-info` 开启时编译。
///
/// 所有引用共用同一个生命周期，回填的字段都与 `stage` 同寿命。
#[cfg(feature = "debug-info")]
fn build_paragraph_debug<'a>(
    mut stage: LayoutDebugStageInput<'a>,
    prep: &'a ParagraphLayoutPrep,
    line_solution: &'a LineSolution,
    final_clusters: &[Cluster],
    annotation: &'a AnnotationGeometryStageResult,
    line_boxes: &'a super::line_geometry_stage::LineBoxStageResult,
    vertical_geometry: &super::line_geometry_stage::LineVerticalGeometryStageResult,
    geometry_decisions: Vec<ClusterGeometryDecisionInfo>,
    edge_trim_decisions: &[LineEdgeTrimDecisionInfo],
) -> LayoutDebugInfo {
    stage.geometry_decisions = geometry_decisions;
    stage.edge_trim_decisions = edge_trim_decisions.to_vec();
    stage.laid_out_lines = &line_boxes.laid_out_lines;
    stage.max_lines_decision = line_boxes.max_lines_decision.clone();
    stage.decoration_decisions = annotation.decoration_decisions.clone();
    stage.decoration_segments = annotation.decoration_segments.clone();
    stage.ruby_decisions = annotation.ruby_decisions.clone();
    stage.bopomofo_decisions = annotation.bopomofo_decisions.clone();
    stage.inline_object_decisions = &annotation.inline_object_decisions;
    stage.ruby_line_height_decision = vertical_geometry.ruby_line_height_decision.clone();
    stage.inline_object_line_height_decision =
        vertical_geometry.inline_object_line_height_decision.clone();
    let mut debug = build_layout_debug_info(stage);
    debug.white_space_units = prep
        .natural_clusters
        .iter()
        .filter_map(|cluster| prep.white_space_units.get(&cluster.range).copied())
        .collect();
    debug.line_white_space_adjustments =
        resolve_line_white_space(&line_solution.lines, final_clusters);
    debug
}

/// 按 feature 决定结果是否携带诊断记录。
#[cfg(feature = "debug-info")]
fn build_layout_result(
    input: super::super::core::text_model::LayoutInput,
    size: Size,
    clusters: Vec<Cluster>,
    glyph_runs: Vec<super::super::core::layout_model::GlyphRun>,
    lines: Vec<super::super::core::layout_model::LineBox>,
    layout: LayoutOutput,
    debug: LayoutDebugInfo,
) -> LayoutResult {
    LayoutResult::with_layout_and_debug(input, size, clusters, glyph_runs, lines, layout, debug)
}

#[cfg(not(feature = "debug-info"))]
fn build_layout_result(
    input: super::super::core::text_model::LayoutInput,
    size: Size,
    clusters: Vec<Cluster>,
    glyph_runs: Vec<super::super::core::layout_model::GlyphRun>,
    lines: Vec<super::super::core::layout_model::LineBox>,
    layout: LayoutOutput,
) -> LayoutResult {
    LayoutResult::with_layout(input, size, clusters, glyph_runs, lines, layout)
}
