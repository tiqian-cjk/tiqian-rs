# 提椠 Tíqiàn (Rust Port)

`tiqian-rs` 是 [提椠 Tíqiàn](https://github.com/tiqian-cjk/tiqian) 中文横排排版核心的 Rust 移植。当前使用确定性 stub shaping、stub font metrics 和 Rust 本地 layout fixture golden 验证移植行为。

## 开发环境

- Rust toolchain（Cargo，edition 2024）

## 段落构造

`api::ParagraphBuilder` 按文本顺序追加内容，并生成现有 `LayoutInput`、颜色和 rich-text 范围。调用方无需计算 `TextRange` 或维护纯绘制范围的 `source_boundaries`：

```rust
use tiqian::api::{ParagraphBuilder, RubyAnnotation, TextStyleOverride};
use tiqian::core::geometry::LayoutConstraints;

let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
builder.push("欢迎使用");
builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
	builder.styled(TextStyleOverride::builder().font_weight(700).build(), "提椠");
});
let output = builder.build()?;
```

将 `output.input` 传入现有 `ParagraphLayoutEngine`；当前 renderer 额外读取 `output.colors` 与 `output.rich_text` 重放颜色和富文本几何。

## Fixture 验证

本仓库在 `tests/fixture_layout/` 保存全部 52 项 deterministic stub fixture 与对应 golden。每项 fixture 都使用 greedy、lookahead、paragraph-DP 三种 breaker，并比较完整 layout decision dump。

运行本地 fixture golden 测试：

```shell
cargo test --test tiqian layout_fixture_golden_test
```

默认模式只读取 golden，缺少、额外或不匹配的 golden 都会失败。更新预期输出时显式设置环境变量：

```shell
TIQIAN_UPDATE_LAYOUT_GOLDENS=1 cargo test --test tiqian layout_fixture_golden_test
```

更新会重新生成全部 52 个 Rust 本地 golden，不会删除未知文件；结束时仍检查 fixture ID 与 golden 文件名集合相等。更新后逐项审阅文本 diff。Kotlin 的 fixture 定义和普通 golden 仅在上游同步时作为对照来源，不参与日常 Rust 测试。

## 本地检查

### 无界面布局性能测量

`paragraph-layout-bench` 复用 desktop demo 的复杂文字 sample 和 HarfRust 字体后端，
按固定宽度序列重复布局，无需窗口或 GPU。默认运行首次序列、20 轮预热和 200 轮测量，
每轮依次使用 672、360、960 物理像素宽度，缩放为 1。

```shell
cargo run --release --example paragraph-layout-bench
cargo run --release --example paragraph-layout-bench -- --iterations 1000 --warmup 50 --widths 672,360,960 --scale 1
```

输出按整份 sample 页面统计，`layout` 是所有 `engine.layout()` 的累计耗时，
包含 HarfRust shaping、字体度量与完整 debug 数据；`result_drop` 是结果集中析构时间；
`total` 还包含输入准备、列表宽度计算和输出数量统计。
各项报告 min、median、p95、mean、max（ms），同时检查预热后的调用数、行数、
cluster 数和正文 glyph 数与首次序列一致。首次序列的后续宽度会复用前面的引擎缓存。
比较优化前后时保持参数与构建配置一致，以各宽度的 `layout` median 为主要指标。

采样时使用 `cargo flamegraph --example paragraph-layout-bench -- --iterations 1000`。
火焰图覆盖整个进程，包含启动、预热、输入准备和析构；分析核心时筛选 `engine.layout`
的调用树。采样运行的耗时不与普通 release 运行直接比较。

### 编译与测试

Rust 侧常规编译与测试：

```shell
cargo check
cargo test
```

运行全部 Rust target 并生成 HTML 覆盖率报告：

```shell
bash tools/coverage.sh
```

报告入口位于 `target/llvm-cov/html/index.html`。覆盖率路径只依赖 Rust toolchain。
