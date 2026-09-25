# 富文本范围身份与逐 cluster 几何

> 状态：已完成
>
> 日期：2026-09-25
>
> 分类：feat

## 目标

让 tiqian-rs 的布局结果同时支持三类消费需求：

1. **范围身份**：调用方为富文本范围与装饰范围声明可选数值 id，id 随布局结果与几何输出原样返回，使消费方能把属于同一个 authored range 的几何归为一组，不必按 source range 与 paint 值猜测。
2. **逐 cluster 几何**：提供逐 positioned cluster 的最终富文本 layer 几何与装饰几何（背景、下划线、删除线、示亡号、专名号、书名号），使消费方能按 cluster 增量绘制连续图形（逐字显示），不必自己切分逐行几何。
3. **输入分片边界声明**：提供 builder 入口，让调用方把输入分片边界声明为 source boundary，使最终 cluster 不跨越分片，从而让每个绘制元素只属于一个输入分片。

下游需求见 `huozi-rs` 的 `docs/iteration/2026-09-21-feat-rich-text-layout-and-sdf-rendering.md`：Huozi 按 positioned cluster 输出增量片段，并用 `continuity_id` 标识连续视觉范围。tiqian-rs 提供身份与逐 cluster 几何，Huozi 负责转成自己的公开类型。

## 现状

| 文件 | 现状 |
| --- | --- |
| `src/core/text_model.rs` | `RichTextSpan { range, layers, semantics }` 与 `DecorationSpan { range, kind }` 都没有身份字段；`RichTextLayer { kind, paints }` 也没有。 |
| `src/api/builder.rs` | `record_rich_text` 为每次 `push` 记录 span；`normalized_rich_text` 合并相邻且 layer 相同、语义为空的 span；`build()` 把 rich-text 范围端点并入 `TiqianTextContent.source_boundaries`。 |
| `src/api/scopes.rs` | 每个 scope 已带 `sequence: u64`，只用于排序，不进入输出。 |
| `src/layout/cluster_role_resolution.rs`、`src/layout/width_independent_annotation_cache.rs` | cluster 切分边界只在这里计算：它把 `content.source_boundaries` 与装饰、注音、行内盒、行内对象、断行范围端点并在一起。因此声明 boundary 会切分 cluster，装饰范围端点本来就会切分 cluster。 |
| `src/core/layout_queries.rs` | `positioned_rich_text_segments()` 按 cluster 给出 occupied 片段，但不做外缘修剪、padding、metric policy 与相邻同样式间隙；`rich_text_background_segments()`、`rich_text_decoration_segments()` 给出逐行最终几何，但不带身份也未切到 cluster。`slice_rect`、`segment_layer`、`trim_outer_punctuation_glue`、`with_adjacent_same_style_clearance` 都不对外公开。 |
| `src/layout/annotation_geometry_stage.rs` | `DecorationSegmentInfo`（示亡号、专名号、书名号，逐行）与 `DecorationDecisionInfo`（着重号，逐 cluster）都不带身份。 |
| `src/core/layout_result_replay_index.rs` | `LayoutResultReplayIndex` 缓存逐行查询结果，供渲染层一次构造。 |

Kotlin 上游的 `RichTextSpan`（单 role 旧模型）与 `DecorationSpan` 同样没有身份字段，也没有逐 cluster 的最终几何查询；本迭代属于 Rust 先行能力。

## 范围

### 包含

- `RichTextLayer` 增加 `id: Option<u32>`；
- `DecorationSpan` 增加 `id: Option<u32>`，并传播到 `DecorationSegmentInfo` 与 `DecorationDecisionInfo`；
- 装饰与行内代码 scope 的带 id 声明入口；
- `RichTextLayerClusterSegment` 与 `LayoutResult::rich_text_layer_cluster_segments()`；
- `DecorationClusterSegment` 与 `LayoutResult::decoration_cluster_segments()`；
- `ParagraphBuilder` 的 source boundary 声明入口；
- `LayoutResultReplayIndex` 纳入两个新查询结果；
- 上述内容的回归测试与既有 golden 中性验证。

### 不包含

