use std::num::NonZeroU32;
use std::rc::Rc;

use softbuffer::{Context, Surface};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::LayoutModel::LayoutResult;
use tiqian::org::tiqian::core::LayoutQueries::positioned_clusters;
use tiqian::org::tiqian::core::TextModel::LineLengthGrid;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, OwnedDisplayHandle};
use winit::window::{Window, WindowId};

use crate::font_backend::DemoFontCatalog;
use crate::renderer::DemoRenderer;
use crate::sample::{build_document_demo, DemoDocument, DemoDocumentDemoBlock};

const WINDOW_TITLE: &str = "Tiqian paragraph demo";
const INITIAL_LOGICAL_WIDTH: f64 = 720.0;
const INITIAL_LOGICAL_HEIGHT: f64 = 480.0;
const LOGICAL_PADDING: f32 = 24.0;
const TOP_LEVEL_GAP_LOGICAL: f32 = 20.0;
const WHEEL_LINE_LOGICAL: f32 = 40.0;

pub struct DesktopParagraphDemo {
    catalog: DemoFontCatalog,
    engine: ExplainableStubParagraphLayoutEngine,
    context: Context<OwnedDisplayHandle>,
    window: Option<Rc<Window>>,
    surface: Option<Surface<OwnedDisplayHandle, Rc<Window>>>,
    page: Option<DemoPage>,
    layout_key: Option<LayoutKey>,
    scroll_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LayoutKey {
    physical_content_width: u32,
    scale_factor: f32,
}

struct DemoPage {
    blocks: Vec<DemoPageBlock>,
    height: f32,
    left_overhang: f32,
    top_overhang: f32,
    right_overhang: f32,
    bottom_overhang: f32,
}

enum DemoPageBlock {
    Text {
        document: DemoDocument,
        layout: LayoutResult,
        y: f32,
    },
    ListItem {
        marker: DemoDocument,
        marker_layout: LayoutResult,
        marker_y: f32,
        body: DemoDocument,
        body_layout: LayoutResult,
        gutter: f32,
        y: f32,
    },
}

fn layout_paint_overhang(layout: &LayoutResult) -> (f32, f32, f32, f32) {
    let width = layout.input.constraints.max_width();
    let positions = positioned_clusters(layout);
    let mut left = 0.0_f32;
    let mut top = 0.0_f32;
    let mut right = 0.0_f32;
    let mut bottom = 0.0_f32;
    for glyph in layout.glyph_runs.iter().flat_map(|run| &run.glyphs) {
        let Some(bounds) = glyph.bounds else {
            continue;
        };
        let Some(cluster) = positions.iter().find(|cluster| cluster.range == glyph.cluster_range) else {
            continue;
        };
        left = left.max(-(cluster.draw_x + glyph.x + bounds.left));
        top = top.max(-(cluster.baseline + glyph.y + bounds.top));
        right = right.max(cluster.draw_x + glyph.x + bounds.right - width);
        bottom = bottom.max(cluster.baseline + glyph.y + bounds.bottom - layout.size.height);
    }
    for ruby in &layout.debug.ruby_decisions {
        let origin_x = ruby.center_x - ruby.width / 2.0;
        let mut cluster_pen_x = 0.0;
        let mut cluster_advance = 0.0;
        let mut previous_range = None;
        for glyph in &ruby.glyphs {
            if previous_range.is_some_and(|range| range != glyph.cluster_range) {
                cluster_pen_x += cluster_advance;
                cluster_advance = 0.0;
            }
            if let Some(bounds) = glyph.bounds {
                left = left.max(-(origin_x + cluster_pen_x + glyph.x + bounds.left));
                top = top.max(-(ruby.baseline_y + glyph.y + bounds.top));
                right = right.max(origin_x + cluster_pen_x + glyph.x + bounds.right - width);
                bottom = bottom.max(ruby.baseline_y + glyph.y + bounds.bottom - layout.size.height);
            }
            cluster_advance += glyph.advance;
            previous_range = Some(glyph.cluster_range);
        }
        left = left.max(-origin_x);
        top = top.max(-(ruby.baseline_y - ruby.ascent));
        right = right.max(origin_x + ruby.width - width);
        bottom = bottom.max(ruby.baseline_y + ruby.descent - layout.size.height);
    }
    for bopomofo in &layout.debug.bopomofo_decisions {
        for placement in &bopomofo.placements {
            for glyph in &placement.glyphs {
                let Some(bounds) = glyph.bounds else {
                    continue;
                };
                left = left.max(-(placement.draw_x + glyph.x + bounds.left));
                top = top.max(-(placement.baseline_y + glyph.y + bounds.top));
                right = right.max(placement.draw_x + glyph.x + bounds.right - width);
                bottom = bottom.max(placement.baseline_y + glyph.y + bounds.bottom - layout.size.height);
            }
            left = left.max(-placement.left);
            top = top.max(-placement.top);
            right = right.max(placement.left + placement.width - width);
            bottom = bottom.max(placement.top + placement.height - layout.size.height);
        }
    }
    for decoration in &layout.debug.decoration_decisions {
        if decoration.applied && decoration.dot_diameter > 0.0 {
            let radius = decoration.dot_diameter / 2.0;
            left = left.max(-(decoration.anchor_x - radius));
            top = top.max(-(decoration.anchor_y - radius));
            right = right.max(decoration.anchor_x + radius - width);
            bottom = bottom.max(decoration.anchor_y + radius - layout.size.height);
        }
    }
    for segment in &layout.debug.decoration_segments {
        left = left.max(-segment.left);
        top = top.max(-segment.top);
        right = right.max(segment.right - width);
        bottom = bottom.max(segment.bottom - layout.size.height);
    }
    (left.max(0.0), top.max(0.0), right.max(0.0), bottom.max(0.0))
}

impl DesktopParagraphDemo {
    pub fn new(catalog: DemoFontCatalog, context: Context<OwnedDisplayHandle>) -> Self {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        Self {
            catalog,
            engine,
            context,
            window: None,
            surface: None,
            page: None,
            layout_key: None,
            scroll_y: 0,
        }
    }

