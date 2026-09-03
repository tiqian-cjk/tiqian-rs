use tiqian::common::HashMap;

use tiqian::core::geometry::{ScalarOffset, TextRange};
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::layout_queries::{
    positioned_clusters, positioned_rich_text_segments, resolved_background_corner_radii,
    rich_text_background_segments, rich_text_decoration_line_y,
    trimmed_rich_text_decoration_segments,
};
use tiqian::core::text_model::{
    RichTextBackgroundDrawStyle, RichTextLinePattern, RichTextRole, RichTextSpan, TextSpan,
    TextStyle,
};
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Cap, Circle, Shape, Stroke};
use vello::peniko::Fill;
use vello::peniko::color::{AlphaColor, Srgb};

use crate::font_backend::DemoFontCatalog;

const WAVE_HALF_LENGTH_EM: f32 = 0.2;
const WAVE_AMPLITUDE_EM: f32 = 0.06;

fn default_text_color() -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(30, 30, 35, 255)
}

#[derive(Clone, Copy)]
pub struct DemoColorSpan {
    pub range: TextRange,
    pub color: AlphaColor<Srgb>,
}

pub struct DemoRenderer<'a> {
    catalog: &'a DemoFontCatalog,
    physical_scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl<'a> DemoRenderer<'a> {
    pub fn new(catalog: &'a DemoFontCatalog, physical_scale: f32) -> Self {
        Self {
            catalog,
            physical_scale,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    pub fn translated(&self, offset_x: f32, offset_y: f32) -> Self {
        Self {
            catalog: self.catalog,
            physical_scale: self.physical_scale,
            offset_x: self.offset_x + offset_x,
            offset_y: self.offset_y + offset_y,
        }
    }

    fn transform(&self) -> Affine {
        Affine::translate((self.offset_x as f64, self.offset_y as f64))
    }

    /// Replays the final body glyphs using only LayoutResult placements and shaped glyph evidence.
    pub fn paint_body(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        colors: &[DemoColorSpan],
    ) -> Result<(), String> {
        let positions: HashMap<_, _> = positioned_clusters(result)
            .into_iter()
            .map(|position| (position.range, position))
            .collect();
        for run in &result.glyph_runs {
            for glyph in &run.glyphs {
                let position = positions.get(&glyph.cluster_range).ok_or_else(|| {
                    format!(
                        "LayoutResult has no positioned cluster for glyph source range {:?}",
                        glyph.cluster_range
                    )
                })?;
                let font_size = text_style_at(
                    &result.input.content.spans,
                    &result.input.text_style,
                    glyph.cluster_range.start(),
                )
                .font_size;
                let color =
                    color_at(colors, glyph.cluster_range).unwrap_or_else(default_text_color);
                self.catalog.paint_glyph(
                    scene,
                    self.transform(),
                    glyph.render_font_key.as_deref().ok_or_else(|| {
                        format!(
                            "glyph {:?} has no render font identity",
                            glyph.cluster_range
                        )
                    })?,
                    glyph.id,
                    font_size,
                    position.draw_x + glyph.x,
                    position.baseline + glyph.y,
                    color,
                )?;
            }
        }
        for line in &result.lines {
            for glyph in &line.hyphen_glyphs {
                let color = color_at(
                    colors,
                    result.clusters[line.cluster_range.last() as usize].range,
                )
                .unwrap_or_else(default_text_color);
                self.catalog.paint_glyph(
                    scene,
                    self.transform(),
                    glyph.render_font_key.as_deref().ok_or_else(|| {
                        format!(
                            "line-end hyphen {:?} has no render font identity",
                            glyph.cluster_range
                        )
                    })?,
                    glyph.id,
                    result.input.text_style.font_size,
                    line.indent + line.visual_width + glyph.x,
                    line.baseline + glyph.y,
                    color,
                )?;
            }
        }
        Ok(())
    }

    /// Replays annotation glyphs from the final coordinates recorded by the layout engine.
    pub fn paint_annotations(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
    ) -> Result<(), String> {
        for ruby in &result.debug.ruby_decisions {
            let origin_x = ruby.center_x - ruby.width / 2.0;
            let mut cluster_pen_x = 0.0;
            let mut cluster_advance = 0.0;
            let mut previous_range = None;
            for glyph in &ruby.glyphs {
                if previous_range.is_some_and(|range| range != glyph.cluster_range) {
                    cluster_pen_x += cluster_advance;
                    cluster_advance = 0.0;
                }
                self.paint_annotation_glyph(
                    scene,
                    glyph,
                    ruby.font_size,
                    origin_x + cluster_pen_x + glyph.x,
                    ruby.baseline_y + glyph.y,
                )?;
                cluster_advance += glyph.advance;
                previous_range = Some(glyph.cluster_range);
            }
        }
        for bopomofo in &result.debug.bopomofo_decisions {
            for placement in &bopomofo.placements {
                for glyph in &placement.glyphs {
                    self.paint_annotation_glyph(
                        scene,
                        glyph,
                        placement.font_size,
                        placement.draw_x + glyph.x,
                        placement.baseline_y + glyph.y,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn paint_annotation_glyph(
        &self,
        scene: &mut Scene,
        glyph: &tiqian::core::layout_model::Glyph,
        font_size: f32,
        origin_x: f32,
        origin_y: f32,
    ) -> Result<(), String> {
        self.catalog.paint_glyph(
            scene,
            self.transform(),
            glyph.render_font_key.as_deref().ok_or_else(|| {
                format!(
                    "annotation glyph {:?} has no render font identity",
                    glyph.cluster_range
                )
            })?,
            glyph.id,
            font_size,
            origin_x,
            origin_y,
            default_text_color(),
        )
    }

    /// Paints only decoration geometry that the layout engine has already resolved.
    pub fn paint_decorations(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        colors: &[DemoColorSpan],
    ) -> Result<(), String> {
        let stroke_width = (result.input.text_style.font_size / 16.0).max(1.0);
        for decision in result
            .debug
            .decoration_decisions
            .iter()
            .filter(|decision| decision.applied)
        {
            if decision.kind == "Emphasis" {
                scene.fill(
                    Fill::NonZero,
                    self.transform(),
                    color_at(colors, decision.cluster_range).unwrap_or_else(default_text_color),
                    None,
                    &Circle::new(
                        (decision.anchor_x as f64, decision.anchor_y as f64),
                        (decision.dot_diameter / 2.0) as f64,
                    )
                    .to_path(0.1),
                );
            }
        }
        for segment in &result.debug.decoration_segments {
            let color = color_at(colors, segment.source_range).unwrap_or_else(default_text_color);
            match segment.kind.as_str() {
                "Mourning" => self.stroke_mourning_segment(scene, segment, color, stroke_width)?,
                "ProperNoun" => {
                    self.stroke_interlinear_segment(scene, result, segment, color, stroke_width)?
                }
                "BookTitle" => self.stroke_book_title_segment(
                    scene,
                    result,
                    segment,
                    color,
                    result.input.text_style.font_size,
                    stroke_width,
                )?,
                kind => return Err(format!("unsupported decoration segment kind: {kind}")),
            }
        }
        Ok(())
    }

    pub fn paint_rich_text_backgrounds(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        spans: &[RichTextSpan],
    ) -> Result<(), String> {
        let occupied = positioned_rich_text_segments(result, spans);
        for segment in rich_text_background_segments(result, &occupied) {
            let color = segment
                .span
                .paint
                .argb
                .map(color_from_argb)
                .unwrap_or_else(|| AlphaColor::from_rgba8(235, 226, 255, 255));
            let border_inset = match segment.span.paint.background.draw_style {
                RichTextBackgroundDrawStyle::Fill => 0.0,
                RichTextBackgroundDrawStyle::Border { stroke_width } => stroke_width / 2.0,
            };
            let radii = resolved_background_corner_radii(&segment, border_inset);
            let path = rounded_rect_path(
                segment.left + border_inset,
                segment.top + border_inset,
                segment.right - border_inset,
                segment.bottom - border_inset,
                [
                    radii.top_left,
                    radii.top_right,
                    radii.bottom_right,
                    radii.bottom_left,
                ],
            )?;
            match segment.span.paint.background.draw_style {
                RichTextBackgroundDrawStyle::Fill => {
                    scene.fill(Fill::NonZero, self.transform(), color, None, &path)
                }
                RichTextBackgroundDrawStyle::Border { stroke_width } => {
                    scene.stroke(
                        &Stroke::new(stroke_width as f64).with_caps(Cap::Butt),
                        self.transform(),
                        color,
                        None,
                        &path,
                    );
                }
            }
        }
        Ok(())
    }

    pub fn paint_rich_text_lines(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        spans: &[RichTextSpan],
    ) -> Result<(), String> {
        let occupied = positioned_rich_text_segments(result, spans);
        for segment in trimmed_rich_text_decoration_segments(result, &occupied) {
            let color = segment
                .span
                .paint
                .argb
                .map(color_from_argb)
                .unwrap_or_else(default_text_color);
            match &segment.span.paint.line_pattern {
                RichTextLinePattern::Solid => self.stroke_rich_line(
                    scene,
                    result,
                    &segment,
                    color,
                    (result.input.text_style.font_size / 16.0).max(1.0),
                )?,
                RichTextLinePattern::Dashed {
                    stroke_width,
                    dash_length,
                    gap_length,
                } => self.stroke_fitted_dashed_rich_line(
                    scene,
                    result,
                    &segment,
                    color,
                    *stroke_width,
                    *dash_length,
                    *gap_length,
                )?,
                RichTextLinePattern::Dotted {
                    dot_diameter,
                    gap_length,
                } => {
                    let y = rich_text_decoration_line_y(result, &segment, *dot_diameter);
                    let centers = fitted_dotted_line_centers(
                        segment.left,
                        segment.right,
                        *dot_diameter,
                        *gap_length,
                    );
                    for (left, right) in
                        self.kept_intervals_for_rich_text_line(result, &segment, y, *dot_diameter)
                    {
                        for x in centers
                            .iter()
                            .copied()
                            .filter(|x| *x >= left && *x <= right)
                        {
                            scene.fill(
                                Fill::NonZero,
                                self.transform(),
                                color,
                                None,
                                &Circle::new((x as f64, y as f64), (dot_diameter / 2.0) as f64)
                                    .to_path(0.1),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn stroke_rich_line(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        segment: &tiqian::core::layout_queries::RichTextLineSegment,
        color: AlphaColor<Srgb>,
        stroke_width: f32,
    ) -> Result<(), String> {
        if !matches!(
            segment.span.role,
            RichTextRole::Underline | RichTextRole::LineThrough
        ) {
            return Err("rich-text line segment has a non-line role".to_owned());
        }
        let y = rich_text_decoration_line_y(result, segment, stroke_width);
        for (left, right) in
            self.kept_intervals_for_rich_text_line(result, segment, y, stroke_width)
        {
            self.stroke_horizontal_line(scene, left, right, y, color, stroke_width)?;
        }
        Ok(())
    }

    fn stroke_fitted_dashed_rich_line(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        segment: &tiqian::core::layout_queries::RichTextLineSegment,
        color: AlphaColor<Srgb>,
        stroke_width: f32,
        dash_length: f32,
        gap_length: f32,
    ) -> Result<(), String> {
        let y = rich_text_decoration_line_y(result, segment, stroke_width);
        let stroke = Stroke::new(stroke_width as f64).with_caps(Cap::Round);
        let dashes =
            fitted_dashed_line_segments(segment.left, segment.right, dash_length, gap_length);
        for (kept_left, kept_right) in
            self.kept_intervals_for_rich_text_line(result, segment, y, stroke_width)
        {
            for (left, right) in dashes
                .iter()
                .map(|&(left, right)| (left.max(kept_left), right.min(kept_right)))
                .filter(|(left, right)| right > left)
            {
                let cap_inset = (stroke_width / 2.0).min((right - left) / 2.0);
                let mut path = BezPath::new();
                path.move_to(((left + cap_inset) as f64, y as f64));
                path.line_to(((right - cap_inset) as f64, y as f64));
                scene.stroke(&stroke, self.transform(), color, None, &path);
            }
        }
        Ok(())
    }

    fn stroke_book_title_segment(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        segment: &tiqian::core::layout_model::DecorationSegmentInfo,
        color: AlphaColor<Srgb>,
        font_size: f32,
        stroke_width: f32,
    ) -> Result<(), String> {
        for (left, right) in kept_intervals(
            segment.left,
            segment.right,
            line_ink_skip_intervals(
                result,
                segment.line_index,
                segment.top - stroke_width.max(1.0),
                segment.top + stroke_width.max(1.0),
            ),
            browser_like_skip_ink_clearance(font_size, stroke_width),
        ) {
            self.stroke_book_title_path(
                scene,
                left,
                right,
                segment.top,
                color,
                font_size,
                stroke_width,
            )?;
        }
        Ok(())
    }

    fn stroke_book_title_path(
        &self,
        scene: &mut Scene,
        left: f32,
        right: f32,
        y: f32,
        color: AlphaColor<Srgb>,
        font_size: f32,
        stroke_width: f32,
    ) -> Result<(), String> {
        let width = right - left;
        if width <= 0.0 {
            return Ok(());
        }
        let mut path = BezPath::new();
        path.move_to((left as f64, y as f64));
        let mut x = left;
        let mut rising = true;
        while x < right {
            let next = (x + (font_size * WAVE_HALF_LENGTH_EM).max(1.0)).min(right);
            let control_x = (x + next) / 2.0;
            let control_y = y + if rising {
                -font_size * WAVE_AMPLITUDE_EM * 2.0
            } else {
                font_size * WAVE_AMPLITUDE_EM * 2.0
            };
            path.quad_to(
                (control_x as f64, control_y as f64),
                (next as f64, y as f64),
            );
            x = next;
            rising = !rising;
        }
        scene.stroke(
            &Stroke::new(stroke_width as f64).with_caps(Cap::Butt),
            self.transform(),
            color,
            None,
            &path,
        );
        Ok(())
    }

    fn stroke_interlinear_segment(
        &self,
        scene: &mut Scene,
        result: &LayoutResult,
        segment: &tiqian::core::layout_model::DecorationSegmentInfo,
        color: AlphaColor<Srgb>,
        stroke_width: f32,
    ) -> Result<(), String> {
        let font_size = text_style_at(
            &result.input.content.spans,
            &result.input.text_style,
            segment.source_range.start(),
        )
        .font_size;
        for (left, right) in kept_intervals(
            segment.left,
            segment.right,
            line_ink_skip_intervals(
                result,
                segment.line_index,
                segment.top - stroke_width.max(1.0),
                segment.top + stroke_width.max(1.0),
            ),
            browser_like_skip_ink_clearance(font_size, stroke_width),
        ) {
            self.stroke_horizontal_line(scene, left, right, segment.top, color, stroke_width)?;
        }
        Ok(())
    }

    fn kept_intervals_for_rich_text_line(
        &self,
        result: &LayoutResult,
        segment: &tiqian::core::layout_queries::RichTextLineSegment,
        line_y: f32,
        stroke_width: f32,
    ) -> Vec<(f32, f32)> {
        if segment.span.role == RichTextRole::LineThrough {
            return vec![(segment.left, segment.right)];
        }
        let font_size = text_style_at(
            &result.input.content.spans,
            &result.input.text_style,
            segment.range.start(),
        )
        .font_size;
        kept_intervals(
            segment.left,
            segment.right,
            line_ink_skip_intervals(
                result,
                segment.line_index,
                line_y - stroke_width.max(1.0),
                line_y + stroke_width.max(1.0),
            ),
            browser_like_skip_ink_clearance(font_size, stroke_width),
        )
    }

    fn stroke_horizontal_line(
        &self,
        scene: &mut Scene,
        left: f32,
        right: f32,
        y: f32,
        color: AlphaColor<Srgb>,
        stroke_width: f32,
    ) -> Result<(), String> {
        if right <= left {
            return Ok(());
        }
        let mut path = BezPath::new();
        path.move_to((left as f64, y as f64));
        path.line_to((right as f64, y as f64));
        scene.stroke(
            &Stroke::new(stroke_width as f64).with_caps(Cap::Butt),
            self.transform(),
            color,
            None,
            &path,
        );
        Ok(())
    }

    fn stroke_mourning_segment(
        &self,
        scene: &mut Scene,
        segment: &tiqian::core::layout_model::DecorationSegmentInfo,
        color: AlphaColor<Srgb>,
        stroke_width: f32,
    ) -> Result<(), String> {
        let mut path = BezPath::new();
        path.move_to((segment.left as f64, segment.top as f64));
        path.line_to((segment.right as f64, segment.top as f64));
        if !segment.open_start {
            path.move_to((segment.left as f64, segment.top as f64));
            path.line_to((segment.left as f64, segment.bottom as f64));
        }
        if !segment.open_end {
            path.move_to((segment.right as f64, segment.top as f64));
            path.line_to((segment.right as f64, segment.bottom as f64));
        }
        path.move_to((segment.left as f64, segment.bottom as f64));
        path.line_to((segment.right as f64, segment.bottom as f64));
        scene.stroke(
            &Stroke::new(stroke_width as f64).with_caps(Cap::Butt),
            self.transform(),
            color,
            None,
            &path,
        );
        Ok(())
    }
}

fn color_from_argb(argb: i32) -> AlphaColor<Srgb> {
    let bits = argb as u32;
    AlphaColor::from_rgba8(
        (bits >> 16) as u8,
        (bits >> 8) as u8,
        bits as u8,
        (bits >> 24) as u8,
    )
}

fn fitted_dashed_line_segments(
    left: f32,
    right: f32,
    dash_length: f32,
    gap_length: f32,
) -> Vec<(f32, f32)> {
    if right <= left || dash_length <= 0.0 || gap_length < 0.0 {
        return Vec::new();
    }
    let width = right - left;
    if width < dash_length * 2.0 {
        return vec![(left, right)];
    }
    let fitted_count = ((width + gap_length) / (dash_length + gap_length))
        .round()
        .max(2.0) as usize;
    let count = fitted_count.min((width / dash_length).floor().max(2.0) as usize);
    let gap = (width - count as f32 * dash_length) / (count - 1) as f32;
    (0..count)
        .map(|index| {
            let dash_left = left + index as f32 * (dash_length + gap);
            (dash_left, dash_left + dash_length)
        })
        .collect()
}

fn fitted_dotted_line_centers(
    left: f32,
    right: f32,
    dot_diameter: f32,
    gap_length: f32,
) -> Vec<f32> {
    if right <= left || dot_diameter <= 0.0 || gap_length < 0.0 {
        return Vec::new();
    }
    let width = right - left;
    let target_pitch = dot_diameter + gap_length;
    let fitted_count = ((width + gap_length) / target_pitch).round().max(1.0) as usize;
    let count = fitted_count.min((width / dot_diameter).floor().max(1.0) as usize);
    if count == 1 {
        return vec![(left + right) / 2.0];
    }
    let first = left + dot_diameter / 2.0;
    let pitch = (width - dot_diameter) / (count - 1) as f32;
    (0..count)
        .map(|index| first + index as f32 * pitch)
        .collect()
}

fn line_ink_skip_intervals(
    result: &LayoutResult,
    line_index: i32,
    band_top: f32,
    band_bottom: f32,
) -> Vec<(f32, f32)> {
    let positions: HashMap<_, _> = positioned_clusters(result)
        .into_iter()
        .filter(|position| position.line_index == line_index)
        .map(|position| (position.range, position))
        .collect();
    let mut intervals: Vec<_> = result
        .glyph_runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .filter_map(|glyph| {
            let bounds = glyph.bounds?;
            let position = positions.get(&glyph.cluster_range)?;
            let top = position.baseline + glyph.y + bounds.top;
            let bottom = position.baseline + glyph.y + bounds.bottom;
            (top < band_bottom && bottom > band_top).then_some((
                position.draw_x + glyph.x + bounds.left,
                position.draw_x + glyph.x + bounds.right,
            ))
        })
        .collect();
    intervals.sort_by(|left, right| left.0.total_cmp(&right.0));
    intervals
}

fn kept_intervals(
    left: f32,
    right: f32,
    skips: Vec<(f32, f32)>,
    clearance: f32,
) -> Vec<(f32, f32)> {
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (skip_left, skip_right) in skips {
        let start = (skip_left - clearance).clamp(left, right);
        let end = (skip_right + clearance).clamp(left, right);
        if end <= start {
            continue;
        }
        if let Some(previous) = merged.last_mut().filter(|previous| start <= previous.1) {
            previous.1 = previous.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut kept = Vec::new();
    let mut cursor = left;
    for (skip_left, skip_right) in merged {
        if skip_left > cursor + 0.5 {
            kept.push((cursor, skip_left));
        }
        cursor = cursor.max(skip_right);
    }
    if cursor < right - 0.5 {
        kept.push((cursor, right));
    }
    kept
}

fn browser_like_skip_ink_clearance(font_size: f32, stroke_width: f32) -> f32 {
    stroke_width.max(font_size * 0.10).min(13.0)
}

fn rounded_rect_path(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radii: [f32; 4],
) -> Result<BezPath, String> {
    if right <= left || bottom <= top {
        return Err("rich-text background has an empty geometry segment".to_owned());
    }
    let [top_left, top_right, bottom_right, bottom_left] = radii;
    const KAPPA: f32 = 0.552_284_8;
    let mut path = BezPath::new();
    path.move_to(((left + top_left) as f64, top as f64));
    path.line_to(((right - top_right) as f64, top as f64));
    curve_corner(
        &mut path,
        right - top_right,
        top,
        right,
        top + top_right,
        top_right,
        KAPPA,
    );
    path.line_to((right as f64, (bottom - bottom_right) as f64));
    curve_corner(
        &mut path,
        right,
        bottom - bottom_right,
        right - bottom_right,
        bottom,
        bottom_right,
        KAPPA,
    );
    path.line_to(((left + bottom_left) as f64, bottom as f64));
    curve_corner(
        &mut path,
        left + bottom_left,
        bottom,
        left,
        bottom - bottom_left,
        bottom_left,
        KAPPA,
    );
    path.line_to((left as f64, (top + top_left) as f64));
    curve_corner(
        &mut path,
        left,
        top + top_left,
        left + top_left,
        top,
        top_left,
        KAPPA,
    );
    path.close_path();
    Ok(path)
}

fn curve_corner(
    path: &mut BezPath,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    radius: f32,
    kappa: f32,
) {
    if radius == 0.0 {
        path.line_to((end_x as f64, end_y as f64));
        return;
    }
    let control = radius * kappa;
    match (end_x > start_x, end_y > start_y) {
        (true, true) => path.curve_to(
            ((start_x + control) as f64, start_y as f64),
            (end_x as f64, (end_y - control) as f64),
            (end_x as f64, end_y as f64),
        ),
        (false, true) => path.curve_to(
            (start_x as f64, (start_y + control) as f64),
            ((end_x + control) as f64, end_y as f64),
            (end_x as f64, end_y as f64),
        ),
        (false, false) => path.curve_to(
            ((start_x - control) as f64, start_y as f64),
            (end_x as f64, (end_y + control) as f64),
            (end_x as f64, end_y as f64),
        ),
        (true, false) => path.curve_to(
            (start_x as f64, (start_y - control) as f64),
            ((end_x - control) as f64, end_y as f64),
            (end_x as f64, end_y as f64),
        ),
    }
}

fn text_style_at(spans: &[TextSpan], base: &TextStyle, offset: ScalarOffset) -> TextStyle {
    spans
        .iter()
        .rev()
        .find(|span| offset >= span.range.start() && offset < span.range.end())
        .map(|span| span.style.clone())
        .unwrap_or_else(|| base.clone())
}

fn color_at(colors: &[DemoColorSpan], range: TextRange) -> Option<AlphaColor<Srgb>> {
    colors
        .iter()
        .rev()
        .find(|span| span.range.start() <= range.start() && span.range.end() > range.start())
        .map(|span| span.color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiqian::core::geometry::{LayoutConstraints, text_range};
    use tiqian::core::text::Text;
    use tiqian::core::text_model::{
        DecorationKind, DecorationSpan, LayoutInput, LineLengthGrid, ParagraphStyle,
        RichTextBackgroundPaint, RichTextLinePattern, RichTextPaint, RichTextRole, RichTextSpan,
        RubyKind, RubySpan, TextStyle, TiqianTextContent,
    };
    use tiqian::core::units::Ic;
    use tiqian::layout::paragraph_layout_engine::{
        ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
    };

    #[test]
    fn body_replay_uses_positioned_layout_glyphs() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("中文 English")),
                LayoutConstraints::with_defaults(160.0),
            )
            .build(),
        );
        let mut scene = Scene::new();
        DemoRenderer::new(&catalog, 1.0)
            .paint_body(&mut scene, &result, &[])
            .unwrap();
        assert_eq!(
            scene.encoding().resources.glyphs.len(),
            result
                .glyph_runs
                .iter()
                .map(|run| run.glyphs.len())
                .sum::<usize>(),
        );
    }

    #[test]
    fn annotation_replay_uses_ruby_and_bopomofo_layout_decisions() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("中文")),
                LayoutConstraints::with_defaults(160.0),
            )
            .ruby_spans(vec![
                RubySpan::new(text_range(0, 1), Text::from("zhōng")),
                RubySpan::builder(text_range(1, 2), Text::from("ㄨㄣˊ"))
                    .kind(RubyKind::Bopomofo)
                    .build(),
            ])
            .build(),
        );
        assert!(!result.debug.ruby_decisions.is_empty());
        assert!(!result.debug.bopomofo_decisions.is_empty());
        assert!(
            result
                .debug
                .ruby_decisions
                .iter()
                .flat_map(|decision| &decision.glyphs)
                .all(|glyph| glyph.render_font_key.is_some())
        );
        assert!(
            result
                .debug
                .bopomofo_decisions
                .iter()
                .flat_map(|decision| &decision.placements)
                .flat_map(|placement| &placement.glyphs)
                .all(|glyph| glyph.render_font_key.is_some())
        );
        let ruby = result.debug.ruby_decisions.first().unwrap();
        let mut cluster_pen_x = 0.0;
        let mut cluster_advance = 0.0;
        let mut previous_range = None;
        let advancing_ruby_origins: Vec<_> = ruby
            .glyphs
            .iter()
            .filter_map(|glyph| {
                if previous_range.is_some_and(|range| range != glyph.cluster_range) {
                    cluster_pen_x += cluster_advance;
                    cluster_advance = 0.0;
                }
                cluster_advance += glyph.advance;
                previous_range = Some(glyph.cluster_range);
                (glyph.advance > 0.0).then_some(cluster_pen_x + glyph.x)
            })
            .collect();
        assert!(
            advancing_ruby_origins
                .windows(2)
                .any(|origins| origins[1] > origins[0] + 0.1)
        );
        let ruby_ink_top = ruby
            .glyphs
            .iter()
            .filter_map(|glyph| {
                glyph
                    .bounds
                    .map(|bounds| ruby.baseline_y + glyph.y + bounds.top)
            })
            .fold(f32::INFINITY, f32::min);
        let ruby_top_bleed = (-ruby_ink_top).max(0.0).ceil();
        assert!(ruby_top_bleed >= -ruby_ink_top);
        let mut scene = Scene::new();
        DemoRenderer::new(&catalog, 1.0)
            .translated(0.0, ruby_top_bleed)
            .paint_annotations(&mut scene, &result)
            .unwrap();
        assert_eq!(
            scene.encoding().resources.glyphs.len(),
            result
                .debug
                .ruby_decisions
                .iter()
                .flat_map(|decision| &decision.glyphs)
                .count()
                + result
                    .debug
                    .bopomofo_decisions
                    .iter()
                    .flat_map(|decision| &decision.placements)
                    .flat_map(|placement| &placement.glyphs)
                    .count(),
        );
    }

    #[test]
    fn body_replay_includes_shape_once_line_end_hyphens() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let mut result = (32..=112)
            .step_by(4)
            .map(|width| {
                engine.layout(
                    LayoutInput::builder(
                        TiqianTextContent::new(Text::from("representation")),
                        LayoutConstraints::with_defaults(width as f32),
                    )
                    .paragraph_style(
                        ParagraphStyle::builder()
                            .first_line_indent(Some(Ic::ZERO))
                            .line_length_grid(LineLengthGrid::with_enabled(false))
                            .build(),
                    )
                    .text_style(
                        TextStyle::builder()
                            .font_families(vec!["Inter".to_owned()])
                            .build(),
                    )
                    .build(),
                )
            })
            .find(|result| {
                result
                    .lines
                    .iter()
                    .any(|line| !line.hyphen_glyphs.is_empty())
            })
            .expect("English hyphenation should provide a usable shape-once line-end hyphen");
        result.glyph_runs.clear();
        let mut scene = Scene::new();
        DemoRenderer::new(&catalog, 1.0)
            .paint_body(&mut scene, &result, &[])
            .unwrap();
        assert_eq!(
            scene.encoding().resources.glyphs.len(),
            result
                .lines
                .iter()
                .map(|line| line.hyphen_glyphs.len())
                .sum::<usize>(),
        );
    }

    #[test]
    fn rich_text_and_decoration_replay_consume_core_geometry() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("中文书名")),
                LayoutConstraints::with_defaults(160.0),
            )
            .decorations(vec![
                DecorationSpan {
                    range: text_range(0, 1),
                    kind: DecorationKind::Emphasis,
                },
                DecorationSpan {
                    range: text_range(0, 2),
                    kind: DecorationKind::Mourning,
                },
                DecorationSpan {
                    range: text_range(2, 3),
                    kind: DecorationKind::ProperNoun,
                },
                DecorationSpan {
                    range: text_range(3, 4),
                    kind: DecorationKind::BookTitle,
                },
            ])
            .build(),
        );
        let rich_text = vec![
            RichTextSpan::with_paint(
                text_range(0, 1),
                RichTextRole::Background,
                RichTextPaint::builder()
                    .argb(0xFFE0F2FE_u32 as i32)
                    .background(
                        RichTextBackgroundPaint::builder()
                            .horizontal_padding(1.0)
                            .vertical_padding(1.0)
                            .corner_radius(2.0)
                            .build(),
                    )
                    .build(),
            ),
            RichTextSpan::with_paint(
                text_range(1, 2),
                RichTextRole::Underline,
                RichTextPaint::builder()
                    .argb(0xFF2563EB_u32 as i32)
                    .line_pattern(RichTextLinePattern::Solid)
                    .build(),
            ),
            RichTextSpan::with_paint(
                text_range(2, 3),
                RichTextRole::Underline,
                RichTextPaint::builder()
                    .argb(0xFF7C3AED_u32 as i32)
                    .line_pattern(RichTextLinePattern::dashed(1.0, 3.0, 2.0))
                    .build(),
            ),
            RichTextSpan::with_paint(
                text_range(3, 4),
                RichTextRole::LineThrough,
                RichTextPaint::builder()
                    .argb(0xFFDC2626_u32 as i32)
                    .line_pattern(RichTextLinePattern::dotted(1.5, 1.5))
                    .build(),
            ),
        ];
        assert!(
            result
                .debug
                .decoration_decisions
                .iter()
                .any(|decision| decision.kind == "Emphasis" && decision.applied)
        );
        assert!(
            result
                .debug
                .decoration_segments
                .iter()
                .any(|segment| segment.kind == "Mourning")
        );
        assert!(
            result
                .debug
                .decoration_segments
                .iter()
                .any(|segment| segment.kind == "ProperNoun")
        );
        assert!(
            result
                .debug
                .decoration_segments
                .iter()
                .any(|segment| segment.kind == "BookTitle")
        );
        let mut scene = Scene::new();
        let renderer = DemoRenderer::new(&catalog, 1.0);
        renderer
            .paint_rich_text_backgrounds(&mut scene, &result, &rich_text)
            .unwrap();
        renderer.paint_body(&mut scene, &result, &[]).unwrap();
        renderer
            .paint_rich_text_lines(&mut scene, &result, &rich_text)
            .unwrap();
        renderer
            .paint_decorations(&mut scene, &result, &[])
            .unwrap();
        assert!(!scene.encoding().is_empty());

        let mourning = result
            .debug
            .decoration_segments
            .iter()
            .find(|segment| segment.kind == "Mourning")
            .unwrap();
        let mut decoration_scene = Scene::new();
        renderer
            .paint_decorations(&mut decoration_scene, &result, &[])
            .unwrap();
        assert!(mourning.right > mourning.left && mourning.bottom > mourning.top);
        assert!(decoration_scene.encoding().n_paths > 0);
    }

    #[test]
    fn skip_ink_merges_clearance_before_emitting_kept_runs() {
        assert_eq!(
            kept_intervals(0.0, 20.0, vec![(4.0, 8.0), (7.0, 12.0)], 2.0),
            vec![(0.0, 2.0), (14.0, 20.0)],
        );
    }
}