- 改变断行、行调整、标点几何、纵向度量、注音避让与行内对象布局；
- 改变 `positioned_rich_text_segments()`、`rich_text_background_segments()`、`rich_text_decoration_segments()`、`rich_text_background_corner_radii()`、`rich_text_decoration_line_y()` 的语义与签名；
- 由 tiqian 分配或校验 id（id 由调用方声明，tiqian 不解释其含义）；
- 文字选择、光标与复制查询；
- 注音的逐 cluster 归属（消费方用 `base_range` 与 positioned cluster 求交即可，不新增 API）；
- 按 id 归组后重新解析圆角与相邻同样式间隙（已确定保持现状，见“待讨论问题”）。

## 设计

### 1. 范围身份

`RichTextLayer` 与 `DecorationSpan` 各增加一个可选数值身份：

```rust
pub struct RichTextLayer {
    pub kind: RichTextLayerKind,
    pub paints: Vec<RichTextPaint>,
    /// 调用方声明的 authored range 身份；None 表示未声明。
    pub id: Option<u32>,
}

pub struct DecorationSpan {
    pub range: TextRange,
    pub kind: DecorationKind,
    /// 调用方声明的 authored range 身份；None 表示未声明。
    pub id: Option<u32>,
}
```

语义：

1. 同一个 id 表示这些几何属于调用方的同一个 authored range。调用方负责分配；tiqian 不解释、不校验唯一性、不合并、不去重、不记录 warning。
2. 类型是 `u32`：调用方只需在单次布局内区分不同 authored range，数值足够，不需要字符串。派生的字符串 id 不属于本迭代。
3. id 不参与 shaping、断行、行调整、行内几何、缓存键与 debug decision。
4. 未声明 id 时，行为与现状一致。

**合并规则**：`normalized_rich_text` 的相邻合并条件是“语义为空 + layer 相等 + 范围相接”。`RichTextLayer` 派生 `PartialEq`，因此加入 id 后，相邻但 id 不同的范围不再合并；这正是需要的行为，两个相邻、同样式、来源不同的 authored range 必须保持为独立范围。同一 id 且 layer 相同的相邻范围继续合并为一个 span。

**几何传播**：`positioned_rich_text_segments()` 已按 layer 把每个 span 拆成只含该 layer 的 span，因此 id 自动随 `segment.span.layers[0].id` 进入所有逐行与逐 cluster 片段，不需要新增几何字段。

**装饰传播**：`DecorationSegmentInfo` 与 `DecorationDecisionInfo` 增加 `id: Option<u32>`，取值来自生成该几何的 `DecorationSpan`。着重号（Emphasis）也带 id，使 layer 家族与装饰家族对称。

**声明入口**：装饰与行内代码的几何由 scope 自身生成，需要新入口：

- `push_decoration_with_id(id, kind)`、`with_decoration_with_id(id, kind, content)`、`try_with_decoration_with_id(...)`；
- `push_inline_code_with_id(id, style, background)`、`with_inline_code_with_id(...)`、`try_with_inline_code_with_id(...)`；
- 既有 `push_decoration`、`with_decoration`、`push_inline_code`、`with_inline_code` 签名不变，等价于 id = `None`。

背景、下划线、删除线与通用 rich-text scope 不需要新入口：调用方用 `with_rich_text`、`push_rich_text` 传入自行构造的 `RichTextLayer`（含 id）即可；这与 `with_background` 等便捷入口推送的 layer 相同。

### 2. 逐 cluster 的富文本 layer 几何

```rust
/// 一个 positioned cluster 上某个 rich-text layer 贡献的最终几何片段。
#[derive(Clone, Debug, PartialEq)]
pub struct RichTextLayerClusterSegment {
    /// 产生该片段的 layer；按 layer 拆分的约定与 positioned_rich_text_segments 一致。
    pub span: Arc<RichTextSpan>,
    /// 该片段在 positioned_clusters(result) 中的下标。
    pub cluster_index: i32,
    pub line_index: i32,
    /// 本 cluster 与该 layer 范围的交集。
    pub range: TextRange,
    /// 本片段覆盖的横向区间。
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    /// 该 layer 在其视觉行上的最终外框。
    pub line_left: f32,
    pub line_right: f32,
    /// 该 layer 在本行是否向相邻行延续；语义与 RichTextLineSegment 的同名方法一致。
    pub continues_from_previous_line: bool,
    pub continues_on_next_line: bool,
    /// 背景 layer 已解析的四角半径（inset = 0）；其他 layer 为 None。
    pub corner_radii: Option<RichTextCornerRadii>,
    /// 下划线与删除线的中心线纵坐标；其他 layer 为 None。
    pub line_y: Option<f32>,
}

impl LayoutResult {
    /// 返回逐 positioned cluster 的最终富文本 layer 几何。
    pub fn rich_text_layer_cluster_segments(&self) -> Vec<RichTextLayerClusterSegment>;
}
```

