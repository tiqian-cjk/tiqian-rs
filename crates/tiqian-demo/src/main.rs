use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod app;
mod font_backend;
mod renderer;
mod sample;

fn main() -> Result<(), String> {
    let catalog = font_backend::DemoFontCatalog::load()?;
    catalog.validate_demo_faces()?;
    let event_loop = winit::event_loop::EventLoop::new()
        .map_err(|error| format!("paragraph-demo event loop creation failed: {error}"))?;
    let mut app = app::DesktopParagraphDemo::new(catalog);
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("paragraph-demo event loop failed: {error}"))
}
