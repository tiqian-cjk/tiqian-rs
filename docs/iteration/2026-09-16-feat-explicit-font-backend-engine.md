# 2026-09-16 显式字体后端引擎入口

> 状态：已完成
>
> 上游依据：Kotlin 当前保留 `ExplainableStubParagraphLayoutEngine` 的历史名称；本迭代不改变
> Kotlin 排版算法；本迭代收敛 Rust 的生产入口与测试支持边界。

## 目标

将生产段落引擎与确定性 fixture 实现分离。生产代码不再以 stub 字体后端作为默认依赖；调用方必须
显式提供实现 `FontBackend` 的平台字体后端。测试、fixture 和 golden 使用位于 `tests/support/` 的
确定性 backend。

完成后，Rust 的依赖方向为：

```text
src/
  ParagraphLayoutEngineBuilder + FontBackend

examples/
  DemoFontCatalog -> ParagraphLayoutEngineBuilder

tests/
  support::DeterministicStubFontBackend -> ParagraphLayoutEngineBuilder
```

`src/` 不依赖 `tests/`；生产库和示例不再构造测试 stub。

## 当前问题

当前 `ExplainableStubParagraphLayoutEngine::default()` 在库代码中直接创建
`DeterministicStubFontBackend`。该 backend 不读取平台字体资源，不执行 OpenType shaping，也不提供 glyph
ink bounds，却同时被 README 作为默认使用入口、被 benchmark 构造，并被桌面 demo 作为替换平台 backend 前的
初始实例。

这使测试 fixture 的实现进入了生产 crate 的默认调用路径，也让调用方容易误把 stub 几何当作可用于实际
渲染的字体结果。根 `tests/` 不能作为直接移动目标，因为 Rust integration test crate 依赖 `src/`，
而 `src/` 不能反向依赖 `tests/`。

## 范围

### 包含

- 在 `api` 模块新增 `ParagraphLayoutEngineBuilder`，显式接收一个 `Box<dyn FontBackend>`，并以现有
  内置 profile、metrics normalizer、贪心断行器、justifier、hyphenator 与 annotation cache 组装生产
  `ParagraphLayoutEngine`；
- 以 `ParagraphLayoutEngine` 替换现有 `ExplainableStubParagraphLayoutEngine` 作为生产引擎实现；
- 将 `DeterministicStubFontBackend` 从 `src/shaping/` 移至 `tests/support/`；
- 在 `tests/support/` 提供确定性 backend，供 fixture、golden 和 integration test 传入生产 builder；
- 迁移 README、开发指南、fixture runner、benchmark 和 desktop demo：平台字体运行路径直接通过 builder 注入
  `DemoFontCatalog`，测试路径显式使用测试 support；
- 保持 layout、字体选择、shaping、metrics、断行、缓存键、fixture 输入和 golden 结果不变。

### 不包含

- 改变 `FontBackend`、`FontMetricsRequest`、`FontFaceId` 或候选尝试语义；
- 改变 CLREQ 规则、字体 role 分类、display substitution、断行、行调整或 glyph replay；
- 将平台字体加载、HarfRust、SkRifa、Vello 或桌面 demo 代码移入 `src/`；
- 为发布版 crate 增加平台字体后端或默认字体资源；
- 重写所有公开模块的可见性或一次性完成 API facade 的其他收敛工作。

## 设计

### 生产引擎由调用方显式组装

`ParagraphLayoutEngineBuilder` 位于 `api` 模块。它至少要求调用方提供 `FontBackend`，并为当前已有的
生产默认策略提供明确默认值：大陆横排 profile、`ScriptAwareFontMetricsNormalizer`、
`GreedyLineBreaker`、`Justifier`、默认 hyphenator 与 LRU annotation cache。

builder 的 `build()` 返回唯一的 `ParagraphLayoutEngine` 实现，且保留当前的完整 pipeline：

```text
LayoutInput
  -> FontBackend::shape
  -> FontBackend::metrics
  -> line planning / adjustment
  -> LayoutResult
```

生产引擎不实现无参 `Default`，因此无法在没有平台 `FontBackend` 的情况下构造。

### 测试 support 保留确定性完整路径

`tests/support/` 仅持有 `DeterministicStubFontBackend`。fixture、golden、断言和覆盖率测试将这个 backend
传入 `ParagraphLayoutEngineBuilder`，与生产调用方使用同一引擎类型和组装路径，不复制 paragraph layout
pipeline，也不引入测试专用 engine 或 builder。

fixture runner 与测试辅助 backend 均从 `tests/support/` 导入。测试 support 不通过 `pub mod` 进入
发布 crate，也不被 README 或生产 benchmark 使用。

### 示例与 benchmark 直接使用平台 backend

桌面 demo 和 `paragraph-layout-bench` 在创建生产引擎时直接向 builder 传入 `DemoFontCatalog`。不再先创建
stub engine 再替换 `font_backend`。README 示例不提供无法产生 glyph 证据的无参 engine；它展示
`FontBackend` 注入位置，并链接 desktop demo 作为可运行的平台字体实现。

