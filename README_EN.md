# Tíqiàn (Rust)

[中文](README.md)

[![crates.io](https://img.shields.io/crates/v/tiqian.svg)](https://crates.io/crates/tiqian)
[![docs.rs](https://docs.rs/tiqian/badge.svg)](https://docs.rs/tiqian)
[![GitHub](https://img.shields.io/github/stars/tiqian-cjk/tiqian-rs?style=flat&logo=github)](https://github.com/tiqian-cjk/tiqian-rs)

Tiqian is a typesetting engine for CJK text, with support for Latin, Greek, Cyrillic, and other scripts. `tiqian-rs` is the Rust implementation of the [Tíqiàn](https://github.com/tiqian-cjk/tiqian) core.

The current implementation focuses on simplified Chinese horizontal writing. It provides core capabilities including font fallback, font metrics, text shaping, Chinese line breaking, kinsoku rules, punctuation spacing, justification, interlinear annotations, decorations, and layout queries. It can be used with custom renderers and UI implementations.

## Current Status

The Rust version continues to follow upstream development while applying optimizations and improvements suited to Rust. For the currently supported capabilities, see the upstream repository.

For differences from the upstream version, see [docs/key-differences.md](docs/key-differences.md).

## Installation

Add the following dependency to your Cargo project's `Cargo.toml`:

```toml
tiqian = "0.1"
```

## Usage

Compared with the upstream version, the Rust version provides a more concise API integration path.

Use `ParagraphBuilder` to construct a paragraph in content order. It generates a `LayoutInput` containing text styles, interlinear annotations, decorations, rich text, and related data, so callers do not need to maintain source-text boundaries for these ranges manually.

```rust
use tiqian::api::{ParagraphBuilder, ParagraphLayoutEngineBuilder, RubyAnnotation, TextStyleOverride};
use tiqian::core::geometry::LayoutConstraints;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
	builder.push("欢迎使用");
	builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
		builder.styled(
			TextStyleOverride::builder().font_weight(700).build(),
			"提椠",
		);
	});
	builder.emphasis("中文排版");

	let input = builder.build()?;
	// Implemented by the host; one backend provides font selection, complete shaping, metrics, and replayable face identity.
	// See the demo for a reference implementation in DemoFontCatalog.
	let font_backend: Box<dyn tiqian::shaping::font_backend::FontBackend> = todo!();
	let mut engine = ParagraphLayoutEngineBuilder::new(font_backend).build();
	let result = engine.layout(input);

	println!("Paragraph size: {} × {}", result.size.width, result.size.height);
	println!("Line count: {}", result.lines.len());
	Ok(())
}
```

The builder requires an explicit `FontBackend`. The backend is responsible for font selection, complete shaping, metrics, and replayable face identity. Deterministic backends are used only in repository test support; the desktop demo path uses HarfRust, SkRifa, and Vello.

`LayoutResult` contains lines, clusters, glyph replay data, ruby and decoration geometry, and structured layout decisions. A host application can use it to draw glyphs, backgrounds, and decorations, as well as to implement selection, copying, and hit testing through the layout queries. Measurement and drawing should use the same font backend to avoid geometry differences caused by reshaping.

All source ranges and layout-query offsets use Unicode scalar values rather than UTF-8 byte offsets. Code that constructs lower-level input manually must follow this coordinate convention.

## Capabilities

The core currently targets simplified Chinese horizontal writing and supports:

- Chinese text line breaking, kinsoku rules, punctuation geometry, tracking, and justification;
- font roles and shaping interfaces for CJK, Latin, numbers, punctuation, and emoji;
- pinyin, bopomofo, emphasis marks, mourning marks, proper-noun marks, and book-title marks;
- rich-text declarations for colors, backgrounds, underlines, strike-through, links, technical text, and inline code;
- glyph replay, range geometry, caret, selection, copying, and hit-testing queries from layout results.

The Rust crate provides the typesetting core and platform backend interfaces without binding to a window system, GPU, or specific UI framework. Host applications are responsible for font resources, glyph drawing, interaction events, and final presentation.

## Documentation

- [API documentation](https://docs.rs/tiqian): public modules, types, and methods.
- [Development and debugging guide](docs/dev-guide.md): development environment, tests, fixtures, goldens, and performance measurement.
- [API design report](docs/api-design-report.md): the current external integration path and API boundary analysis.
- [Key differences](docs/key-differences.md): intentional differences between the Rust implementation and the Kotlin upstream.
- [Upstream tracking](docs/tracking.md): synchronization scope and records for the Kotlin version.

## License

Tiqian is released under the [Mozilla Public License 2.0](LICENSE).