    fn update_layout(&mut self, physical_size: PhysicalSize<u32>, scale_factor: f64) {
        let scale_factor = scale_factor as f32;
        let padding = (LOGICAL_PADDING * scale_factor).round().max(0.0) as u32;
        let physical_content_width = physical_size.width.saturating_sub(padding.saturating_mul(2)).max(1);
        let key = LayoutKey {
            physical_content_width,
            scale_factor,
        };
        if self.layout_key == Some(key) {
            return;
        }
        let document = build_document_demo(physical_content_width as f32, scale_factor);
        let mut blocks = Vec::new();
        let mut y = 0.0;
        let mut left_overhang = 0.0_f32;
        let mut top_overhang = 0.0_f32;
        let mut right_overhang = 0.0_f32;
        let mut bottom_overhang = 0.0_f32;
        for (index, block) in document.blocks.into_iter().enumerate() {
            match block {
                DemoDocumentDemoBlock::TextField(document)
                | DemoDocumentDemoBlock::Paragraph(document) => {
                    let (document, layout) = self.layout_document(document, physical_content_width as f32);
                    let (left, top, right, bottom) = layout_paint_overhang(&layout);
                    left_overhang = left_overhang.max(left);
                    top_overhang = top_overhang.max(top - y);
                    right_overhang = right_overhang.max(right);
                    bottom_overhang = bottom_overhang.max(y + layout.size.height + bottom);
                    let block_y = y;
                    y += layout.size.height;
                    blocks.push(DemoPageBlock::Text {
                        document,
                        layout,
                        y: block_y,
                    });
                    if index < 3 {
                        y += TOP_LEVEL_GAP_LOGICAL * scale_factor;
                    }
                }
                DemoDocumentDemoBlock::ListItem { marker, body } => {
                    let font_size = body.input.text_style.font_size;
                    let mut marker_measurement = marker.clone();
                    marker_measurement.input.paragraph_style.line_length_grid = LineLengthGrid::with_enabled(false);
                    let (_, marker_measurement_layout) =
                        self.layout_document(marker_measurement, 100_000.0);
                    let gutter = (marker_measurement_layout.size.width / font_size).ceil().max(1.0) * font_size;
                    let (marker, marker_layout) = self.layout_document(marker, gutter);
                    let (body, body_layout) = self.layout_document(
                        body,
                        (physical_content_width as f32 - gutter).max(1.0),
                    );
                    let marker_y = body_layout
                        .lines
                        .first()
                        .zip(marker_layout.lines.first())
                        .map_or(0.0, |(body, marker)| body.baseline - marker.baseline);
                    let (marker_left, marker_top, marker_right, marker_bottom) =
                        layout_paint_overhang(&marker_layout);
                    let (body_left, body_top, body_right, body_bottom) =
                        layout_paint_overhang(&body_layout);
                    left_overhang = left_overhang.max(marker_left);
                    left_overhang = left_overhang.max(body_left - gutter);
                    top_overhang = top_overhang.max(marker_top - (y + marker_y));
                    top_overhang = top_overhang.max(body_top - y);
                    right_overhang = right_overhang.max(marker_right);
                    right_overhang = right_overhang.max(body_right);
                    bottom_overhang = bottom_overhang.max(y + marker_y + marker_layout.size.height + marker_bottom);
                    bottom_overhang = bottom_overhang.max(y + body_layout.size.height + body_bottom);
                    let height = body_layout
                        .size
                        .height
                        .max(marker_y + marker_layout.size.height);
                    blocks.push(DemoPageBlock::ListItem {
                        marker,
                        marker_layout,
                        marker_y,
                        body,
                        body_layout,
                        gutter,
                        y,
                    });
                    y += height;
                }
                DemoDocumentDemoBlock::Section { height } => y += height,
            }
        }
        self.page = Some(DemoPage {
            blocks,
            height: y,
            left_overhang: left_overhang.max(0.0).ceil(),
            top_overhang: top_overhang.max(0.0).ceil(),
            right_overhang: right_overhang.max(0.0).ceil(),
            bottom_overhang: (bottom_overhang - y).max(0.0).ceil(),
        });
        self.layout_key = Some(key);
        self.clamp_scroll(physical_size.height, padding);
    }

