# 2026-09-06 统一富文本旁路模型

> 状态：已完成

## 目标

将 `ColorSpan` 与旧的单 role `RichTextSpan` 合并为统一的富文本旁路模型。新的模型以 source range
为单位保存多个视觉 layer、每个 layer 的多个绘制定义以及链接等非视觉语义。它由
`ParagraphBuilder` 生成，在 layout 后与 `LayoutResult` 的范围几何一起交给 renderer。

本迭代落实 [R0002](../adr/R0002-unified-rich-text-paint-model.md)。它调整输入构造与 layout 后的
数据传递，不调整 shaping、字体选择、度量、断行、行调整或两端对齐。

## 当前状态

当前 `ParagraphBuildOutput` 包含：

```text
input: LayoutInput
colors: Vec<ColorSpan>
rich_text: Vec<RichTextSpan>
```

`ColorSpan` 以 `start`、`end` 和 ARGB 表示文本填充。旧 `RichTextSpan` 以 `range`、`RichTextRole`
和单个 `RichTextPaint` 表示背景、下划线、删除线、链接和 inline code。颜色与 rich text 都是
layout 后由 renderer 消费的 source range 记录；builder 为它们写入 `source_boundaries`。

## 范围

### 包含

- 定义统一的 `RichTextSpan`、`RichTextLayer`、`RichTextPaint` 与 `RichTextSemantic`；
- 以 `Fill`、`Stroke`、`Shadow` 枚举表达每个 layer 的绘制定义；
- 以文本、背景、下划线、删除线、CLREQ decoration 与 ruby / bopomofo annotation 表达第一批视觉 layer；
- 迁移 `ColorSpan` 为文本 layer 的 `Fill`，删除独立颜色输出；
- 迁移现有背景、下划线、删除线、link、inline code、decoration 与 ruby 的 builder lowering；
- 调整 range 几何查询、paragraph demo renderer、单元测试和文档；
- 为未绘制的 stroke 和 shadow 建立明确的 demo 行为与测试边界。

### 不包含

- 不将富文本数据加入 `LayoutInput`，也不修改 layout pipeline 输入；
- 改变 layout 的字体度量、断行、行高、cluster、glyph 或 bounds；
- 在本迭代中实现所有 renderer 的 stroke、shadow 或彩色 emoji 效果；
- 规定跨 renderer 的全局 layer 绘制顺序或同类 paint 合成算法；
- 改变 `LayoutResult` 是否持有构建输出的决定；
- 修改 Kotlin 上游的富文本模型或前端实现。

## 已确认设计

### 数据关系

```text
ParagraphBuilder
    ↓
ParagraphBuildOutput
├─ input: LayoutInput
└─ rich_text: Vec<RichTextSpan>
    ↓                                  ↓
layout(input)                      renderer / frontend
    ↓                                  ↑
LayoutResult ─────────────────────────┘
```

`LayoutInput` 是 layout pipeline 的唯一输入。`rich_text` 不参与 layout；renderer 使用 span 的 source
range 与 `LayoutResult` 提供的 cluster、line 和范围几何重放视觉 layer 或消费语义。

### 统一 span

目标结构为：

```text
RichTextSpan
├─ range: TextRange
├─ layers: Vec<RichTextLayer>
└─ semantics: Vec<RichTextSemantic>
```

`RichTextLayer` 保存每个 layer 共有的 paint，`RichTextLayerKind` 保存对象类型和专属几何：

```text
RichTextLayer
├─ kind: RichTextLayerKind
└─ paints: Vec<RichTextPaint>

RichTextLayerKind
├─ Text
├─ Background { background: RichTextBackgroundPaint }
├─ Underline { line: RichTextLinePaint }
├─ LineThrough { line: RichTextLinePaint }
├─ Decoration { kind: DecorationKind }
└─ Annotation { kind: RubyKind }
```

`RichTextPaint` 直接保存 paint 字段：

```text
RichTextPaint
├─ Fill { argb: i32 }
├─ Stroke { argb: i32, width: f32 }
└─ Shadow { argb: i32, offset_x: f32, offset_y: f32, blur_radius: f32, spread_radius: f32 }
```

core 不为 paint 字段提供构造校验；`f32` 值原样保留给 renderer。

一个 span 可有多个 layer；多个 span 的 range 可相交。每个 layer 也可有多个同类 paint。core 保留
调用方提供的全部 paint，不合并、不去重，不以 `layers` 或 `paints` 列表顺序规定绘制顺序。renderer 自行决定
对象排序、paint 合成和平台不支持效果的处理。

