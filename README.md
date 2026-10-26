# 提椠 Tíqiàn（Rust）

[English](https://github.com/tiqian-cjk/tiqian-rs/blob/main/README_EN.md)

[![crates.io](https://img.shields.io/crates/v/tiqian.svg)](https://crates.io/crates/tiqian)
[![docs.rs](https://docs.rs/tiqian/badge.svg)](https://docs.rs/tiqian)
[![GitHub](https://img.shields.io/github/stars/tiqian-cjk/tiqian-rs?style=flat&logo=github)](https://github.com/tiqian-cjk/tiqian-rs)

`tiqian-rs` 是[提椠 Tíqiàn](https://github.com/tiqian-cjk/tiqian) 核心的 Rust 实现。提椠是专注于中日韩文本的文字排印引擎，同时兼容拉丁、希腊、西里尔、婆罗米等文本的排版。

Rust 版本持续跟随上游迭代，并根据 Rust 的语言特性作优化与调整；与上游的有意差异见[关键差异](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/key-differences.md)。

crate 与窗口系统、GPU 及 UI 框架解耦，可用于任意自定义渲染和 UI 实现；字体资源、字形绘制与交互事件由调用方负责。

当前重点覆盖简体中文横排。

## 功能

- 中文断行与行调整：避头尾、标点挤压与拉伸、行尾悬挂、两端对齐、字距调整、字格量化、首行缩进
- 三种断行策略：贪心、前瞻与整段动态规划
- 空白与间距：CSS 风格空白模式、中西文自动间距、制表位、英文断词
- 字体：字体回退、字形整形、度量与仿粗仿斜；CJK、Latin、数字、标点与 emoji 的字体角色与整形接口
- 行间注与装饰：拼音、注音、着重号、示亡号、专名号、书名号
- 富文本：颜色、背景、下划线、删除线、链接、技术文本、行内代码
- 布局结果：字形重放、注音与装饰几何、结构化布局决策，以及范围几何、光标、选择、复制与命中测试
- 运行环境：默认 `std`，同时提供 `no_std + alloc` 配置

## 平台支持

| target | `std` | `no-std` | 验证方式 |
| --- | :---: | :---: | --- |
| `x86_64-pc-windows-msvc` | ✔ | ✔ | 构建、测试、桌面示例 |
| `x86_64-unknown-linux-gnu` | ✔ | ✔ | CI 构建与测试 |
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

上表是已验证过的常见目标。核心库只依赖 `core` 与 `alloc`，因而理论上也可用于其他具备这两项能力的目标平台。

## 安装

```toml
[dependencies]
tiqian = "0.8"
```

需要 `no_std` 时，关闭默认 feature 并启用 `no-std` 预设（仍需 `alloc`）：

```toml
[dependencies]
tiqian = { version = "0.8", default-features = false, features = ["no-std"] }
```

`debug-info` feature 会在布局结果中附带结构化决策，供诊断与测试使用。

## 使用

相比上游版本，Rust 版本的 API 接入路径更为简洁。

使用 `ParagraphBuilder` 按内容顺序构造段落。它会为文本样式、行间注、装饰和富文本生成 `LayoutInput`，并代为维护这些范围的源文本边界。

```rust
use tiqian::api::{
    ParagraphBuilder, ParagraphLayoutEngineBuilder, RubyAnnotation, TextStyleOverride,
};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::shaping::font_backend::FontBackend;

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

    // 字体后端由调用方提供，负责字体选择、字形整形与度量
    let backend: Box<dyn FontBackend> = todo!("见 crates/tiqian-demo/src/font_backend.rs");
    let mut engine = ParagraphLayoutEngineBuilder::new(backend).build();
    let result = engine.layout(input);

    println!(
        "{} × {}，共 {} 行",
        result.size.width,
        result.size.height,
        result.lines.len()
    );
    Ok(())
}
```

builder 要求显式提供 `FontBackend`：负责字体选择、字形整形、度量查询，以及字形重放所需的字体实例标识。确定性字体后端面向仓库测试，桌面示例使用 HarfRust、SkRifa 与 Vello。

`LayoutResult` 包含行、字簇、字形重放数据、注音与装饰几何，以及结构化的布局决策。调用方据此绘制字形、背景和装饰，也可以用布局查询实现选择、复制和命中测试。测量与绘制使用同一字体后端，避免重新整形造成几何差异。

所有源文本范围与查询偏移使用 Unicode 标量值；Rust 字符串的 UTF-8 字节偏移需先换算。手工构造底层输入时同样遵循这一约定。

## 文档

- [API 文档](https://docs.rs/tiqian)：公开模块、类型与方法。
- [开发与调试指南](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/dev-guide.md)：环境、测试、fixture、golden 与性能测量。
- [API 设计报告](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/api-design-report.md)：外部接入路径与 API 边界分析。
- [架构决策记录](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/adr/README.md)：架构与排版取舍。
- [关键差异](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/key-differences.md)：与 Kotlin 上游的有意差异。
- [上游同步状态](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/tracking.md)：跟进范围与同步记录。

## 开发

```shell
cargo test --all-targets
cargo run -p tiqian-demo --release
```

仓库是 Cargo workspace：`tiqian` 位于仓库根，`crates/` 下分别是集成测试与 golden、桌面示例、基准程序。桌面示例的字体由 Git LFS 托管，克隆后执行 `git lfs pull`。详细环境、测试与性能测量见[开发与调试指南](https://github.com/tiqian-cjk/tiqian-rs/blob/main/docs/dev-guide.md)。

## 贡献

欢迎提交 issue、测试与文档。涉及公共 API、排版规则或较大重构的改动，请先开 issue 说明使用场景与预期行为；排版行为变化需要附带测试，影响布局时同步更新 fixture 与 golden。提交标题使用 `type(scope): subject` 单行格式。

## 许可

提椠以 [Mozilla Public License 2.0](https://github.com/tiqian-cjk/tiqian-rs/blob/main/LICENSE) 发布。

桌面示例内置字体的许可声明位于 `crates/tiqian-demo/resources/fonts/LICENSE-*.txt`。
