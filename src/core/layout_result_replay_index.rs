use crate::common::HashMap;

use super::geometry::{Rect, ScalarOffset, TextRange};
use super::layout_model::{Glyph, LayoutResult};
use super::layout_queries::{
    PositionedCluster, RichTextLineSegment, coerce_selection_offset, get_line_for_offset,
    get_selection_word_boundary, nearest_line_for_position, offset_for_x, positioned_clusters,
    x_for_offset,
};
use super::source_interaction_boundaries::SourceBoundaryBias;

/// Immutable lookup data derived exclusively from one [`LayoutResult`].
#[derive(Clone, Debug)]
pub struct LayoutResultReplayIndex {
    pub positioned_clusters: Vec<PositionedCluster>,
    pub positioned_clusters_by_line: Vec<Vec<PositionedCluster>>,
    pub rich_text_segments: Vec<RichTextLineSegment>,
    pub rich_text_background_segments: Vec<RichTextLineSegment>,
    pub rich_text_decoration_segments: Vec<RichTextLineSegment>,
    pub glyphs_by_cluster_range: HashMap<TextRange, Vec<Glyph>>,
    pub open_type_features_by_cluster_range: HashMap<TextRange, Vec<String>>,
    pub font_role_by_cluster_range: HashMap<TextRange, Option<String>>,
}

/// Builds immutable replay lookup data from one final layout result.
pub fn to_replay_index(result: &LayoutResult) -> LayoutResultReplayIndex {
    let positioned_clusters = positioned_clusters(result);
    let mut positioned_clusters_by_line = vec![Vec::new(); result.lines.len()];
    for positioned in &positioned_clusters {
        if let Some(line) = positioned_clusters_by_line.get_mut(positioned.line_index as usize) {
            line.push(positioned.clone());
        }
    }
    let rich_text_segments = result.positioned_rich_text_segments_from_clusters(&positioned_clusters);
    let rich_text_background_segments = result.rich_text_background_segments_from_occupied(
        &rich_text_segments, &positioned_clusters,
    );
    let rich_text_decoration_segments = result.rich_text_decoration_segments_from_occupied(&rich_text_segments);
    let mut glyphs_by_cluster_range: HashMap<TextRange, Vec<Glyph>> = HashMap::new();
    for glyph in result.glyph_runs.iter().flat_map(|run| &run.glyphs) {
        glyphs_by_cluster_range
            .entry(glyph.cluster_range)
            .or_default()
            .push(glyph.clone());
    }
    let mut open_type_features_by_cluster_range = HashMap::new();
    let mut run_index = 0;
    for cluster in &result.clusters {
        while result
            .glyph_runs
            .get(run_index)
            .is_some_and(|run| run.range.end() <= cluster.range.start())
        {
            run_index += 1;
        }
        let features = result
            .glyph_runs
            .get(run_index)
            .filter(|run| cluster.range.start() >= run.range.start() && cluster.range.end() <= run.range.end())
            .map(|run| run.open_type_features.clone())
            .unwrap_or_default();
        open_type_features_by_cluster_range.insert(cluster.range, features);
    }
    let mut font_role_by_cluster_range = HashMap::new();
    let mut decision_index = 0;
    for positioned in &positioned_clusters {
        let range = result.clusters[positioned.cluster_index as usize].range;
        while result
            .debug
            .font_decisions
            .get(decision_index)
            .is_some_and(|decision| decision.range.end() <= range.start())
        {
            decision_index += 1;
        }
        let role = result
            .debug
            .font_decisions
            .get(decision_index)
            .filter(|decision| range.start() >= decision.range.start() && range.end() <= decision.range.end())
            .map(|decision| decision.role.clone());
        font_role_by_cluster_range.insert(range, role);
    }
    LayoutResultReplayIndex {
        positioned_clusters,
        positioned_clusters_by_line,
        rich_text_segments,
        rich_text_background_segments,
        rich_text_decoration_segments,
        glyphs_by_cluster_range,
        open_type_features_by_cluster_range,
        font_role_by_cluster_range,
    }
}

/// Hit-tests a selection endpoint over immutable replay geometry.
pub fn selection_offset_for_position(
    index: &LayoutResultReplayIndex,
    result: &LayoutResult,
    x: f32,
    y: f32,
) -> ScalarOffset {
    if result.lines.is_empty() {
        return ScalarOffset::ZERO;
    }
    let line_index = nearest_line_for_position(result, y);
    let positioned = index
        .positioned_clusters_by_line
        .get(line_index)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if positioned.is_empty() {
        return coerce_selection_offset(
            result,
            result.lines[line_index].range.start(),
            SourceBoundaryBias::Nearest,
        );
    }
    if x <= positioned[0].left {
        return coerce_selection_offset(result, positioned[0].range.start(), SourceBoundaryBias::Nearest);
    }
    if x >= positioned[positioned.len() - 1].right {
        return coerce_selection_offset(
            result,
            positioned[positioned.len() - 1].range.end(),
            SourceBoundaryBias::Nearest,
        );
    }
    let cluster = positioned
        .iter()
        .find(|cluster| x >= cluster.left && x <= cluster.right)
        .unwrap_or_else(|| nearest_cluster_for_x(positioned, x));
    let raw_offset = offset_for_x(cluster, x);
    let backward = coerce_selection_offset(result, raw_offset, SourceBoundaryBias::Backward);
    let forward = coerce_selection_offset(result, raw_offset, SourceBoundaryBias::Forward);
    if backward == forward {
        return backward;
    }
    let backward_distance = (cursor_rect(index, result, backward).left - x).abs();
    let forward_distance = (cursor_rect(index, result, forward).left - x).abs();
    if backward_distance < forward_distance {
        backward
    } else {
        forward
    }
}

