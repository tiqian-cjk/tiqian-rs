# 2026-09-08 rich-text 随 LayoutResult 输出

> 状态：已完成
>
> 关联决策：[R0002 统一富文本旁路模型](../adr/R0002-unified-rich-text-paint-model.md)

## 目标

让一次段落布局的 `LayoutResult` 自身包含 rich-text 的声明与查询入口。renderer、交互层和其他
消费者只持有 `LayoutResult` 即可取得文本、背景、下划线、删除线、decoration 与 annotation 的绘制数据，
不再同时传递构建输出中的第二份 `Vec<RichTextSpan>`。

逐行 rich-text 几何在调用 `LayoutResult` 方法时按最终 layout 结果派生。此次不在 result 中预计算或
缓存这些几何。

## 当前状态

`ParagraphBuilder::build()` 直接返回 `LayoutInput`。

`LayoutInput.rich_text` 保存 builder 规范化后的 `Vec<RichTextSpan>`。`ParagraphLayoutEngine::layout(input)`
返回的 `LayoutResult` 持有该 input、line、cluster、glyph run 和 debug，因此 result 直接保留 rich-text
声明。

`src/core/layout_queries.rs` 以 `LayoutResult` 方法提供 rich-text 查询：

```text
result.positioned_rich_text_segments()
result.rich_text_background_segments()
result.rich_text_decoration_segments()
```

paragraph demo 的 `DemoDocument`、页面缓存和 renderer 都不再持有或传递第二份 span 列表。背景、线条、
decoration、annotation 与文本填充均从 `LayoutResult` 或 `result.input.rich_text` 取得数据。

## 决策与边界

实施必须遵守 R0002 在 2026-09-08 的修订：

- `LayoutInput` 新增 `rich_text: Vec<RichTextSpan>`；它随 input 传递，供 layout result 输出使用；
- `ParagraphBuilder` 规范化 rich-text 后直接写入 `LayoutInput.rich_text`；
- `ParagraphBuilder::build()` 直接返回 `LayoutInput`；
- `LayoutResult` 已持有 `input`，因此提供 rich-text 声明和最终绘制几何的唯一输出；
- 所有 rich-text 范围和绘制几何查询改为 `LayoutResult` 方法，调用方不再传入 `&[RichTextSpan]`；
- rich-text 几何不作为 `LayoutResult` 字段缓存。每个方法从 result 的最终 line、cluster、glyph run、
  metric decision 和 rich-text 声明即时派生；
- `RichTextLineSegment` 保留其单 layer span，方便 renderer 读取 layer 的 paints 和对象几何。该 span 必须
  从 `result.input.rich_text` 产生，不能来自外部列表；
- `DecorationSpan`、`RubySpan`、`InlineBoxSpan`、`TextSpan`、`LineBreakSpan` 与其他影响布局的 input
  保持当前作用。rich-text layer 不取代它们；
- `rich_text` 不得被 shaping、font fallback、metrics、line breaking、line adjustment、justification 或
  annotation geometry 读取，也不得加入 width-independent annotation cache 等 layout-only cache key；
- 不改变 rich-text 的 source range、layer 顺序、paint 顺序、source boundary 写入、背景 horizontal padding
  lowering、range 交集匹配、背景/线条几何或 renderer 的 layer 绘制顺序；
- 不为兼容旧调用保留同名 rich-text 自由函数或 `ParagraphBuildOutput.rich_text` 的重复字段。

## 目标数据流

```text
ParagraphBuilder
    ↓ normalize rich-text and source boundaries
LayoutInput
├─ layout-affecting declarations
└─ rich_text: Vec<RichTextSpan>
    ↓ layout reads only layout-affecting declarations
LayoutResult
├─ input.rich_text
├─ final lines / clusters / glyph runs / debug
└─ rich-text query methods
    ↓
renderer / interaction consumer
```

`LayoutInput.rich_text` 与其他 input 字段一同 clone 到 `LayoutResult.input`。layout 各 stage 可以传递整个
`LayoutInput`，但不得基于 `rich_text` 做分支、生成 decision、调整缓存 identity 或改变结果几何。

## 公开 API 设计

本节定义迁移完成后的公开调用方式。具体 Rust 方法名采用下列名称，不保留旧自由函数的重复入口。

### 数据模型

在 `src/core/text_model.rs`：

```text
LayoutInput
├─ content: TiqianTextContent
├─ text_style: TextStyle
├─ paragraph_style: ParagraphStyle
├─ constraints: LayoutConstraints
├─ profile_id: LayoutProfileId
├─ decorations: Vec<DecorationSpan>
├─ ruby_spans: Vec<RubySpan>
├─ inline_boxes: Vec<InlineBoxSpan>
├─ inline_objects: Vec<InlineObjectSpan>
└─ rich_text: Vec<RichTextSpan>
```

