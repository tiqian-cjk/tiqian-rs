# 开发与调试指南

本文面向 tiqian-rs 的开发者和维护者。

## 环境

- 任意支持 Rust 的操作系统，Windows 下需要 Bash 支持（WSL/MSYS2皆可）。
- Rust toolchain，建议使用最新版。

## 日常检查

在仓库根目录运行：

```shell
cargo check
cargo test
```

运行桌面段落示例：

```shell
cargo run --example paragraph-demo
```

示例使用 HarfRust、SkRifa 和 Vello 接入字体、字形 shaping 与绘制，展示平台接入方式；crate 默认使用 stub 字体后端。

## Fixture 与 golden

`tests/fixture_layout/` 保存 deterministic stub fixture 与完整 layout decision golden。日常回归测试使用 Rust 仓库内的 fixture，不依赖 Kotlin 仓库。

运行 fixture golden 测试：

```shell
cargo test --test tiqian layout_fixture_golden_test
```

需要确认布局预期发生变化时，显式更新 golden：

```shell
TIQIAN_UPDATE_LAYOUT_GOLDENS=1 cargo test --test tiqian layout_fixture_golden_test
```

更新后逐项检查文本 diff。更新模式会重新生成现有 fixture 的 golden，不会替代缺失的 fixture 或删除未知文件。

## 性能测量

`paragraph-layout-bench` 使用桌面示例的段落和平台字体后端，在不打开窗口的情况下测量布局：

```shell
cargo run --release --example paragraph-layout-bench
cargo run --release --example paragraph-layout-bench -- --iterations 1000 --warmup 50 --widths 672,360,960 --scale 1
```

比较实现时保持构建配置和参数一致，主要观察各宽度的 `layout` median。`--replay` 可以额外测量整页重放索引：

```shell
cargo run --release --example paragraph-layout-bench -- --replay
```

需要采样时使用 `cargo flamegraph --example paragraph-layout-bench -- --iterations 1000`。火焰图包含进程启动、预热、输入准备和析构；分析核心时筛选 `engine.layout` 调用树，不要把采样运行时间直接与普通 release 运行比较。

## 覆盖率

在提供 Bash 的环境中运行：

```shell
bash tools/coverage.sh
```

HTML 报告位于 `target/llvm-cov/html/index.html`。

## API 接入注意事项

- 优先使用 `api::ParagraphBuilder` 按内容顺序构造段落；`build()` 返回 `LayoutInput`。
- `TextRange` 和布局查询使用 Unicode scalar offset；UTF-8 byte offset 仅用于 Rust 字符串的底层索引。
- 默认的 `ExplainableStubParagraphLayoutEngine` 使用确定性 stub shaping 和字体度量，适合测试与行为验证。
- 接入平台字体时，实现一个 `FontBackend`，由它统一提供候选选择、完整 shaping、metrics 和可重放的 face identity。
	段落 shaping 的每个 backend request 都是独立的字体选择原子范围；同一个旧 range 被内部切分后，不应在调用方
	重新合并 `FontResolution`。桌面示例中的字体 backend 提供了参考实现。
- `LayoutResult` 同时包含行、cluster、glyph replay 数据和结构化 debug 信息。renderer 应重放布局结果，不应自行重新计算断行或标点几何。

## 文档与同步

涉及上游同步或架构取舍时，按以下顺序查阅并更新对应文档：

- `docs/tracking.md`：Kotlin 上游跟进状态与同步记录。
- `docs/key-differences.md`：Rust 与 Kotlin 的有意差异。
- `docs/adr/`：已经确定的架构决策。
- `docs/iteration/`：当前迭代的目标、设计和验证记录。

提交前至少运行 `git diff --check`，并确认只包含当前任务相关的改动。行为改变时，补充或更新相关测试；涉及布局行为时，同时检查 fixture golden。