覆盖 `RichTextLayerKind::Background`、`Underline`、`LineThrough`。`Text` 与 `Annotation` layer 不参与：正文由 glyph 表达，注音由 ruby、bopomofo decision 表达。

切分规则：复用现有逐行最终几何（即 `rich_text_background_segments()` 与 `rich_text_decoration_segments()` 使用的同一套外缘修剪、水平与垂直 padding、metric policy 与相邻同样式间隙），再把每个行段按 positioned cluster 切开。规则只有一份实现，逐 cluster 与逐行不会给出不同答案。

补充约定：

1. 输出顺序沿用现有 span-major 迭代：按 `input.rich_text` 顺序（span 及其 layer），同一 layer 内按 (`line_index`, `cluster_index`) 升序；不做额外排序。
2. 同一个行段的片段平铺 `line_left..line_right`：首片段 `left == line_left`，末片段 `right == line_right`，相邻片段边界坐标重合；cluster 之间若有未被 cluster box 覆盖的间距（自动间距、行调整余量），该间距并入左侧片段。因此消费方用可见片段的 `min(left)..max(right)` 即得当前可见范围，全部可见时等于 `line_left..line_right`。消费方也可按 `cluster_index` 做一次线性散列把片段归属到 cluster。
3. `top`、`bottom`、`baseline`、`line_left`、`line_right` 与两个 `continues_*` 在同一行段的所有片段上相同。
4. 同一行段的所有片段共用同一组 `corner_radii` 与同一个 `line_y`，按该行段的起点解析，不按 cluster 重新解析。两者与 `rich_text_background_corner_radii(行段, 0.0)`、`rich_text_decoration_line_y(行段, layer.thickness)` 结果一致。

### 3. 逐 cluster 的装饰几何

```rust
/// 一个 positioned cluster 上某条装饰范围贡献的最终几何片段。
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationClusterSegment {
    /// 调用方声明的范围身份；None 表示未声明。
    pub id: Option<u32>,
    pub kind: DecorationKind,
    pub cluster_index: i32,
    pub line_index: i32,
    /// 本 cluster 与该装饰行段范围的交集。
    pub range: TextRange,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    /// 该装饰范围在其视觉行上的最终外框。
    pub line_left: f32,
    pub line_right: f32,
    /// 该范围在本行是否向相邻行延续；示亡号据此判断哪些边框属于范围自身。
    pub open_start: bool,
    pub open_end: bool,
}

impl LayoutResult {
    /// 返回逐 positioned cluster 的最终装饰几何。
    pub fn decoration_cluster_segments(&self) -> Vec<DecorationClusterSegment>;
}
```

覆盖 `DecorationKind::Mourning`、`ProperNoun`、`BookTitle`。来源是既有的逐行 `debug.decoration_segments`，按覆盖的 cluster 切分，不重新计算几何。

约定：

1. 示亡号片段的 `top`、`bottom` 是边框纵向边界；专名号与书名号片段的 `top == bottom`，即中心线纵坐标，消费方与既有平台渲染器一样按 `top` 绘制。
2. `open_start`、`open_end` 取自所属装饰行段，表示该 authored range 是否向相邻行延续；同一行段的片段取值相同。
3. 平铺规则与第 2 节相同：同一行段的片段覆盖 `line_left..line_right`，首尾片段拥有外缘。
4. `DecorationKind::Emphasis` 不进入本查询：着重号已由 `debug.decoration_decisions` 逐 cluster 给出锚点与直径，本迭代只在该 decision 上补充 `id`。

### 4. 调用方声明的 source boundary

`ParagraphBuilder` 增加：

```rust
impl ParagraphBuilder {
    /// 声明一个 source offset 为边界，使最终 cluster 不跨越它。
    pub fn source_boundary(&mut self, offset: ScalarOffset) -> &mut Self;
    /// 批量声明；重复与越界的 offset 自然无效。
    pub fn source_boundaries(&mut self, offsets: impl IntoIterator<Item = ScalarOffset>) -> &mut Self;
}
```