    fn layout_document(
        &mut self,
        mut document: DemoDocument,
        physical_content_width: f32,
    ) -> (DemoDocument, LayoutResult) {
        document.input.constraints = LayoutConstraints::with_defaults(physical_content_width.max(1.0));
        let layout = self.engine.layout(document.input.clone());
        (document, layout)
    }

    fn render(&mut self, physical_size: PhysicalSize<u32>) -> Result<(), String> {
        if physical_size.width == 0 || physical_size.height == 0 {
            return Ok(());
        }
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| "redraw requested before the demo window was created".to_owned())?
            .clone();
        let scale_factor = window.scale_factor() as f32;
        self.update_layout(physical_size, window.scale_factor());
        let page = self
            .page
            .as_ref()
            .ok_or_else(|| "demo page layout was not produced".to_owned())?;
        let padding = (LOGICAL_PADDING * scale_factor).round() as i32;
        let content_width = self
            .layout_key
            .ok_or_else(|| "paragraph layout key was not produced".to_owned())?
            .physical_content_width;
        let page_width = (page.left_overhang + content_width as f32 + page.right_overhang)
            .ceil()
            .max(1.0) as u32;
        let content_height = (page.top_overhang + page.height + page.bottom_overhang)
            .ceil()
            .max(1.0) as u32;
        let mut page_pixmap = tiny_skia::Pixmap::new(page_width, content_height)
            .ok_or_else(|| "cannot allocate demo page pixmap".to_owned())?;
        let renderer = DemoRenderer::new(&self.catalog, scale_factor);
        for block in &page.blocks {
            match block {
                DemoPageBlock::Text { document, layout, y } => {
                    self.paint_document(
                        &mut page_pixmap,
                        document,
                        layout,
                        page.left_overhang.round() as i32,
                        (page.top_overhang + y).round() as i32,
                        &renderer,
                    )?;
                }
                DemoPageBlock::ListItem {
                    marker,
                    marker_layout,
                    marker_y,
                    body,
                    body_layout,
                    gutter,
                    y,
                } => {
                    self.paint_document(
                        &mut page_pixmap,
                        marker,
                        marker_layout,
                        page.left_overhang.round() as i32,
                        (page.top_overhang + y + marker_y).round() as i32,
                        &renderer,
                    )?;
                    self.paint_document(
                        &mut page_pixmap,
                        body,
                        body_layout,
                        (page.left_overhang + gutter).round() as i32,
                        (page.top_overhang + y).round() as i32,
                        &renderer,
                    )?;
                }
            }
        }