`ParagraphBuilder::new()` 的顶级 paints 为 `Fill { argb: 0xFF1E1E23 }`。`.paints(&[RichTextPaint])`
与 `.text_style(...)` 一样只能在追加 source text 前设置，并替换顶级默认值。`with_paints` 与
`try_with_paints` 建立词法 paint scope：最近 scope 的完整 paints 替换外层集合，不合并。每次追加非空
source text 时，若没有匹配的 `Text` layer，builder 复制当前最近 paints 并为该 range 生成 `Text` layer。

`Fill` 只保存调用方显式提供的 ARGB 颜色；alpha 为零表示显式透明填充。一个 layer 缺少 `Fill` 时，
renderer 不绘制填充，且不得从主题、正文颜色或其他 layer 补充颜色。

`Stroke` 只保存调用方显式提供的 ARGB 颜色与物理 layout-unit 宽度；alpha 为零表示
显式透明描边。一个 layer 缺少 `Stroke` 时，renderer 不绘制描边。line cap、line join 与 miter limit
由 renderer 按自身绘制表示决定，不进入公开 rich-text 类型。

`Shadow` 只保存调用方显式提供的 ARGB 颜色、x/y 位移、blur 半径与 spread 半径；alpha 为零表示
显式透明阴影。spread 在生成阴影前相对对象轮廓向外扩张或向内收缩。一个 layer 缺少 `Shadow` 时，
renderer 不绘制阴影。

`RichTextSemantic` 只包含两个不直接定义视觉绘制的范围语义：

```text
RichTextSemantic
├─ Link { target: String }
└─ TechnicalInline
```

两者都不保存 paint 或 layout 参数。link 显示内容为地址时，构建 API 继续按已有规则产生
`LineBreakSpan`；`TechnicalInline` 的 technical 断行与自动间距抑制也继续由构建 API 生成。

`Decoration` layer 通过 range 和 `DecorationKind` 匹配 core 生成的着重号或逐行装饰几何；`Annotation`
layer 通过 `RubySpan.base_range` 和 `RubyKind` 匹配 core 生成的 ruby / bopomofo glyph placement。它们只提供
paint：`DecorationSpan` 和 `RubySpan` 仍在 `LayoutInput` 中决定 shaping、断行、行高、几何和复制语义。

builder 为可绘制对象从内向外查找当前声明中类型匹配的 layer：正文查找 `Text`，decoration 查找同 kind 的
`Decoration`，ruby / bopomofo 注文查找同 kind 的 `Annotation`。找到时使用该 layer 的 paints；找不到时
使用当前 paint 栈的最近值。

对每个对象类型和 kind，最近一个包含匹配 layer 的当前声明完整覆盖外层同类声明；其他类型和 kind 继续从
外层查找。同一个声明中的同类 layer 按输入顺序全部保留，构成该对象的完整 layer 列表。文本追加时读取
当前 `Text`、`Background`、`Underline` 与 `LineThrough` 声明，并以追加的文本 range 生成相应 layer。
Ruby、Decoration、InlineBox、Link、Technical 与 InlineCode 继续在其既有内容对象的完整范围确定时生成
layout 记录或 semantic；R0002 只替换这些记录的数据模型，不改变范围确定时机。每次 `push()` 还会为当时活跃的
Decoration 与 Ruby scope 以该次追加的文本 range 记录对应 layer。相邻且 layer
列表完全相同的文本输出记录合并；builder 只为合并后的 `RichTextSpan` 首尾写入 `source_boundaries`，不保留
内部 `push()` 分界。合并只收敛构建输出，不改变绘制或范围查询语义。

不同对象产生点得到的记录若具有完全相同的实际 range，builder 将其 layers 与 semantics 归并为一个
`RichTextSpan`；不同 range 保持独立并允许相交。归并保留各 layer 与语义的产生顺序，不合并、不去重。

`with_decoration(kind, ...)` 与 `with_ruby(annotation, ...)` 保持现有签名。非空 Decoration 或 Ruby 内容对象
继续在 scope 结束时生成既有 layout 声明；每次 `push()` 则快照匹配的 `Decoration` 或 `Annotation` 声明并以
该文本 range 生成对应 layer。找不到时，builder 使用当时的当前 paints 生成同 kind 的回退 layer。因此 layer 的
range 可以只是对应 layout 声明范围的一部分；consumer 以 range 交集与 kind 关联二者。Ruby 的基字正文仍按 `Text`
layer 的查找规则取 paint。调用方需要统一覆盖未匹配 layer 的 paint 时，以 `with_paints` 或 `try_with_paints`
包围对应内容。

