use std::sync::Arc;

use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text_model::LineLengthGrid;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use vello::peniko::color::palette::css::WHITE;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::font_backend::DemoFontCatalog;
use crate::renderer::DemoRenderer;
use crate::sample::{DemoDocument, DemoDocumentDemoBlock, build_document_demo};

const WINDOW_TITLE: &str = "Tiqian paragraph demo";
const INITIAL_LOGICAL_WIDTH: f64 = 720.0;
const INITIAL_LOGICAL_HEIGHT: f64 = 480.0;
const LOGICAL_PADDING: f32 = 24.0;
const TOP_LEVEL_GAP_LOGICAL: f32 = 20.0;
const WHEEL_LINE_LOGICAL: f32 = 40.0;

pub struct DesktopParagraphDemo {
    catalog: DemoFontCatalog,
    engine: ExplainableStubParagraphLayoutEngine,
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    page: Option<DemoPage>,
    layout_key: Option<LayoutKey>,
    scroll_y: i32,
}

#[derive(Debug)]
enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
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
        paint_top: f32,
        paint_bottom: f32,
    },
    ListItem {
        marker: DemoDocument,
        marker_layout: LayoutResult,
        marker_y: f32,
        body: DemoDocument,
        body_layout: LayoutResult,
        gutter: f32,
        y: f32,
        paint_top: f32,
        paint_bottom: f32,
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
        let Some(cluster) = positions
            .iter()
            .find(|cluster| cluster.range == glyph.cluster_range)
        else {
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
                bottom =
                    bottom.max(placement.baseline_y + glyph.y + bounds.bottom - layout.size.height);
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
    pub fn new(catalog: DemoFontCatalog) -> Self {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        Self {
            catalog,
            engine,
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: RenderState::Suspended(None),
            scene: Scene::new(),
            page: None,
            layout_key: None,
            scroll_y: 0,
        }
    }

    fn update_layout(&mut self, physical_size: PhysicalSize<u32>, scale_factor: f64) {
        let scale_factor = scale_factor as f32;
        let padding = (LOGICAL_PADDING * scale_factor).round().max(0.0) as u32;
        let physical_content_width = physical_size
            .width
            .saturating_sub(padding.saturating_mul(2))
            .max(1);
        let key = LayoutKey {
            physical_content_width,
            scale_factor,
        };
        if self.layout_key == Some(key) {
            return;
        }

        let t = std::time::Instant::now();

        let document = build_document_demo(physical_content_width as f32, scale_factor);

        let mut blocks = Vec::new();
        let mut y = 0.0;
        let mut left_overhang = 0.0_f32;
        let mut top_overhang = 0.0_f32;
        let mut right_overhang = 0.0_f32;
        let mut bottom_overhang = 0.0_f32;
        for (index, block) in document.blocks.into_iter().enumerate() {
            match block {
                DemoDocumentDemoBlock::Paragraph(document) => {
                    let (document, layout) =
                        self.layout_document(document, physical_content_width as f32);
                    let (left, top, right, bottom) = layout_paint_overhang(&layout);
                    left_overhang = left_overhang.max(left);
                    top_overhang = top_overhang.max(top - y);
                    right_overhang = right_overhang.max(right);
                    bottom_overhang = bottom_overhang.max(y + layout.size.height + bottom);
                    let block_y = y;
                    let paint_top = block_y - top;
                    let paint_bottom = block_y + layout.size.height + bottom;
                    y += layout.size.height;
                    blocks.push(DemoPageBlock::Text {
                        document,
                        layout,
                        y: block_y,
                        paint_top,
                        paint_bottom,
                    });
                    if index < 3 {
                        y += TOP_LEVEL_GAP_LOGICAL * scale_factor;
                    }
                }
                DemoDocumentDemoBlock::NarrowParagraph {
                    document,
                    max_width,
                } => {
                    let (document, layout) = self.layout_document(document, max_width);
                    let (left, top, right, bottom) = layout_paint_overhang(&layout);
                    left_overhang = left_overhang.max(left);
                    top_overhang = top_overhang.max(top - y);
                    right_overhang = right_overhang.max(right);
                    bottom_overhang = bottom_overhang.max(y + layout.size.height + bottom);
                    let block_y = y;
                    let paint_top = block_y - top;
                    let paint_bottom = block_y + layout.size.height + bottom;
                    y += layout.size.height;
                    blocks.push(DemoPageBlock::Text {
                        document,
                        layout,
                        y: block_y,
                        paint_top,
                        paint_bottom,
                    });
                }
                DemoDocumentDemoBlock::ListItem { marker, body } => {
                    let font_size = body.input.text_style.font_size;
                    let mut marker_measurement = marker.clone();
                    marker_measurement.input.paragraph_style.line_length_grid =
                        LineLengthGrid::with_enabled(false);
                    let (_, marker_measurement_layout) =
                        self.layout_document(marker_measurement, 100_000.0);
                    let gutter = (marker_measurement_layout.size.width / font_size)
                        .ceil()
                        .max(1.0)
                        * font_size;
                    let (marker, marker_layout) = self.layout_document(marker, gutter);
                    let (body, body_layout) = self
                        .layout_document(body, (physical_content_width as f32 - gutter).max(1.0));
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
                    bottom_overhang = bottom_overhang
                        .max(y + marker_y + marker_layout.size.height + marker_bottom);
                    bottom_overhang =
                        bottom_overhang.max(y + body_layout.size.height + body_bottom);
                    let height = body_layout
                        .size
                        .height
                        .max(marker_y + marker_layout.size.height);
                    let paint_top = (y + marker_y - marker_top).min(y - body_top);
                    let paint_bottom = (y + marker_y + marker_layout.size.height + marker_bottom)
                        .max(y + body_layout.size.height + body_bottom);
                    blocks.push(DemoPageBlock::ListItem {
                        marker,
                        marker_layout,
                        marker_y,
                        body,
                        body_layout,
                        gutter,
                        y,
                        paint_top,
                        paint_bottom,
                    });
                    y += height;
                }
                DemoDocumentDemoBlock::Section { height } => y += height,
            }
        }

        println!(
            "layout demo page with physical_content_width={} in {:?}",
            physical_content_width,
            t.elapsed()
        );

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
        document.input.constraints =
            LayoutConstraints::with_defaults(physical_content_width.max(1.0));
        let layout = self.engine.layout(document.input.clone());
        (document, layout)
    }

    fn render(&mut self, physical_size: PhysicalSize<u32>) -> Result<(), String> {
        if physical_size.width == 0 || physical_size.height == 0 {
            return Ok(());
        }
        let window = match &self.state {
            RenderState::Active {
                valid_surface: true,
                window,
                ..
            } => window.clone(),
            RenderState::Active { .. } => return Ok(()),
            RenderState::Suspended(_) => {
                return Err("redraw requested before the demo window was created".to_owned());
            }
        };
        let scale_factor = window.scale_factor() as f32;
        self.update_layout(physical_size, window.scale_factor());
        let page = self
            .page
            .as_ref()
            .ok_or_else(|| "demo page layout was not produced".to_owned())?;
        let padding = (LOGICAL_PADDING * scale_factor).round() as i32;
        let renderer = DemoRenderer::new(&self.catalog, scale_factor);
        let page_origin_x = padding - page.left_overhang.round() as i32;
        let page_origin_y = padding - self.scroll_y - page.top_overhang.round() as i32;
        let viewport_top = self.scroll_y as f32;
        let viewport_bottom = viewport_top
            + physical_size
                .height
                .saturating_sub((padding.max(0) as u32).saturating_mul(2)) as f32;
        self.scene.reset();
        for block in &page.blocks {
            let (paint_top, paint_bottom) = match block {
                DemoPageBlock::Text {
                    paint_top,
                    paint_bottom,
                    ..
                }
                | DemoPageBlock::ListItem {
                    paint_top,
                    paint_bottom,
                    ..
                } => (*paint_top, *paint_bottom),
            };
            if paint_bottom <= viewport_top || paint_top >= viewport_bottom {
                continue;
            }
            match block {
                DemoPageBlock::Text {
                    document,
                    layout,
                    y,
                    ..
                } => {
                    Self::paint_document(
                        &mut self.scene,
                        document,
                        layout,
                        page_origin_x + page.left_overhang.round() as i32,
                        page_origin_y + (page.top_overhang + y).round() as i32,
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
                    ..
                } => {
                    Self::paint_document(
                        &mut self.scene,
                        marker,
                        marker_layout,
                        page_origin_x + page.left_overhang.round() as i32,
                        page_origin_y + (page.top_overhang + y + marker_y).round() as i32,
                        &renderer,
                    )?;
                    Self::paint_document(
                        &mut self.scene,
                        body,
                        body_layout,
                        page_origin_x + (page.left_overhang + gutter).round() as i32,
                        page_origin_y + (page.top_overhang + y).round() as i32,
                        &renderer,
                    )?;
                }
            }
        }

        window.pre_present_notify();
        let RenderState::Active { surface, .. } = &mut self.state else {
            return Err("redraw requested before the demo surface was created".to_owned());
        };
        let device_handle = &self.context.devices[surface.dev_id];
        self.renderers[surface.dev_id]
            .as_mut()
            .ok_or_else(|| "demo renderer was not created for the active GPU device".to_owned())?
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &surface.target_view,
                &vello::RenderParams {
                    base_color: WHITE,
                    width: surface.config.width,
                    height: surface.config.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|error| format!("Vello scene render failed: {error}"))?;
        let surface_texture = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                return Err("GPU surface configuration became outdated".to_owned());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err("GPU surface was lost".to_owned());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("GPU surface acquisition failed validation".to_owned());
            }
        };
        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Tiqian paragraph demo surface blit"),
                });
        surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &surface.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
        device_handle
            .device
            .poll(wgpu::PollType::Poll)
            .map_err(|error| {
                format!("GPU device poll failed after presenting paragraph demo: {error}")
            })?;
        Ok(())
    }

    fn paint_document(
        scene: &mut Scene,
        document: &DemoDocument,
        layout: &LayoutResult,
        x: i32,
        y: i32,
        renderer: &DemoRenderer<'_>,
    ) -> Result<(), String> {
        let renderer = renderer.translated(x as f32, y as f32);
        renderer.paint_rich_text_backgrounds(scene, layout, &document.rich_text)?;
        renderer.paint_body(scene, layout, &document.colors)?;
        renderer.paint_rich_text_lines(scene, layout, &document.rich_text)?;
        renderer.paint_decorations(scene, layout, &document.colors)?;
        renderer.paint_annotations(scene, layout)?;
        Ok(())
    }

    fn request_layout_and_redraw(&mut self) {
        let RenderState::Active { window, .. } = &self.state else {
            return;
        };
        let window = window.clone();
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.update_layout(size, scale_factor);
        let padding = (LOGICAL_PADDING * scale_factor as f32).round().max(0.0) as u32;
        self.clamp_scroll(size.height, padding);
        window.request_redraw();
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
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };
        let window = cached_window.take().unwrap_or_else(|| {
            Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(WINDOW_TITLE)
                            .with_inner_size(LogicalSize::new(
                                INITIAL_LOGICAL_WIDTH,
                                INITIAL_LOGICAL_HEIGHT,
                            )),
                    )
                    .expect("paragraph-demo window creation failed"),
            )
        });
        let size = window.inner_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("paragraph-demo GPU surface creation failed");
        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id].get_or_insert_with(|| {
            Renderer::new(
                &self.context.devices[surface.dev_id].device,
                RendererOptions {
                    use_cpu: false,
                    ..Default::default()
                },
            )
            .expect("paragraph-demo GPU renderer creation failed")
        });
        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: size.width != 0 && size.height != 0,
            window,
        };
        self.request_layout_and_redraw();
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let (window, surface, valid_surface) = match &mut self.state {
            RenderState::Active {
                surface,
                valid_surface,
                window,
            } if window.id() == window_id => (window.clone(), surface, valid_surface),
            _ => return,
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                    self.request_layout_and_redraw();
                } else {
                    *valid_surface = false;
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.request_layout_and_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let size = window.inner_size();
                let scale_factor = window.scale_factor();
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        -y * WHEEL_LINE_LOGICAL * scale_factor as f32
                    }
                    MouseScrollDelta::PixelDelta(position) => -position.y as f32,
                };
                self.scroll_by(delta, size, scale_factor);
                window.request_redraw();
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

    fn assert_replayable(
        catalog: &DemoFontCatalog,
        result: &LayoutResult,
        document: &DemoDocument,
    ) {
        assert!(
            result
                .glyph_runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .chain(result.lines.iter().flat_map(|line| &line.hyphen_glyphs))
                .all(|glyph| glyph.render_font_key.is_some())
        );
        let mut scene = Scene::new();
        let renderer = DemoRenderer::new(catalog, 1.0);
        renderer
            .paint_rich_text_backgrounds(&mut scene, result, &document.rich_text)
            .unwrap();
        renderer
            .paint_body(&mut scene, result, &document.colors)
            .unwrap();
        renderer
            .paint_rich_text_lines(&mut scene, result, &document.rich_text)
            .unwrap();
        renderer
            .paint_decorations(&mut scene, result, &document.colors)
            .unwrap();
        renderer.paint_annotations(&mut scene, result).unwrap();
        assert!(!scene.encoding().draw_tags.is_empty());
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
        assert!(
            wide.lines.len() != narrow.lines.len()
                || layout_signature(&wide) != layout_signature(&narrow)
        );
        assert_eq!(layout_signature(&wide), layout_signature(&restored));
        assert_eq!(wide.input.content.text, narrow.input.content.text);
        assert_eq!(wide.input.content.text, restored.input.content.text);
        assert_eq!(
            wide.input.content.source_boundaries,
            narrow.input.content.source_boundaries
        );
        assert_eq!(
            wide.input.content.source_boundaries,
            restored.input.content.source_boundaries
        );
        assert_replayable(&catalog, &wide, &wide_document);
        assert_replayable(&catalog, &narrow, &narrow_document);
        assert_replayable(&catalog, &restored, &restored_document);
    }

    #[test]
    fn demo_blocks_are_all_layout_and_replayable() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog.clone());
        let document = build_document_demo(640.0, 1.0);
        for block in document.blocks {
            match block {
                DemoDocumentDemoBlock::Paragraph(document) => {
                    let result = engine.layout(document.input.clone());
                    assert_replayable(&catalog, &result, &document);
                }
                DemoDocumentDemoBlock::NarrowParagraph {
                    document,
                    max_width,
                } => {
                    let mut input = document.input.clone();
                    input.constraints = LayoutConstraints::with_defaults(max_width);
                    let result = engine.layout(input);
                    assert_replayable(&catalog, &result, &document);
                }
                DemoDocumentDemoBlock::ListItem { marker, body } => {
                    let marker_result = engine.layout(marker.input.clone());
                    let body_result = engine.layout(body.input.clone());
                    assert_replayable(&catalog, &marker_result, &marker);
                    assert_replayable(&catalog, &body_result, &body);
                }
                DemoDocumentDemoBlock::Section { .. } => {}
            }
        }
    }

    #[test]
    fn formal_sample_uses_span_selected_font_faces() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog);
        let document = build_document_demo(640.0, 1.0);
        let mut expected_faces = vec![
            ("黑体", 0, "demo-cjk@wght=400"),
            ("宋体", 0, "demo-serif@wght=400"),
            ("sans-serif", 0, "demo-latin@wght=400"),
            ("serif", 1, "demo-serif@wght=400"),
            ("monospace", 0, "demo-monospace@wght=400"),
            ("editorial-notes.md", 0, "demo-monospace@wght=400"),
            ("👩🏽‍💻", 0, "demo-emoji@wght=400"),
            ("👨‍👩‍👧‍👦", 0, "demo-emoji@wght=400"),
            ("🇨🇳", 0, "demo-emoji@wght=400"),
            ("1️⃣", 0, "demo-emoji@wght=400"),
            ("✈️", 0, "demo-emoji@wght=400"),
            ("office affinity waffle", 0, "demo-garamond@wght=400"),
            ("-> <= := != === //", 0, "demo-monospace@wght=400"),
        ];

        for block in document.blocks {
            let documents = match block {
                DemoDocumentDemoBlock::Paragraph(document) => vec![document],
                DemoDocumentDemoBlock::NarrowParagraph { document, .. } => vec![document],
                DemoDocumentDemoBlock::ListItem { marker, body } => vec![marker, body],
                DemoDocumentDemoBlock::Section { .. } => Vec::new(),
            };
            for document in documents {
                let layout = engine.layout(document.input);
                expected_faces.retain(|(text, occurrence, expected_face)| {
                    let Some((byte_start, _)) = layout
                        .input
                        .content
                        .text
                        .match_indices(text)
                        .nth(*occurrence)
                    else {
                        return true;
                    };
                    let range_start = layout.input.content.text[..byte_start]
                        .encode_utf16()
                        .count() as i32;
                    let range_end = range_start + text.encode_utf16().count() as i32;
                    let decisions: Vec<_> = layout
                        .debug
                        .shaping_decisions
                        .iter()
                        .filter(|decision| {
                            decision.range.start() >= range_start
                                && decision.range.end() <= range_end
                        })
                        .collect();
                    let matches_expected_face = !decisions.is_empty()
                        && decisions.iter().all(|decision| {
                            decision.resolved_face.as_deref() == Some(*expected_face)
                        });
                    !matches_expected_face
                });
            }
        }

        assert!(
            expected_faces.is_empty(),
            "missing span-selected faces: {expected_faces:?}"
        );
    }

    #[test]
    fn default_window_sample_exhibits_hanging_punctuation_and_hyphenation() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog);
        let document = build_document_demo(672.0, 1.0);
        let mut hanging_was_seen = false;
        let mut hyphenation_was_seen = false;

        for block in document.blocks {
            match block {
                DemoDocumentDemoBlock::Paragraph(document) => {
                    let is_mixed_sample =
                        document.input.content.text.starts_with("中文书刊经常夹用");
                    let layout = engine.layout(document.input);
                    if is_mixed_sample {
                        hyphenation_was_seen = layout
                            .lines
                            .iter()
                            .any(|line| !line.hyphen_glyphs.is_empty());
                    }
                }
                DemoDocumentDemoBlock::NarrowParagraph {
                    document,
                    max_width,
                } => {
                    let is_hanging_sample =
                        document.input.content.text.as_str() == "校样排印，宜留呼吸。";
                    let is_hyphenation_sample = document
                        .input
                        .content
                        .text
                        .starts_with("术语 internationalization");
                    let mut input = document.input;
                    input.constraints = LayoutConstraints::with_defaults(max_width);
                    let layout = engine.layout(input);
                    if is_hanging_sample {
                        hanging_was_seen = layout
                            .lines
                            .iter()
                            .any(|line| line.hanging_punctuation_advance > 0.0);
                    }
                    if is_hyphenation_sample {
                        hyphenation_was_seen = layout
                            .lines
                            .iter()
                            .any(|line| !line.hyphen_glyphs.is_empty());
                    }
                }
                DemoDocumentDemoBlock::ListItem { .. } | DemoDocumentDemoBlock::Section { .. } => {}
            }
        }

        assert!(
            hanging_was_seen,
            "default-width punctuation sample must exhibit hanging punctuation"
        );
        assert!(
            hyphenation_was_seen,
            "default-width mixed sample must exhibit English hyphenation"
        );
    }

    #[test]
    fn narrow_demo_word_uses_hyphens_at_multiple_line_ends() {
        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog);
        let document = build_document_demo(672.0, 1.0);
        let mut input = document
            .blocks
            .into_iter()
            .find_map(|block| match block {
                DemoDocumentDemoBlock::NarrowParagraph { document, .. }
                    if document
                        .input
                        .content
                        .text
                        .starts_with("术语 internationalization") =>
                {
                    Some(document.input)
                }
                _ => None,
            })
            .expect("demo must contain the narrow hyphenation sample");
        input.constraints = LayoutConstraints::with_defaults(4.0 * input.text_style.font_size);
        let result = engine.layout(input);
        let hyphenated_lines = result
            .lines
            .iter()
            .filter(|line| !line.hyphen_glyphs.is_empty())
            .count();

        assert!(
            hyphenated_lines >= 2,
            "expected multiple hyphenated line ends: {:?}",
            result
                .lines
                .iter()
                .map(|line| (line.range, line.hyphen_advance))
                .collect::<Vec<_>>()
        );
    }
}
