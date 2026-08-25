#[path = "paragraph_demo/font_backend.rs"]
mod font_backend;
#[path = "paragraph_demo/renderer.rs"]
mod renderer;
#[path = "paragraph_demo/sample.rs"]
mod sample;
#[path = "paragraph_demo/app.rs"]
mod app;

fn main() -> Result<(), String> {
    let catalog = font_backend::DemoFontCatalog::load()?;
    catalog.validate_demo_faces()?;
    let event_loop = winit::event_loop::EventLoop::new()
        .map_err(|error| format!("paragraph-demo event loop creation failed: {error}"))?;
    let context = softbuffer::Context::new(event_loop.owned_display_handle())
        .map_err(|error| format!("paragraph-demo softbuffer context creation failed: {error}"))?;
    let mut app = app::DesktopParagraphDemo::new(catalog, context);
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("paragraph-demo event loop failed: {error}"))
}
