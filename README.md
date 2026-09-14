# 提椠 Tíqiàn（Rust）

[English](README_EN.md)

[![crates.io](https://img.shields.io/crates/v/tiqian.svg)](https://crates.io/crates/tiqian)
[![docs.rs](https://docs.rs/tiqian/badge.svg)](https://docs.rs/tiqian)
[![GitHub](https://img.shields.io/github/stars/tiqian-cjk/tiqian-rs?style=flat&logo=github)](https://github.com/tiqian-cjk/tiqian-rs)

提椠是专注于中日韩文本的文字排印引擎，同时兼容拉丁、希腊、西里尔等文本的排版。`tiqian-rs` 是[提椠 Tíqiàn](https://github.com/tiqian-cjk/tiqian) 核心的 Rust 实现。

当前实现重点覆盖简体中文横排，并提供字体 fallback、字体度量、文本 shaping、中文断行、避头尾、标点空间、两端对齐、行间注、装饰和布局查询等核心能力。可用于任意自定义渲染和 UI 实现。

## 当前状态

Rust 版本持续跟随上游迭代，并根据 Rust 特性作一定的优化和改进。当前支持的能力请参考上游仓库。

若想要了解与上游版本的差异，请参考 [docs/key-differences.md](docs/key-differences.md)。

## 安装

在 Cargo 项目的 `Cargo.toml` 中加入：

```toml
tiqian = "0.1"
```

## 使用

相比于上游版本，Rust 版本的 API 接入路径更为简洁。

使用 `ParagraphBuilder` 按内容顺序构造段落。它会为文本样式、行间注、装饰和富文本等生成 `LayoutInput`，调用方不需要手动维护这些范围的源文本边界。

```rust
use tiqian::api::{ParagraphBuilder, RubyAnnotation, TextStyleOverride};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::layout::paragraph_layout_engine::{
	ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

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
	let mut engine = ExplainableStubParagraphLayoutEngine::default();
	let result = engine.layout(input);

	println!("段落大小：{} × {}", result.size.width, result.size.height);
	println!("行数：{}", result.lines.len());
	Ok(())
}
```

示例中的 `ExplainableStubParagraphLayoutEngine` 使用确定性的 stub shaping 和字体度量，适合快速试用、测试和排版行为验证。接入平台字体时，需要实现并注入 `FallbackResolver`、`FontMetricsResolver` 和 `TextShaper`；`examples/paragraph-demo.rs` 展示了使用 HarfRust、SkRifa 和 Vello 的桌面接入路径。

`LayoutResult` 包含行、cluster、glyph replay 数据、注音和装饰几何，以及结构化的布局决策。宿主应用可以据此绘制字形、背景和装饰，也可以使用布局查询实现选择、复制和命中测试。测量与绘制应使用同一字体后端，避免重新 shaping 造成几何差异。

所有 source range 和布局查询 offset 使用 Unicode scalar value，不使用 UTF-8 byte offset。手动构造底层输入时，需要遵守这一坐标约定。

## 能力范围

核心目前以简体中文横排为主要目标，支持：

- 中文正文的断行、避头尾、标点几何、字距调整和两端对齐；
- CJK、Latin、数字、标点和 emoji 的字体角色与 shaping 接口；
- 拼音、注音、着重号、示亡号、专名号和书名号；
- 颜色、背景、下划线、删除线、链接、技术文本和行内代码等富文本声明；
- 布局结果的 glyph 重放、范围几何、caret、选择、复制和命中查询。

Rust crate 提供排版核心和平台后端接口，不绑定窗口系统、GPU 或特定 UI 框架。宿主应用需要负责字体资源、字形绘制、交互事件和最终呈现。

## 文档

- [API 文档](https://docs.rs/tiqian)：公开模块、类型和方法。
- [开发与调试指南](docs/dev-guide.md)：开发环境、测试、fixture、golden 和性能测量。
- [API 设计报告](docs/api-design-report.md)：当前外部接入路径和 API 边界分析。
- [关键差异](docs/key-differences.md)：Rust 实现与 Kotlin 上游的有意差异。
- [上游同步状态](docs/tracking.md)：与 Kotlin 版本的同步范围和记录。

## 协议

提椠以 [Mozilla Public License 2.0](LICENSE) 发布。