`push_rich_text`、`with_rich_text` 与 `try_with_rich_text` 删除旧的 `RichTextRole` 参数，统一接收
`&[RichTextLayer]`；它们只更新当前 layer 声明。每次 `push()` 按文本的实际 range 快照当前 `Text`、
`Background`、`Underline` 与 `LineThrough` 声明；未查找到 `Text` 时生成使用当前 paint 栈最近值的回退 `Text`
layer。相邻且等价的文本输出记录按 `TextSpan` 的既有规则合并。`color`、
`with_color`、`try_with_color` 与 `push_color` 保留为调用方便利方法，生成仅含 `Fill { argb }` 的 paint scope。

### 绘制与布局边界

`Fill`、`Stroke` 与 `Shadow` 都是纯渲染参数。它们不改变 `LayoutInput`、layout dump、
`LayoutResult.size`、line、cluster、glyph 或 glyph bounds。`Decoration` 和 `Annotation` layer 不新增 layout
规则；它们只为既有 `DecorationSpan` 和 `RubySpan` 产生的最终结果定义 paint。renderer 以 `LayoutResult`
的最终几何进行绘制，宿主自身的 clip 行为仍由宿主决定。

本迭代中，demo 必须继续绘制已有的文本 fill、背景、下划线和删除线。对于新增的 stroke 与 shadow，
demo 可以按 paint 单独跳过；同一 layer 中已支持的 paint 继续绘制。不得用其他效果替代，也不得将它们
作为布局参数。彩色 emoji 的效果仅在 renderer 可可靠支持时绘制。

### 现有类型的迁移

| 当前记录 | 目标记录 |
| --- | --- |
| `ColorSpan { start, end, argb }` | 同 range 的文本 layer，包含 `RichTextPaint::Fill`。 |
| `RichTextRole::Background` | background layer 与背景几何。 |
| `RichTextRole::Underline` | underline layer 与线条几何。 |
| `RichTextRole::LineThrough` | line-through layer 与线条几何。 |
| `RichTextRole::Link { target }` | `RichTextSemantic::Link { target }`。 |
| `RichTextRole::InlineCode` | 同 range 的 Background layer；旧 paint 拆分为背景几何与通用 paints。`inline_code()` 的局部样式继续生成 `TextSpan`，并生成 `TechnicalInline` semantic 与既有 technical layout 输入。 |
| `RichTextRole::TechnicalInline` | `RichTextSemantic::TechnicalInline`。 |
| `DecorationSpan { range, kind }` | 保留为 `LayoutInput` 的布局声明；同 range、同 kind 的 `Decoration { kind }` layer 为其结果提供 paint。 |
| `RubySpan { base_range, kind, .. }` | 保留为 `LayoutInput` 的布局声明；同 base range、同 kind 的 `Annotation { kind }` layer 为其结果提供 paint。 |

旧 `RichTextPaint` 的 ARGB、背景几何、线型和相邻范围间距按新的所属对象拆分。`RichTextBackgroundPaint`
属于 background layer，保存 padding、圆角、续行圆角、度量策略与 `adjacent_same_style_clearance`；
背景的填充、边框和阴影直接使用该 layer 的 `RichTextPaint::Fill`、`RichTextPaint::Stroke` 与
`RichTextPaint::Shadow`。旧 `RichTextBackgroundDrawStyle` 删除，不再保留填充与边框二选一的定义。
`RichTextLinePaint` 属于 underline 与 line-through layer，保存实线、虚线、点线等线条几何与
`adjacent_same_style_clearance`。颜色、描边和阴影属于通用 `RichTextPaint`。

`RichTextLinePaint` 以 `thickness` 定义所有线型的基础尺寸：实线与虚线使用它
生成带状路径，点线使用它作为圆点直径。线型只保存实线、虚线的 dash/gap 或点线的 gap。`Fill`、
`Stroke`、`Shadow` 作用于已生成的线条形状；`Stroke.width` 表示该形状外缘的描边宽度。

`RichTextLinePattern` 固定为 `Solid`、`Dashed { dash_length, gap_length }` 与
`Dotted { gap_length }`。点线直径始终来自 `RichTextLinePaint.thickness`。