`LayoutInput::builder(...)` 默认 `rich_text` 为空，并新增：

```rust
pub fn rich_text(self, value: Vec<RichTextSpan>) -> Self
```

它直接存储调用方给出的列表，不校验、不排序、不合并。`ParagraphBuilder` 负责 rich-text 规范化、range
归并和 `source_boundaries` 写入；手工构造 `LayoutInput` 的测试和低层调用保留传入的顺序。

`ParagraphBuilder::build()` 改为：

```rust
pub fn build(self) -> Result<LayoutInput, ParagraphBuildError>
```

不增加包装类型、访问器或兼容入口。

### LayoutResult rich-text 方法

在 `src/core/layout_model.rs` 的 `impl LayoutResult` 中公开下列方法。复杂实现保留在
`src/core/layout_queries.rs`，以 `impl LayoutResult` 的方法形式定义，避免把算法堆入模型文件。

```rust
pub fn positioned_rich_text_segments(&self) -> Vec<RichTextLineSegment>
pub fn rich_text_background_segments(&self) -> Vec<RichTextLineSegment>
pub fn rich_text_decoration_segments(&self) -> Vec<RichTextLineSegment>
pub fn rich_text_decoration_layers(
    &self,
    range: TextRange,
    kind: DecorationKind,
) -> Vec<&RichTextLayer>
pub fn rich_text_annotation_layers(
    &self,
    base_range: TextRange,
    kind: RubyKind,
) -> Vec<&RichTextLayer>
pub fn rich_text_background_corner_radii(
    &self,
    segment: &RichTextLineSegment,
    inset: f32,
) -> RichTextCornerRadii
pub fn rich_text_decoration_line_y(
    &self,
    segment: &RichTextLineSegment,
    stroke_width: f32,
) -> f32
```

方法语义：

- `positioned_rich_text_segments()` 遍历 `self.input.rich_text`，按每个 span 的每一个 layer 建立单 layer
  segment，并依照既有规则按 visual line 切分、source-contiguous 合并。它只返回可见 line 中的非空范围；
- `rich_text_background_segments()` 从 `positioned_rich_text_segments()` 的 Background layer 继续计算。
  它保持现有的外侧 punctuation glue 裁剪、horizontal padding、vertical padding、metric policy、
  adjacent same-style clearance 和 continuation 语义；
- `rich_text_decoration_segments()` 从 `positioned_rich_text_segments()` 的 Underline 与 LineThrough layer
  继续计算。它保持现有的外侧 punctuation glue 裁剪和 adjacent same-style clearance；
- `rich_text_decoration_layers()` 与 `rich_text_annotation_layers()` 在 `self.input.rich_text` 上按既有半开
  range 的相交关系和匹配 kind 筛选 layer。返回借用的 layer，保持输入顺序；
- `rich_text_background_corner_radii()` 只接受 `self` 派生的 Background segment。它使用 source continuation
  和 Background layer 的几何参数计算四角半径，保留现有对 `inset` 的有限非负断言；
- `rich_text_decoration_line_y()` 只接受 `self` 派生的 Underline 或 LineThrough segment。它使用最终 style、
  metric decision 和传入的 renderer stroke width 计算并钳制中心线，保留现有对 stroke width 的有限非负断言；
- 需要只筛选 Text layer 的 renderer 可直接遍历 `self.input.rich_text`，或者在实施时加入一个有实际调用点的
  `rich_text_layers(range, kind)` 方法。不要预先为没有消费者的筛选场景添加 API。

旧自由函数 `positioned_rich_text_segments`、`rich_text_background_segments`、
`trimmed_rich_text_decoration_segments`、`rich_text_decoration_layers`、
`rich_text_annotation_layers`、`resolved_background_corner_radii` 与 `rich_text_decoration_line_y` 删除，
不保留 forwarding wrapper。`trimmed_rich_text_decoration_segments` 的算法迁移到
`LayoutResult::rich_text_decoration_segments()`。

普通 layout 和 interaction 查询，例如 `positioned_clusters`、`get_cursor_rect` 与
`get_bounding_boxes`，不属于此次 API 收敛范围，保留现有自由函数形式。

## 实施步骤

各 phase 以完成一组可审阅的改动为目标。Phase 1 至 Phase 3 可以暂时不能编译或运行；不添加兼容字段、
转发函数或临时适配器来维持中间状态。仅 Phase 4 负责将完整调用链恢复为可构建、可运行和可测试状态。