行为：

1. `build()` 把这些 offset 与既有 rich-text 范围端点一起写入 `TiqianTextContent.source_boundaries`，不新增数据通道。
2. 越界或重复的 offset 不 panic、不 warning：它们落在没有 cluster 边界的位置时不产生效果，与 tiqian 不为数据正确性做运行时检查的既定取舍一致。
3. 该入口只影响 cluster 切分粒度，不得改变断行、行调整、glyph advance 与 draw_x；实施时必须用对照测试证明。
4. 不使用该入口时，cluster 划分与现状一致。

### 5. 与既有查询和索引的关系

- 既有逐行查询与解析函数保持不变，其他渲染器继续使用。
- `to_replay_index` 增加 `rich_text_layer_cluster_segments` 与 `decoration_cluster_segments` 两个字段，与既有逐行结果一起一次构造，避免渲染层重复查询。
- 保留 `positioned_rich_text_segments()`：它是逐 cluster 的 occupied 几何，仍是选择、命中与几何调试的基础。

## 语义不变量

以下不变量是本迭代的验收依据，也是消费方可以依赖的公开约定：

1. **聚合等价**：把 `rich_text_layer_cluster_segments()` 按 (span, line_index) 聚合，`min(left)`、`max(right)`、`top`、`bottom` 与 `rich_text_background_segments()`、`rich_text_decoration_segments()` 对应行段逐字段相等。
2. **平铺**：同一行段的片段按 cluster 顺序覆盖 `line_left..line_right`，首尾片段拥有外缘，相邻片段边界重合。
3. **身份直通**：`RichTextLayer.id` 原样出现在该 layer 的每个片段上；`DecorationSpan.id` 原样出现在其装饰片段与着重号 decision 上。
4. **id 中性**：同一输入声明 id 与不声明 id 时，除 `id` 字段外所有输出逐字段相等。
5. **边界中性**：声明 source boundary 只改变 cluster 粒度；行的 range 与 visual width、glyph 的 id、x、y、advance、baseline 序列不变。
6. **既有查询不变**：未使用新查询与新入口时，既有输出与 golden 零 diff。

## 兼容性与回滚

- 本 crate 处于开发阶段。为公开结构体新增字段属于破坏性变更（`RichTextLayer`、`DecorationSpan`、`DecorationSegmentInfo`、`DecorationDecisionInfo`），发布时按 minor 版本处理；新增查询与类型是增量。
- 回滚：删除两个新查询、两个新类型、builder 的 id 与 boundary 入口，并移除 4 个结构体的新增字段；`normalized_rich_text` 的合并条件随字段移除自动回到现状。断行、行调整与几何算法不受本迭代影响，回滚不需要更新 golden。

## 性能要求

1. 两个新查询保持线性：输出规模与 (layer 或装饰行段 × 覆盖 cluster) 线性相关，不重复扫描 `input.rich_text`、`glyphs` 或 `clusters`。
2. 现有逐行实现里 `trim_outer_punctuation_glue` 对每个行段做一次 `debug.geometry_decisions` 线性查找；逐 cluster 查询必须先建立一次 `TextRange → geometry decision` 索引再切分，不得放大成每片段一次全表查找。
3. 片段列表一次 `Vec::with_capacity` 分配，不为每个片段单独分配集合；`Arc<RichTextSpan>` 与逐行查询共用，不复制 layer 内容。
4. 不为 id 引入字符串、哈希或跨布局全局表。

## 验证

### 最小必要测试