`RichTextLinePaint::default()` 使用 `thickness = 1.0`、`pattern = Solid` 与
`adjacent_same_style_clearance = 0`。demo 保留已有的按字号推导线条尺寸规则，但在 `ParagraphBuilder`
构造 layer 时将结果写入明确的 `thickness`；renderer 不再推导。相邻 range 的 clearance 匹配规则不在
本迭代调整，迁移时保持现有 range 几何查询行为。

`RichTextBackgroundPaint` 保留 `horizontal_padding`、`vertical_padding`、`corner_radius`、
`continuation_corner_radius`、`metric_policy` 与 `adjacent_same_style_clearance` 六项字段，并保留
当前类型内部的 `Default` 与 builder 默认构造语义：两个 padding、两个圆角和 clearance 默认为零，
`metric_policy` 默认为 `MarkedFaces`，builder 未设置 `continuation_corner_radius` 时复制
`corner_radius`。这些默认值只定义背景范围盒几何；没有 `Fill`、`Stroke`、`Shadow` 时，默认几何
不会产生对应绘制。

当前 builder 对旧 `RichTextRole::Background` 与 `RichTextRole::InlineCode` 保留 Compose 的既有 lowering：
`horizontal_padding` 大于零时，为同一非空 range 生成 `InlineBoxSpan`，两个边缘均使用该 padding，
`outer_spacing` 为 `Narrow`。该 `InlineBoxSpan` 继续作为既有 `LayoutInput` 记录参与断行、两端对齐与
CJK 边界间距；R0002 将这条 lowering 迁移到 Background layer 的构造，不在 renderer 端补偿，也不改变
layout 算法。

核心模型不定义 `InlineCode` layer 或 `InlineCode` semantic。旧 role 当前只用于选择背景范围几何，迁移为
Background layer。`inline_code()` 保留为构建 API：它继续将 `TextStyleOverride` 解析为 `TextSpan`，并为
同一非空 range 生成 Background layer、`TechnicalInline` semantic、`ProgressiveTechnical` 断行记录、
自动间距抑制范围与 source boundary。`technical()` 与 `inline_code()` 重叠时，相同 range 的同一断行策略
和自动间距抑制范围在构建输出中各保留一项。

`push_inline_code`、`with_inline_code`、`try_with_inline_code` 与 `inline_code` 的旧 `RichTextPaint`
参数改为 `RichTextBackgroundPaint`，只保存背景几何。InlineCode 内容对象的完整范围确定时，若当前声明
不存在 `Background` layer，builder 使用该背景几何和当时的当前 paints 生成 Background layer；存在声明时
按一般 Background layer 查找规则处理。需要专用背景 paint 时，调用方使用 `with_paints` / `try_with_paints`
或外层 `Background` layer 声明。

### 已确认的公开字段

`RichTextPaint`、`RichTextLayer` 与 `RichTextLayerKind` 的公开结构已确认。paint 和 layer 专属几何字段
均为直接数据字段；core 不提供构造校验。renderer 对不支持的单个 paint 跳过处理，并继续绘制同 layer 中
受支持的 paint。

## 实施阶段

各 phase 以完成一组迁移任务为目标；中间状态可以不能编译、运行或通过测试。只有 Phase 4 结束时，仓库必须恢复
完整可运行、可测试状态。

### Phase 1：替换公开数据模型

1. 在 `src/core/text_model.rs` 定义新的 `RichTextSpan`、`RichTextLayer`、`RichTextLayerKind`、
    `RichTextPaint`、`RichTextLinePaint`、`RichTextLinePattern` 与 `RichTextSemantic`；
2. 删除 `ColorSpan`、旧的 role/paint 模型及 `RichTextBackgroundDrawStyle`，将背景和线条的专属几何迁移到
    对应 layer，通用绘制定义迁移到 `Fill`、`Stroke`、`Shadow`；
3. 将 `ParagraphBuildOutput` 收敛为 `input` 与 `rich_text`，调整公开导出和直接依赖这些类型的声明；
4. 保持 `LayoutInput`、`LayoutResult` 及 layout pipeline 不变。

完成条件：旧旁路类型已删除，后续 builder、查询和 demo 可以只面向统一类型继续迁移。

### Phase 2：迁移 builder 产出

1. 将 `ParagraphBuilder` 的 `colors` 与旧 rich-text 暂存记录替换为统一 rich-text 暂存记录；
2. 增加顶级 `.paints(...)`、`with_paints` 与 `try_with_paints`，实现当前 paints 的完整替换和 layer 声明查找；
3. 将 `*_rich_text` 改为接收 `&[RichTextLayer]`，保留 `with_color`、`color` 等调用方便利方法，使其生成单个
    fill 的 paint scope；`push()` 按当前文本相关声明生成并收敛 layer；
