<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/tiqian_rust_pinyin_with_logo_white.svg">
    <img src="docs/images/tiqian_rust_pinyin_with_logo_black.svg" alt="提椠 Tíqiàn" height="48">
  </picture>
</h1>

[中文](https://github.com/tiqian-cjk/tiqian-rs/blob/main/README.md)

[![crates.io](https://img.shields.io/crates/v/tiqian.svg)](https://crates.io/crates/tiqian)
[![docs.rs](https://docs.rs/tiqian/badge.svg)](https://docs.rs/tiqian)
[![GitHub](https://img.shields.io/github/stars/tiqian-cjk/tiqian-rs?style=flat&logo=github)](https://github.com/tiqian-cjk/tiqian-rs)

`tiqian-rs` is the Rust implementation of the [Tíqiàn](https://github.com/tiqian-cjk/tiqian) core. Tiqian is a typesetting engine focused on CJK text that also handles Latin, Greek, Cyrillic, Brahmic and other scripts.

The Rust version follows upstream development while applying optimizations and adjustments suited to Rust; intentional differences are listed in [key differences](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/key-differences.md).

The crate is decoupled from window systems, GPUs, and UI frameworks, so it fits any custom renderer and UI implementation; font resources, glyph drawing, and interaction events stay with the caller.

The current focus is simplified Chinese horizontal writing.

## Features

- Chinese line breaking and adjustment: kinsoku rules, punctuation compression and expansion, hanging punctuation, justification, tracking, grid quantization, first-line indent
- Three line breakers: greedy, lookahead, and paragraph-wide dynamic programming
- Whitespace and spacing: CSS-style white space modes, automatic CJK/Latin spacing, tab stops, English hyphenation
- Fonts: font fallback, shaping, metrics, and synthetic bold and oblique; font roles and shaping interfaces for CJK, Latin, digits, punctuation, and emoji
- Ruby and decorations: pinyin, bopomofo, emphasis dots, mourning marks, proper-noun marks, book-title marks
- Rich text: colors, backgrounds, underline, line-through, links, technical text, inline code
- Layout result: glyph replay, ruby and decoration geometry, structured layout decisions, plus range geometry, caret, selection, copy, and hit testing
- Runtime: `std` by default, with a `no_std + alloc` configuration

## Platform support

| target | `std` | `no-std` | Verified by |
| --- | :---: | :---: | --- |
| `x86_64-pc-windows-msvc` | ✔ | ✔ | Build, test, desktop example |
| `x86_64-unknown-linux-gnu` | ✔ | ✔ | CI build and test |
| `armv7-unknown-linux-gnueabihf` | ✔ | ✔ | `cargo check` |
| `aarch64-apple-darwin` | ✔ | ✔ | `cargo check` |
| `aarch64-apple-ios` | ✔ | ✔ | `cargo check` |
| `aarch64-linux-android` | ✔ | ✔ | `cargo check` |
| `wasm32-unknown-unknown` | ✔ | ✔ | `cargo check` |
| `wasm32-wasip1` | ✔ | ✔ | `cargo check` |
| `thumbv7em-none-eabihf` | | ✔ | `cargo check` |
| `thumbv7m-none-eabi` | | ✔ | `cargo check` |
| `riscv32imac-unknown-none-elf` | | ✔ | `cargo check` |
| `x86_64-unknown-none` | | ✔ | `cargo check` |

The table lists common verified targets. The core crate depends only on `core` and `alloc`, so other targets with the same capabilities are expected to work.

## Installation

```toml
[dependencies]
tiqian = "0.8"
```

For `no_std`, disable default features and enable the `no-std` preset (still needs `alloc`):

```toml
[dependencies]
tiqian = { version = "0.8", default-features = false, features = ["no-std"] }
```

The `debug-info` feature adds structured decisions to the layout result for diagnosis and tests.

## Usage

Build a paragraph with `ParagraphBuilder` in content order. It produces a `LayoutInput` covering text styles, ruby, decorations, and rich text, and maintains the source boundaries for those ranges on the caller's behalf. Compared with the upstream version, the Rust API takes a shorter integration path.

```rust
use tiqian::api::{
    ParagraphBuilder, ParagraphLayoutEngineBuilder, RubyAnnotation, TextStyleOverride,
};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::shaping::font_backend::FontBackend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.push("Welcome to ");
    builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
        builder.styled(
            TextStyleOverride::builder().font_weight(700).build(),
            "提椠",
        );
    });
    builder.emphasis("CJK typesetting");
    let input = builder.build()?;

    // The font backend is provided by the caller and handles font selection, shaping, and metrics.
    let backend: Box<dyn FontBackend> = todo!("see crates/tiqian-demo/src/font_backend.rs");
    let mut engine = ParagraphLayoutEngineBuilder::new(backend).build();
    let result = engine.layout(input);

    println!(
        "{} × {}, {} lines",
        result.size.width,
        result.size.height,
        result.lines.len()
    );
    Ok(())
}
```

The builder requires an explicit `FontBackend`, which handles font selection, shaping, metrics, and the font instance identity needed for glyph replay. A deterministic backend serves repository tests, while the desktop example uses HarfRust, SkRifa, and Vello.

`LayoutResult` carries lines, clusters, glyph replay data, ruby and decoration geometry, and structured layout decisions. Callers draw glyphs, backgrounds, and decorations from it, and use the layout queries for selection, copy, and hit testing. Measurement and drawing share one font backend so that reshaping cannot shift geometry.

Source ranges and query offsets use Unicode scalar values; UTF-8 byte offsets of the Rust string are converted first. Hand-built low-level input follows the same convention.

## Documentation

- [API documentation](https://docs.rs/tiqian): public modules, types, and methods.
- [Development and debugging guide](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/dev-guide.md): environment, tests, fixtures, goldens, and performance measurement.
- [API design report](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/api-design-report.md): external integration paths and API boundary analysis.
- [Architecture decision records](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/adr/README.md): architecture and typesetting trade-offs.
- [Key differences](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/key-differences.md): intentional differences from the Kotlin upstream.
- [Upstream tracking](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/tracking.md): synchronization scope and records.

## Development

```shell
cargo test --all-targets
cargo run -p tiqian-demo --release
```

The repository is a Cargo workspace: `tiqian` lives at the repository root, and `crates/` holds integration tests with goldens, the desktop example, and benchmarks. Demo fonts are stored in Git LFS, so run `git lfs pull` after cloning. See the [development guide](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/dev-guide.md) for environment, testing, and performance measurement details.

## Contributing

Issues, tests, and documentation are welcome. Open an issue describing the use case and expected behavior before public API changes, typesetting rule changes, or large refactors. Behavior changes need tests, and layout changes also update fixtures and goldens. Use single-line `type(scope): subject` commit titles.

## License

Tiqian is released under the [Mozilla Public License 2.0](https://github.com/tiqian-cjk/tiqian-rs/blob/main/LICENSE).

Bundled demo fonts ship with their license notices in `crates/tiqian-demo/resources/fonts/LICENSE-*.txt`.