/// Returns the selection word/source unit under a point over immutable replay geometry.
pub fn selection_word_range_for_position(
    index: &LayoutResultReplayIndex,
    result: &LayoutResult,
    x: f32,
    y: f32,
) -> Option<TextRange> {
    if result.lines.is_empty() || result.input.content.text.is_empty() {
        return None;
    }
    let positioned = index
        .positioned_clusters_by_line
        .get(nearest_line_for_position(result, y))
        .map(Vec::as_slice)
        .unwrap_or_default();
    if positioned.is_empty() {
        return None;
    }
    let cluster = positioned
        .iter()
        .find(|cluster| x >= cluster.left && x <= cluster.right)
        .unwrap_or_else(|| nearest_cluster_for_x(positioned, x));
    if cluster.range.is_empty() {
        return None;
    }
    let offset = offset_for_x(cluster, x).clamp(cluster.range.start(), cluster.range.end() - 1);
    Some(get_selection_word_boundary(result, offset))
}

/// Returns a caret rectangle using immutable replay geometry.
pub fn cursor_rect(
    index: &LayoutResultReplayIndex,
    result: &LayoutResult,
    offset: ScalarOffset,
) -> Rect {
    if result.lines.is_empty() {
        return Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };
    }
    let clamped = offset.min(result.input.content.text.scalar_len());
    let line_index = get_line_for_offset(result, clamped).max(0) as usize;
    let line = &result.lines[line_index];
    let positioned = index
        .positioned_clusters_by_line
        .get(line_index)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let x = if positioned.is_empty() {
        line.indent
    } else if clamped <= positioned[0].range.start() {
        positioned[0].left
    } else if clamped >= positioned[positioned.len() - 1].range.end() {
        positioned[positioned.len() - 1].right
    } else {
        x_for_offset(
            positioned
                .iter()
                .find(|cluster| clamped >= cluster.range.start() && clamped <= cluster.range.end())
                .expect("source offset must belong to a positioned cluster"),
            clamped,
        )
    };
    Rect { left: x, top: line.top, right: x + 1.0, bottom: line.bottom }
}

/// Returns one continuous selection rectangle per visible line using immutable replay geometry.
pub fn selection_boxes(
    index: &LayoutResultReplayIndex,
    result: &LayoutResult,
    range: TextRange,
) -> Vec<Rect> {
    if range.is_empty() || result.lines.is_empty() {
        return Vec::new();
    }
    let text_length = result.input.content.text.scalar_len();
    let start = range.start().min(text_length);
    let end = range.end().min(text_length).max(start);
    if start == end {
        return Vec::new();
    }
    let first_line = get_line_for_offset(result, start).clamp(0, result.lines.len() as i32 - 1) as usize;
    let last_line = get_line_for_offset(result, end).clamp(first_line as i32, result.lines.len() as i32 - 1) as usize;
    let mut boxes = Vec::new();
    for line_index in first_line..=last_line {
        let positioned = index
            .positioned_clusters_by_line
            .get(line_index)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if positioned.is_empty() {
            continue;
        }
        let line_start = start.max(positioned[0].range.start());
        let line_end = end.min(positioned[positioned.len() - 1].range.end());
        if line_start >= line_end {
            continue;
        }
        let left = if line_start > positioned[0].range.start() {
            cursor_rect(index, result, line_start).left
        } else {
            positioned[0].left
        };
        let right = if line_end < positioned[positioned.len() - 1].range.end() {
            cursor_rect(index, result, line_end).left
        } else {
            positioned[positioned.len() - 1].right
        };
        if right > left {
            let line = &result.lines[line_index];
            boxes.push(Rect { left, top: line.top, right, bottom: line.bottom });
        }
    }
    boxes
}

fn nearest_cluster_for_x<'a>(positioned: &'a [PositionedCluster], x: f32) -> &'a PositionedCluster {
    positioned
        .iter()
        .min_by(|left, right| {
            (x - left.left)
                .abs()
                .min((x - left.right).abs())
                .total_cmp(&(x - right.left).abs().min((x - right.right).abs()))
        })
        .expect("positioned clusters must be non-empty")
}