4. 迁移 Ruby、Decoration、InlineBox、Link、Technical 与 InlineCode 的既有 lowering：scope 结束时生成原有
    layout 输入或 semantic，每次 `push()` 为活跃 Ruby 与 Decoration scope 记录局部 layer；`inline_code`
    系列改为接收 `RichTextBackgroundPaint`；
5. 在 builder 完成收敛和同 range 归并后写入 `source_boundaries`，不保留连续相同文本 layer 的内部 `push()`
    分界。

完成条件：所有 builder 输入只产出统一 `rich_text` 与既有 `LayoutInput`；不再写入独立颜色旁路。

### Phase 3：迁移范围查询与 demo 消费端

1. 将 `src/core/layout_queries.rs` 的旧 role 筛选改为 layer 筛选，保持既有逐行范围几何、背景 padding 和线条
    clearance 计算；增加 Decoration 与 Annotation 按 range、kind 查询 paint 的路径；
2. 将 paragraph demo 的文档模型、sample 和 renderer 收敛为一个 `rich_text` 列表；迁移文本 fill、背景、
    下划线、删除线、CLREQ decoration、ruby、bopomofo 与 inline code；
3. renderer 按 paint 独立处理能力：继续绘制受支持的 paint，跳过本迭代尚未支持的 `Stroke`、`Shadow` 或彩色
    emoji 效果。

完成条件：所有旧 rich-text 消费点已改为统一 layer / semantic 模型，demo 不再读取 `ColorSpan` 或旧 role。

### Phase 4：恢复完整性并验证

1. 迁移 builder、便利方法、范围查询和 demo 的测试，使它们直接构造和断言统一 layer / semantic 模型；
2. 修复前三个 phase 合并后暴露的编译、所有权、导出和测试断言问题，恢复所有 target 可构建、所有测试可运行；
3. 运行相关 builder、范围查询和 paragraph demo 测试，再运行 `cargo test --all-targets` 与 `cargo check`；
4. 审阅 layout dump 与 paragraph demo，确认 rich-text 迁移未改变字体、断行、行高、范围几何或其他 layout
    结果；
5. 更新本迭代文档、`docs/key-differences.md` 和实现后确有必要更新的持续维护文档；不修改历史迭代文档；
6. 运行文档风格检查、`git diff --check`，并审阅最终目标 diff。

完成条件：统一模型、builder、范围查询、demo 和测试迁移完成；仓库可运行、可测试，且验证要求全部通过。

## 验证要求

测试至少覆盖：

- 单个文本 fill 取代原 `ColorSpan`；
- 同一 span 的文本、背景和下划线 layer；
- 多个 span 的相交背景与下划线；
- 同一 layer 的 fill、stroke、shadow 同时存在及重复 paint 保留；
- 嵌套文本 fill 的后声明覆盖语义；
- `source_boundaries` 对统一 rich-text range 的首尾维护；
- 连续且相同的文本 layer 收敛后不保留内部 `push()` 分界；
- 背景、下划线和删除线仍按最终行几何切分；
- decoration 按对应 `DecorationKind` 取得独立 paint，且不改变核心装饰几何；
- ruby 与 bopomofo 按对应 `RubyKind` 取得独立 paint，且不改变 annotation glyph placement；
- stroke 与 shadow 不改变 layout 输出；
- demo 对已支持效果的重放，以及跳过不支持 paint 时不会造成 renderer 错误。

## 风险与处理

| 风险 | 处理 |
| --- | --- |
| 旧 role 查询遗漏新的 layer 筛选 | 将 background 与 line 查询的测试与迁移同步完成。 |
| 颜色覆盖顺序在迁移中改变 | 使用嵌套 builder 测试固定后声明 fill 的优先级。 |
| renderer 把新增 paint 误作 layout 参数 | 使用 layout dump 与结果几何对照测试。 |
| demo 静默忽略不支持效果 | 对 stroke 和 shadow 写明 no-op 行为，并在测试中断言该路径。 |

## 回滚

若统一模型无法保持现有颜色、背景、下划线和删除线的重放结果，撤回本迭代的 core、builder、query 和
demo 改动，恢复 `ColorSpan` 与旧 `RichTextSpan` 的两条旁路输出。R0002 仍保留为已接受的架构方向；
恢复实施前需在新的迭代文档中说明阻塞原因和后续方案。