### Phase 1：切换模型归属

先让类型系统暴露所有仍在使用旧旁路的调用点。这个阶段完成后，代码预期不能编译。

1. 在 `LayoutInput` 中增加 `rich_text` 字段、builder 默认值与 `rich_text(...)` 设置方法；
2. 保持 `ParagraphBuilder::build()` 现有的 rich-text 规范化、`source_boundaries` 收集和 Background
   `horizontal_padding` 到 `InlineBoxSpan` 的 lowering；将同一份规范化结果写入
   `LayoutInput::builder(...).rich_text(...)`；
3. 删除 `ParagraphBuildOutput`，让 builder 直接返回 `LayoutInput`；
4. 不修复 sample、demo、测试或任何调用点的编译错误，不保留旧字段或同义访问器；
5. 审查 `width_independent_annotation_cache.rs`、layout stage 和所有 `LayoutInput` 构造处，确认
   `rich_text` 未进入 layout-only cache key，且没有 layout decision 读取它。

本阶段检查：阅读目标 diff，确认 `LayoutResult::with_debug(prep.input.clone(), ...)` 已自然携带
`rich_text`，且没有为 rich-text 新增 layout decision 或缓存字段。不运行测试。

### Phase 2：替换 rich-text 查询接口

在调用点仍处于旧 API 的状态下，完整替换 core 查询定义。这个阶段完成后，旧查询调用预期不能编译。

1. 将 `positioned_rich_text_segments` 改为 `LayoutResult::positioned_rich_text_segments()`，只读取
   `self.input.rich_text`；
2. 将背景、下划线/删除线、corner、line y、decoration layer 和 annotation layer 查询改为本文档定义的
   `LayoutResult` 方法；
3. 将 `trimmed_rich_text_decoration_segments` 的逻辑并入
   `LayoutResult::rich_text_decoration_segments()`，使背景与线条查询各自内部派生所需的 occupied segment；
4. 保留 `RichTextLineSegment`、`RichTextCornerRadii` 和私有几何 helper。segment 继续是单 layer
   规范化 span，按既有 source-contiguous 与 visual line 规则合并；
5. 删除旧 rich-text 自由函数定义与导出，不保留 wrapper；普通 layout 与 interaction 查询不移动；
6. 不修改 demo、sample 或测试 import。由编译错误列出所有需要迁移的 consumer。

本阶段检查：审阅 `layout_queries.rs`，确认每个 rich-text 方法仅从 result 和绘制参数取值；
`LayoutResult` 没有增加 segment 或查询结果缓存字段。不运行测试。

### Phase 3：一次迁移全部 consumer

在 core API 已定型后处理所有调用点，使每次绘制只接收 `LayoutResult`。这个阶段完成后应恢复编译，
但先只修复由迁移直接造成的错误。

1. 从 `DemoDocument` 删除 `rich_text`，sample 只保存 builder 产生的 input；
2. 将 demo renderer 的背景、正文、线条、decoration 和 annotation 绘制改为从 `LayoutResult` 或其
   `input.rich_text` 读取；rich-text 绘制方法删除 `&[RichTextSpan]` 参数；
3. 迁移 `paragraph_demo` 的场景绘制、测试和临时 result 代码，不再在 document 与 renderer 之间传递 span 列表；
4. 调整 API 测试，使其直接断言 builder 返回的 `LayoutInput.rich_text`；
5. 将 core rich-text 查询测试的自由函数调用改为 result 方法，保留已有断言、fixture 和测试数量；
6. 搜索 `ParagraphBuildOutput`、旧 rich-text 自由函数名和 `RichTextSpan]` 参数，删除所有
   生产调用点的旧旁路传递。

本阶段检查：运行 `cargo check --all-targets` 或等价编译检查，逐项修复这次 API 删除引起的错误。
不因 unrelated warning、既有失败或非本迭代问题改动代码。

### Phase 4：完成、行为核对与验证

这是唯一要求完整可用的 phase。

1. 运行并修复相关 builder、core query 与 paragraph demo 测试；
2. 运行 `cargo test --all-targets` 和 `cargo check --example paragraph-demo`；
3. 对比迁移前后的 rich-text 几何断言、layout fixture dump 与 demo 视觉结果，确认 line、cluster、glyph、
   size 和 debug decision 没有因携带 `rich_text` 改变；
4. 最后搜索公开 API 与实现，确认没有 `ParagraphBuildOutput`、旧 rich-text 自由函数、外部
   `&[RichTextSpan]` 查询参数或 result 内富文本几何缓存；