| 验证目标 | 最小证据 |
| --- | --- |
| id 进入输入 | 带 id 的 layer 与 decoration 出现在 `LayoutInput.rich_text`、`decorations` 中，id 原样保留。 |
| 相邻同 id 合并 | 同一 id、同 layer 的相邻 push 合并为一个 span，id 不变。 |
| 相邻异 id 不合并 | 相邻、同样式但 id 不同的两个范围保持为两个 span，几何相互独立。 |
| id 中性 | 同一输入声明与不声明 id，除 id 外逐字段相等。 |
| 逐 cluster 片段 | 一个背景跨 3 个 cluster 时输出 3 个片段，`cluster_index` 递增，首片段 `left == line_left`，末片段 `right == line_right`，相邻片段边界重合。 |
| 聚合等价 | 片段聚合结果与对应逐行查询逐字段相等（背景、下划线、删除线各一例）。 |
| 解析值一致 | 片段的 `corner_radii` 等于 `rich_text_background_corner_radii(行段, 0.0)`；片段的 `line_y` 等于 `rich_text_decoration_line_y(行段, thickness)`。 |
| 装饰片段 | 示亡号跨行时每行片段带正确的 `open_start`、`open_end`；专名号与书名号片段 `top == bottom` 且覆盖 `line_left..line_right`。 |
| 相邻缩短 | 两个相邻同样式范围（`adjacent_same_style_clearance` > 0）的片段外缘反映缩短后的 `line_left`、`line_right`。 |
| 边界中性 | 在同一个 Latin 词内部声明 boundary 后，行的 range、visual width 与 glyph 序列（id、x、y、advance）与未声明时一致，只增加 cluster 粒度。 |
| 边界越界 | 声明越界或重复 offset 不 panic，也不改变布局结果。 |
| 既有行为 | `cargo test` 全部通过，fixture golden 零 diff。 |

### 命令

```text
cargo test
cargo test --test tiqian layout_fixture_golden_test
cargo check --all-targets
git diff --check
```

需要对查询采样时使用既有 example `cargo run --release --example paragraph-layout-bench`，并在其中增加一个只调用新查询的采样模式。

## 文档与跟踪

实施完成后判断并更新：

| 文档 | 需要判断的内容 |
| --- | --- |
| `docs/key-differences.md` | Kotlin 上游没有范围身份与逐 cluster 几何，属于 Rust 先行能力；建议在“技术决策”一节记录一条。是否需要 ADR 由讨论决定，本文倾向不新增 ADR。 |
| `docs/tracking.md` | 若该能力需要在同步时向上游说明，补充对应条目。 |
| 本文 | 实际实施差异、测试与验证结果。 |

## 相关文件

| 文件 | 本迭代职责 |
| --- | --- |
| `src/core/text_model.rs` | `RichTextLayer.id`、`DecorationSpan.id`。 |
| `src/api/builder.rs` | id 感知的 span 记录与合并规则、`source_boundary`、`source_boundaries`、`build()` 边界合并。 |
| `src/api/scopes.rs` | 装饰与行内代码的带 id scope 与 `_with_id` 入口；`finish_scope` 写入 id。 |
| `src/api/convenience.rs` | 装饰便捷入口的带 id 变体（按需要）。 |
| `src/layout/annotation_geometry_stage.rs` | 把 `DecorationSpan.id` 写入 `DecorationSegmentInfo` 与 `DecorationDecisionInfo`。 |
| `src/core/layout_queries.rs` | 两个新查询、两个新类型，以及逐行几何按 cluster 复用（含 `TextRange → geometry decision` 索引）。 |
| `src/core/layout_model.rs` | `DecorationSegmentInfo`、`DecorationDecisionInfo` 的字段与 debug 组装。 |
| `src/core/layout_result_replay_index.rs` | 缓存两个新查询结果。 |
| `tests/core/`、`tests/api/` | 结构化回归测试：`tests/core/rich_text_cluster_segments_test.rs` 覆盖两个新查询与 boundary 入口，`tests/api/paragraph_builder.rs` 覆盖 id 声明、合并与中性。 |

## 待讨论问题

以下问题已于 2026-09-25 确定。

1. 已确定：`corner_radii` 与 `line_y` 在片段上预解析，解析规则留在 tiqian；消费方不需要知道 inset 与 thickness 细节。参数化 resolver（`rich_text_background_corner_radii`、`rich_text_decoration_line_y`）保持现状，继续服务逐行消费方。
2. 已确定：本迭代只做逐行几何的机械切分，不在 tiqian 内按 id 归组重新解析圆角与相邻同样式间隙。内部样式变化带来的内部边界保持现状，改进留给后续决策。
3. 已确定：不新建 example。在现有 `paragraph-layout-bench` 增加一个只调用新查询的采样模式。

## 实施结果

### 实际实现