## 兼容性与迁移

当前 crate 仍处于开发阶段，不保留 `ExplainableStubParagraphLayoutEngine::default()`、其可替换公有字段或
测试 stub 的发布 API 兼容层。测试名称和 fixture 文案可以继续使用“stub”来描述确定性输入，但不得把它们
导出为普通产品入口。

本迭代不改变 `LayoutInput`、`LayoutResult`、`FontBackend` 或 layout 行为。预期 fixture golden 无差异；
若出现 golden diff，必须逐项证明其并非由本迭代的模块移动引入，否则停止并修复。

## 实施阶段

各 phase 只保证依赖关系清晰，不要求单独通过编译或测试。迁移期间可以暂时移除旧入口，直到全部
调用方完成迁移后再统一恢复构建与验证。

### Phase 1：切换生产引擎所有权

1. 将当前段落引擎实现改为唯一的 `ParagraphLayoutEngine`，保留现有 layout pipeline 与内部策略字段；
2. 新增 `ParagraphLayoutEngineBuilder`，以 `FontBackend` 为必填依赖，并在 builder 内组装既有生产默认策略；
3. 移除 `ExplainableStubParagraphLayoutEngine`、无参 `Default` 与生产引擎对 deterministic stub 的引用；
4. 在 `api` facade 导出 builder，并为必填 backend 与默认策略添加定向测试。

本阶段结束时允许现有示例和测试仍引用已删除的旧入口。生产引擎的类型、构造入口和依赖方向必须先固定，
避免在大量调用点迁移时继续改变目标 API。

### Phase 2：迁移所有调用方

1. 将 `DeterministicStubFontBackend` 移至 `tests/support/`，供测试直接传入生产 builder；
2. 迁移 fixture runner、golden、integration test 与测试 transform backend；直接验证内部 stage 的测试在
  `tests/` 中显式构造该 stage 所需依赖，不通过 engine 私有字段取得依赖；
3. 删除 `src/shaping/stub_font_backend.rs` 和对应库模块导出；
4. 迁移 desktop demo 与 benchmark，使其直接以 `DemoFontCatalog` 构造生产引擎；
5. 搜索并删除全部旧 engine 名称、无参默认构造和直接替换 `font_backend` 的调用路径。

本阶段结束时依赖链应完整：生产代码与示例只构造显式 backend 的生产引擎，测试代码只从
`tests/support/` 取得确定性 backend 并传入 production builder。

### Phase 3：统一文档、验证与收尾

1. 更新 README、开发指南和 API 设计报告，删除旧 stub 默认入口说明；
2. 对照 R0004、关键差异与本迭代范围，检查公开 API、依赖方向和发布清单；
3. 运行 builder、fixture、golden、layout 与 demo 的相关验证，再运行 `cargo test --all-targets`、
  `cargo check --examples` 与 `git diff --check`；
4. 检查编辑器诊断和变更文件；确认测试 support 不会进入发布 crate；
5. 记录验证结果，将本文件和 ADR 的实施状态更新为完成。

只有本阶段完成时，代码必须可构建、全部相关测试必须可运行，且生产、示例、测试和文档入口必须一致。

## 验证与回滚

至少验证：

- 生产 builder 不能在未提供 `FontBackend` 时构造；
- desktop demo 和 benchmark 使用 `DemoFontCatalog`，不会构造确定性 stub；
- fixture 与 golden 仍使用同一套 deterministic metrics、advance、face identity 和 shaping decision；
- 生产与测试调用方通过同一 builder 进入同一段落 layout pipeline；
- fixture golden 无差异；
- 发布包仅包含 `src/` 和声明的资源，不包含 `tests/support/`。

如迁移改变 fixture golden、产生不同的布局 geometry，或无法避免 `src/` 依赖 `tests/`，撤回本迭代的
入口替换，保留当前统一 `FontBackend` contract 与既有测试证据，再重新确定生产 engine 的构造边界。

## 完成记录

- `ParagraphLayoutEngineBuilder::new(Box<dyn FontBackend>)` 已成为生产引擎的唯一构造入口；
- `DeterministicStubFontBackend` 已移至 `tests/support/`，`src/` 不再导出或引用该实现；
- fixture golden 的独立 integration test crate 通过 `#[path = "support/mod.rs"]` 复用测试 support；
- desktop demo 与 `paragraph-layout-bench` 直接将 `DemoFontCatalog` 传入 builder；
- README、开发指南和 API 设计报告已更新为显式 backend 接入方式；
- 已执行 `cargo check --examples` 与 `cargo test --all-targets --no-fail-fast --color never`；全部测试通过，fixture golden 无差异。
- `git diff --check` 与本次修改中文文档的风格检查通过；
- `cargo package --list --allow-dirty` 的清单不包含 `tests/support/`。