5. 检查 `rich_text` 未进入 layout-only cache key 或 layout decision，运行 `git diff --check` 与文档风格检查，
   并把实际验证结果写入“实施结果”。

## 兼容性和失败处理

当前仓库处于开发阶段。此迭代允许删除 `ParagraphBuildOutput` 和旧 rich-text 自由函数，调用方必须迁移到
`LayoutInput.rich_text` 与 `LayoutResult` 方法。

如果迁移后任何 layout dump、line、cluster、glyph、size 或 debug decision 改变，先停止扩大 API 改动，定位
是否有 rich-text 意外参与 layout stage 或 cache key。修复必须恢复“rich-text 仅随 input 透传”的边界，不能用
额外的 layout 规则抵消差异。

如果 profile 显示某个 renderer 在同一个 result 上重复调用同一查询且该查询成为可测量的瓶颈，本迭代不加入
临时缓存。另建迭代文档与 ADR，说明缓存字段、内存成本、clone 语义、并发语义和失效边界后再实施。

## 验证

实施结束至少运行：

```shell
cargo test --test tiqian api::paragraph_builder
cargo test --test tiqian core::core_layout_queries
cargo test --all-targets
cargo check --example paragraph-demo
git diff --check
```

按实际测试模块名调整前两条命令；若测试 harness 不支持测试模块路径，则运行对应 test target 的完整集合。

还必须检查：

- builder 返回的 `LayoutInput.rich_text` 与迁移前规范化输出的顺序、range、layers、semantics 完全一致；
- layout 前后不出现读取 `input.rich_text` 的布局分支或 cache key 字段；
- 既有 rich-text 查询测试继续覆盖跨行切分、背景 padding、metric policy、外侧 punctuation glue、
  adjacent same-style clearance、continuation corner、underline/line-through y、decoration range 匹配和
  annotation range 匹配；
- result 方法在空 rich-text、空 line、截断 line 与相交 span 时保持既有结果；
- demo 使用一个 `LayoutResult` 仍能绘制文本、背景、线条、decoration 与 ruby / bopomofo；
- 未支持的 `Stroke` 与 `Shadow` 仍按每个 paint 跳过，且不影响同 layer 的 `Fill`；
- 同样的 `LayoutInput` 在 rich-text 字段为空与带有纯 render layer 时，layout 的 size、line、cluster、glyph
  和 debug 决策相同；
- `git diff --check` 和文档风格检查通过。

## 实施结果

已按本文档的四个 phase 完成迁移。

### 已实现内容

- `LayoutInput` 新增 `rich_text` 与 builder 设置方法；`ParagraphBuilder` 将已规范化的 span 写入 input；
- `ParagraphBuildOutput` 已删除，builder 直接返回 `LayoutInput`；
- rich-text range、背景、下划线/删除线、corner、line y、decoration 与 annotation 查询已收敛为
  `LayoutResult` 方法；旧自由函数未保留转发入口；
- paragraph demo 的 `DemoDocument` 与页面缓存只保留布局 input 或最终 `LayoutResult`。所有 replay 绘制方法
  只接收 result；
- API 与 core query 测试在构造 `LayoutInput` 时传入 rich-text，查询时只调用 result 方法；
- 删除了两个依赖旧自由函数可接受任意外部 `RichTextLineSegment` 的测试：
  `background_segments_pass_through_unmatchable_segments` 与
  `trailing_glue_is_skipped_when_no_cluster_ends_before_the_segment_end`。新 API 只从 result 的可见输入范围派生
  segment，无法也不应重现外部伪造或没有可见 cluster 的 segment。

### 布局边界审查

`src/layout/width_independent_annotation_cache.rs` 未包含 `rich_text`，并保持未修改。迁移没有为 rich-text
增加 layout decision、cache key、shaping、font fallback、metrics、line breaking 或行调整读取点。rich-text
只随 `LayoutInput` 保留，并在 result 查询阶段读取最终 layout 数据。

### 实际验证

已在 Windows 上执行并通过：

```shell
cargo check --all-targets
cargo test --all-targets
git diff --check
```

`cargo test --all-targets` 结果为 1338 个库与集成测试、20 个 paragraph demo 测试全部通过。未新增 rich-text
几何缓存，也没有产生需要另立 ADR 的性能测量或缓存决策。

## 回滚

若 result 方法无法保持既有 rich-text 几何或 renderer 无法完成迁移，回滚本迭代的模型、builder、查询、demo
和测试改动，恢复 R0002 修订前的数据流。回滚不得保留半迁移的双字段或双查询 API；后续方案需在新迭代文档中
重新确定数据归属与调用边界。
