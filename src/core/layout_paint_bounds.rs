use crate::common::HashMap;

use super::layout_model::{LayoutResult, LineBox};
use super::layout_queries::{PositionedCluster, positioned_clusters};

/// Paint overhang belonging to already-visible occupied text geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutPaintOverhang {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Returns the right clip edge authorized by an engine-selected punctuation hang.
pub fn legal_hanging_punctuation_clip_edge(line: &LineBox, viewport_width: f32) -> f32 {
    if line.hanging_punctuation_advance > 0.0 {
        line.indent + line.visual_width
    } else {
        viewport_width
    }
}

/// Returns paint overhang for glyph ink and annotations attached to occupied geometry visible in a viewport.
pub fn visible_paint_overhang(
    result: &LayoutResult,
    viewport_width: f32,
    viewport_height: f32,
    positioned: Option<&[PositionedCluster]>,
) -> LayoutPaintOverhang {
    let owned_positions;
    let positioned = match positioned {
        Some(value) => value,
        None => {
            owned_positions = positioned_clusters(result);
            &owned_positions
        }
    };
    let positions_by_range: HashMap<_, _> = positioned
        .iter()
        .map(|cluster| (cluster.range, cluster))
        .collect();
    let mut overhang = LayoutPaintOverhang::default();

    let mut include_paint = |cluster: &PositionedCluster,
                             paint_left: f32,
                             paint_top: f32,
                             paint_right: f32,
                             paint_bottom: f32| {
        if cluster.right <= 0.0
            || cluster.left >= viewport_width
            || cluster.bottom <= 0.0
            || cluster.top >= viewport_height
        {
            return;
        }
        overhang.left = overhang.left.max(cluster.left - paint_left);
        overhang.top = overhang.top.max(cluster.top - paint_top);
        overhang.right = overhang.right.max(paint_right - cluster.right);
        overhang.bottom = overhang.bottom.max(paint_bottom - cluster.bottom);
    };

    for glyph in result.glyph_runs.iter().flat_map(|run| &run.glyphs) {
        let Some(bounds) = glyph.bounds else {
            continue;
        };
        let Some(cluster) = positions_by_range.get(&glyph.cluster_range) else {
            continue;
        };
        include_paint(
            cluster,
            cluster.draw_x + glyph.x + bounds.left,
            cluster.baseline + glyph.y + bounds.top,
            cluster.draw_x + glyph.x + bounds.right,
            cluster.baseline + glyph.y + bounds.bottom,
        );
    }
    for dot in result
        .debug
        .decoration_decisions
        .iter()
        .filter(|dot| dot.applied && dot.dot_diameter > 0.0)
    {
        let Some(cluster) = positions_by_range.get(&dot.cluster_range) else {
            continue;
        };
        let radius = dot.dot_diameter / 2.0;
        include_paint(
            cluster,
            dot.anchor_x - radius,
            dot.anchor_y - radius,
            dot.anchor_x + radius,
            dot.anchor_y + radius,
        );
    }
    for ruby in &result.debug.ruby_decisions {
        let base: Vec<_> = positioned
            .iter()
            .filter(|cluster| {
                cluster.line_index == ruby.line_index
                    && cluster.range.start() >= ruby.base_range.start()
                    && cluster.range.end() <= ruby.base_range.end()
            })
            .collect();
        let Some(occupied_left) = base.iter().map(|cluster| cluster.left).reduce(f32::min) else {
            continue;
        };
        let occupied_top = base.iter().map(|cluster| cluster.top).reduce(f32::min).unwrap();
        let occupied_right = base.iter().map(|cluster| cluster.right).reduce(f32::max).unwrap();
        let occupied_bottom = base.iter().map(|cluster| cluster.bottom).reduce(f32::max).unwrap();
        if occupied_right <= 0.0
            || occupied_left >= viewport_width
            || occupied_bottom <= 0.0
            || occupied_top >= viewport_height
        {
            continue;
        }
        let paint_left = ruby.center_x - ruby.width / 2.0;
        let paint_top = ruby.baseline_y - ruby.ascent;
        let paint_right = ruby.center_x + ruby.width / 2.0;
        let paint_bottom = ruby.baseline_y + ruby.descent;
        overhang.left = overhang.left.max(occupied_left - paint_left);
        overhang.top = overhang.top.max(occupied_top - paint_top);
        overhang.right = overhang.right.max(paint_right - occupied_right);
        overhang.bottom = overhang.bottom.max(paint_bottom - occupied_bottom);
    }
    overhang
}