        let mut frame = tiny_skia::Pixmap::new(physical_size.width, physical_size.height)
            .ok_or_else(|| "cannot allocate window pixmap".to_owned())?;
        frame.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        frame.draw_pixmap(
            padding - page.left_overhang.round() as i32,
            padding - self.scroll_y - page.top_overhang.round() as i32,
            page_pixmap.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        window.pre_present_notify();
        let surface = self
            .surface
            .as_mut()
            .ok_or_else(|| "redraw requested before the demo surface was created".to_owned())?;
        surface
            .resize(
                NonZeroU32::new(physical_size.width).unwrap(),
                NonZeroU32::new(physical_size.height).unwrap(),
            )
            .map_err(|error| format!("softbuffer resize failed: {error}"))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|error| format!("softbuffer buffer acquisition failed: {error}"))?;
        for (pixel, rgba) in buffer.iter_mut().zip(frame.data().chunks_exact(4)) {
            *pixel = u32::from(rgba[2]) | (u32::from(rgba[1]) << 8) | (u32::from(rgba[0]) << 16);
        }
        buffer
            .present()
            .map_err(|error| format!("softbuffer present failed: {error}"))
    }

    fn paint_document(
        &self,
        page: &mut tiny_skia::Pixmap,
        document: &DemoDocument,
        layout: &LayoutResult,
        x: i32,
        y: i32,
        renderer: &DemoRenderer<'_>,
    ) -> Result<(), String> {
        let renderer = renderer.translated(x as f32, y as f32);
        renderer.paint_rich_text_backgrounds(page, layout, &document.rich_text)?;
        renderer.paint_body(page, layout, &document.colors)?;
        renderer.paint_rich_text_lines(page, layout, &document.rich_text)?;
        renderer.paint_decorations(page, layout, &document.colors)?;
        renderer.paint_annotations(page, layout)?;
        Ok(())
    }