| 位置 | 实际内容 |
| --- | --- |
| `src/core/text_model.rs` | `RichTextLayer.id`、`DecorationSpan.id`；`DecorationKind` 实现 `Display`（与 `Debug` 一致），使调试与 dump 格式不变。 |
| `src/api/scopes.rs` | 装饰与行内代码 scope 携带可选 id；新增 `push_decoration_with_id`、`with_decoration_with_id`、`try_with_decoration_with_id`、`push_inline_code_with_id`、`with_inline_code_with_id`、`try_with_inline_code_with_id`。 |
| `src/api/builder.rs` | 新增 `source_boundary` / `source_boundaries`；`build()` 把它们与 rich-text 端点一起写入 `content.source_boundaries`；正文与装饰 layer 的记录路径按需要携带 id。 |
| `src/core/layout_model.rs` | `DecorationSegmentInfo` 与 `DecorationDecisionInfo` 的 `kind` 由 `String` 改为类型化 `DecorationKind`，并各增加 `id`；`DecorationDecisionInfo::new`/`builder` 新增 `id` 参数。 |
| `src/layout/annotation_geometry_stage.rs` | 装饰决策与行段直接转写 `DecorationSpan` 的 `kind` 与 `id`；相邻行间线缩短改用类型匹配。 |
| `src/core/layout_queries.rs` | 新增 `RichTextLayerClusterSegment`、`DecorationClusterSegment`、`rich_text_layer_cluster_segments()`、`decoration_cluster_segments()`、内部 `covered_clusters`、`collect_cluster_fragments` 与 `background_segment_padding_and_metrics`。 |
| `src/core/layout_result_replay_index.rs` | `to_replay_index` 一次构造两个新查询结果。 |
| `examples/paragraph_demo/renderer.rs` | 删除装饰名称到 `DecorationKind` 的字符串映射，直接用类型化 kind。 |
| `examples/paragraph-layout-bench.rs` | 新增 `--cluster-queries` 采样模式。 |

### 与计划的偏差

1. 逐行终结步骤被提取为 `background_segment_padding_and_metrics`，两个逐行查询与逐 cluster 查询共用同一实现；这是“聚合等价”与“规则只有一份”的直接做法，没有改为在 `rich_text_background_segments` 与新查询之间复制代码。
2. 未新增 `TextRange → geometry decision` 索引：逐 cluster 查询只在行段粒度执行既有查找，片段切分本身不做任何查找，因此不存在 性能要求 第 2 条所指的按片段放大。现有逐行实现内的线性查找保持原样。
3. 相邻同样式间隙继续按可见样式比较，不看 id；这正是待讨论问题第 2 项的“保持现状”。副作用是两个相邻异 id、同样式的范围不再被 `normalized_rich_text` 合并，因此各自拥有独立外缘，与本迭代的合并规则一致。
4. `src/api/convenience.rs` 未增加装饰便捷方法的带 id 变体：背景与线条由调用方直接构造带 id 的 layer，装饰由 scope 入口声明，无需再复制一组便捷方法。
5. 未新增 ADR，理由见“文档与跟踪”。

### 验证

| 项目 | 结果 |
| --- | --- |
| `cargo test` | 20 + 1 + 1353 项全部通过，doc-test 0 项。 |
| `cargo test --test layout_fixture_golden_test` | 通过；`tests/fixture_layout/golden/` 未发生改动。 |
| `cargo check --all-targets` | 通过；剩余 warning 均来自本次未改动的文件（`paragraph_dp_line_breaker.rs`、`punctuation_geometry_ledger.rs`、`tests/support`、`examples/paragraph_demo/app.rs`）。 |
| `git diff --check` | 通过。 |
| `cargo run --release --example paragraph-layout-bench -- --cluster-queries --iterations 20 --warmup 5 --widths 672` | 每次调用得到 56 个 cluster 片段；`cluster_query` 中位数为同一次运行 `layout` 中位数的 2% 左右（两次运行分别测得 0.111 ms / 2.483 ms 与 0.281 ms / 12.723 ms，绝对值随机器负载浮动）。 |
| 人工窗口检查 | 未进行；demo 的装饰与富文本重放路径由 `examples/paragraph_demo/renderer.rs` 的内置测试覆盖。 |

### 文档与跟踪

- `docs/key-differences.md` 增加“富文本范围身份与逐 cluster 几何”一条，记录该能力为 Rust 先行。
- `docs/tracking.md` 未修改：它面向 Kotlin 上游同步，本迭代不来自上游；需要同步说明时已可从 `key-differences` 读到。
- 不新增 ADR：本迭代只扩展富文本旁路的数据与查询，没有改变 R0002 的模型边界。