    fn request_layout_and_redraw(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.update_layout(size, scale_factor);
        let padding = (LOGICAL_PADDING * scale_factor as f32).round().max(0.0) as u32;
        self.clamp_scroll(size.height, padding);
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn scroll_by(&mut self, delta: f32, physical_size: PhysicalSize<u32>, scale_factor: f64) {
        let padding = (LOGICAL_PADDING * scale_factor as f32).round().max(0.0) as u32;
        self.scroll_y = (self.scroll_y as f32 + delta).round() as i32;
        self.clamp_scroll(physical_size.height, padding);
    }

    fn clamp_scroll(&mut self, physical_height: u32, padding: u32) {
        let viewport_height = physical_height.saturating_sub(padding.saturating_mul(2)) as f32;
        let maximum = self
            .page
            .as_ref()
            .map(|page| (page.height - viewport_height).ceil().max(0.0) as i32)
            .unwrap_or(0);
        self.scroll_y = self.scroll_y.clamp(0, maximum);
    }
}

impl ApplicationHandler for DesktopParagraphDemo {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Rc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(WINDOW_TITLE)
                        .with_inner_size(LogicalSize::new(INITIAL_LOGICAL_WIDTH, INITIAL_LOGICAL_HEIGHT)),
                )
                .expect("paragraph-demo window creation failed"),
        );
        let surface = Surface::new(&self.context, window.clone())
            .expect("paragraph-demo softbuffer surface creation failed");
        self.window = Some(window);
        self.surface = Some(surface);
        self.request_layout_and_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.request_layout_and_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let size = window.inner_size();
                let scale_factor = window.scale_factor();
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * WHEEL_LINE_LOGICAL * scale_factor as f32,
                    MouseScrollDelta::PixelDelta(position) => -position.y as f32,
                };
                self.scroll_by(delta, size, scale_factor);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let size = window.inner_size();
                if let Err(error) = self.render(size) {
                    eprintln!("paragraph-demo render failed: {error}");
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::build_document;

    fn layout_signature(result: &LayoutResult) -> Vec<(i32, i32, String)> {
        result
            .lines
            .iter()
            .map(|line| {
                (
                    line.range.start(),
                    line.range.end(),
                    format!("{:?}", line.end_reason),
                )
            })
            .collect()
    }

    fn assert_replayable(catalog: &DemoFontCatalog, result: &LayoutResult, document: &DemoDocument) {
        assert!(result
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .chain(result.lines.iter().flat_map(|line| &line.hyphen_glyphs))
            .all(|glyph| glyph.render_font_key.is_some()));
        let width = result.input.constraints.max_width().ceil().max(1.0) as u32;
        let height = result.size.height.ceil().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
        let renderer = DemoRenderer::new(catalog, 1.0);
        renderer
            .paint_rich_text_backgrounds(&mut pixmap, result, &document.rich_text)
            .unwrap();
        renderer.paint_body(&mut pixmap, result, &document.colors).unwrap();
        renderer
            .paint_rich_text_lines(&mut pixmap, result, &document.rich_text)
            .unwrap();
        renderer.paint_decorations(&mut pixmap, result, &document.colors).unwrap();
        renderer.paint_annotations(&mut pixmap, result).unwrap();
    }

    #[test]
    fn width_round_trip_restores_the_same_layout() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let wide_document = build_document(640.0, 1.0);
        let narrow_document = build_document(240.0, 1.0);
        let restored_document = build_document(640.0, 1.0);
        let wide = engine.layout(wide_document.input.clone());
        let narrow = engine.layout(narrow_document.input.clone());
        let restored = engine.layout(restored_document.input.clone());
        assert!(wide.lines.len() != narrow.lines.len() || layout_signature(&wide) != layout_signature(&narrow));
        assert_eq!(layout_signature(&wide), layout_signature(&restored));
        assert_eq!(wide.input.content.text, narrow.input.content.text);
        assert_eq!(wide.input.content.text, restored.input.content.text);
        assert_eq!(wide.input.content.source_boundaries, narrow.input.content.source_boundaries);
        assert_eq!(wide.input.content.source_boundaries, restored.input.content.source_boundaries);
        assert_replayable(&catalog, &wide, &wide_document);
        assert_replayable(&catalog, &narrow, &narrow_document);
        assert_replayable(&catalog, &restored, &restored_document);
    }

    #[test]
    fn compose_demo_blocks_are_all_layout_and_replayable() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let document = build_document_demo(640.0, 1.0);
        let mut rendered_text_blocks = 0;
        for block in document.blocks {
            match block {
                DemoDocumentDemoBlock::TextField(document)
                | DemoDocumentDemoBlock::Paragraph(document) => {
                    let result = engine.layout(document.input.clone());
                    assert_replayable(&catalog, &result, &document);
                    rendered_text_blocks += 1;
                }
                DemoDocumentDemoBlock::ListItem { marker, body } => {
                    let marker_result = engine.layout(marker.input.clone());
                    let body_result = engine.layout(body.input.clone());
                    assert_replayable(&catalog, &marker_result, &marker);
                    assert_replayable(&catalog, &body_result, &body);
                    rendered_text_blocks += 2;
                }
                DemoDocumentDemoBlock::Section { .. } => {}
            }
        }
        assert_eq!(rendered_text_blocks, 23);
    }
}
