# ADR R0002: 统一富文本旁路模型与绘制定义

- Status: Accepted
- Date: 2026-09-06
- Implementation: Complete
- Relates: [2026-09-06 统一富文本旁路模型](../iteration/2026-09-06-unified-rich-text-paint-model.md)

## Context

实施前，`ParagraphBuildOutput` 分别返回 `colors: Vec<ColorSpan>` 与
`rich_text: Vec<RichTextSpan>`。两类数据都绑定 source range，不参与字体选择、度量、断行、行调整
或两端对齐，并在 layout 后依据 `LayoutResult` 的 source range 几何由 renderer 消费。

`ColorSpan` 只表示文本 ARGB 填充。现有 `RichTextSpan` 使用 `range`、单个 `RichTextRole` 与单个
`RichTextPaint` 表示背景、下划线、删除线、链接和 inline code。`RichTextPaint` 同时保存颜色、背景
参数、线型和相邻范围间距；其中部分字段只适用于特定 role。

这个模型不能在一项 range 内直接表示多个视觉对象及其各自的绘制定义。例如，文本可使用黑色填充和
白色描边，下划线可使用蓝色填充和独立阴影，背景可使用另一种填充。调用方只能建立多个 role 单一的
span，并且 `ColorSpan` 与 `RichTextSpan` 没有统一的构造输出列表。

当前 CLREQ decoration 与 ruby / bopomofo 由 `LayoutInput` 驱动，并在 layout 后分别生成装饰几何与
注文 glyph placement。它们没有独立 paint 来源：demo 的 decoration 沿用 `ColorSpan` 的文本颜色，
ruby / bopomofo 则固定使用默认正文色。因此删除 `ColorSpan` 后，统一模型还必须能为这些已有的 layout
输出指定独立绘制定义。

## Decision

### 统一旁路输出

删除独立的 `ColorSpan` 输出。`ParagraphBuildOutput` 保留 `input: LayoutInput` 与一个统一的
`rich_text: Vec<RichTextSpan>` 字段。

`RichTextSpan` 继续以一个半开 Unicode scalar `TextRange` 绑定 source text。它包含视觉 layer 和
非视觉语义：

```text
RichTextSpan
├─ range: TextRange
├─ layers: Vec<RichTextLayer>
└─ semantics: Vec<RichTextSemantic>
```

颜色、背景、下划线、删除线、CLREQ decoration、ruby / bopomofo 和链接均从同一组 range
旁路记录产生。`inline_code()` 的视觉部分也使用同 range 的 Background layer。ruby / bopomofo layer 的
`range` 是其 `RubySpan.base_range`；它只为 layout 产出的注文
glyph placement 指定 paint，不承载注文文本、字体或几何。builder 继续为每个非空 rich-text range 将首尾
写入 `LayoutInput.content.source_boundaries`，使 layout 输出精确的范围几何。

`ParagraphBuilder::new()` 初始化顶级 paints 为 `Fill { argb: 0xFF1E1E23 }`。`.paints(&[RichTextPaint])`
与 `.text_style(...)` 一样只能在追加 source text 前设置，用新的完整集合替换该顶级默认值。
`.with_paints(&[RichTextPaint], ...)` 与 `.try_with_paints(&[RichTextPaint], ...)` 建立词法 paint scope；
内层 scope 完整替换而不合并外层集合。它们提供没有同类显式 layer 时使用的当前 paints；顶级 paints 是
该栈的初始值。因此正文颜色由 builder 的显式输出决定，不再由 renderer 提供默认值。

### Layer、绘制定义与语义

`RichTextLayer` 将所有 layer 共有的 paint 与对象描述分开：

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

`paints` 属于每个 layer 的共同绘制定义；`RichTextLayerKind` 只保存对象类型和专属几何。`RichTextPaint`
直接保存各 paint 的公开字段：

```text
RichTextPaint
├─ Fill { argb: i32 }
├─ Stroke { argb: i32, width: f32 }
└─ Shadow { argb: i32, offset_x: f32, offset_y: f32, blur_radius: f32, spread_radius: f32 }
```

列表允许出现多个相同变体；core 不合并、不去重，也不由列表顺序定义绘制顺序或合成方式。
core 不为 paint 字段提供构造校验；`f32` 值原样保留给 renderer。

`Fill` 只携带调用方显式提供的 ARGB 颜色；alpha 为零表示显式透明填充。缺少 `Fill` 时 renderer 不绘制
填充，也不得从主题、正文颜色或其他 layer 推导默认颜色。

`Stroke` 只携带调用方显式提供的 ARGB 颜色与物理 layout-unit 宽度；alpha 为零表示
显式透明描边。缺少 `Stroke` 时 renderer 不绘制描边。line cap、line join 与 miter limit 属于 renderer
对自身绘制表示的实现选择，不进入公共 rich-text 模型。

`Shadow` 只携带调用方显式提供的 ARGB 颜色、x/y 位移、blur 半径与 spread 半径；alpha 为零表示
显式透明阴影。spread 在生成阴影前相对对象轮廓向外扩张或向内收缩。缺少 `Shadow` 时 renderer 不绘制阴影。

背景 layer 使用 `RichTextBackgroundPaint` 保存背景范围的 padding、圆角、续行圆角、度量策略与
`adjacent_same_style_clearance`。背景的填充、边框和阴影直接由该 layer 的 `RichTextPaint::Fill`、
`RichTextPaint::Stroke` 与 `RichTextPaint::Shadow` 表达；删除旧的
`RichTextBackgroundDrawStyle`，不再以二选一的 draw style 表达背景。下划线与删除线 layer 使用
`RichTextLinePaint` 保存实线、虚线或点线等线条几何与 `adjacent_same_style_clearance`。`Decoration` layer 以 range 和 `DecorationKind` 匹配 layout 生成的
着重号 decision 或逐行 segment；`Annotation` layer 以 base range 和 `RubyKind` 匹配 ruby / bopomofo
的最终 glyph placement。`Fill`、`Stroke` 与 `Shadow` 是可被各 layer 使用的绘制定义，不属于文字、
下划线、装饰或注文中的任一种专属字段。

`RichTextLinePaint` 使用 `thickness` 定义所有线型的基础尺寸：实线与虚线以此
生成带状路径，点线以此作为圆点直径。它的线型只定义实线、虚线的 dash/gap 或点线的 gap；填充、
外缘描边与阴影由该 layer 的 `Fill`、`Stroke`、`Shadow` 作用于已生成的线条形状。`Stroke.width`
定义已生成线条形状外缘的描边宽度。

`RichTextLinePattern` 包含 `Solid`、`Dashed { dash_length, gap_length }` 与 `Dotted { gap_length }`。
点线直径不在 pattern 中重复存储，始终取自 `RichTextLinePaint.thickness`。

`RichTextLinePaint` 保留类型内部的 `Default`：`thickness` 为 `1.0`，`pattern` 为 `Solid`，
`adjacent_same_style_clearance` 为零。demo 现有按字号推导线条尺寸的规则保留在 `ParagraphBuilder`
构造 layer 的阶段，并将结果写入明确的 `thickness`；renderer 不再从 `TextStyle` 或段落字号推导线条尺寸。
相邻 range 的 clearance 匹配规则不在本迭代调整，迁移时保持现有 range 几何查询行为。

`RichTextBackgroundPaint` 保留 `horizontal_padding`、`vertical_padding`、`corner_radius`、
`continuation_corner_radius`、`metric_policy` 与 `adjacent_same_style_clearance` 六项字段，并保留
类型内部的 `Default` 与 builder 默认构造语义：两个 padding、两个圆角和 clearance 默认为零，
`metric_policy` 默认为 `MarkedFaces`，builder 未显式设置 `continuation_corner_radius` 时复制
`corner_radius`。这些默认值只解析 Background layer 的对象几何；它们不创建或补充任何 `Fill`、
`Stroke`、`Shadow`。layer 专属几何同样不提供 core 构造校验。

Background layer 的 `horizontal_padding` 大于零时，`ParagraphBuilder` 为同一非空 range 生成既有的
`InlineBoxSpan`，两个边缘均为该 padding，`outer_spacing` 为 `Narrow`。该 lowering 保持 Compose 背景和
inline code 的现有行为，使水平 padding 进入既有的断行、两端对齐和 CJK 边界间距；它不由 renderer 补偿，
也不改变 layout 算法。

`Decoration` 与 `Annotation` layer 不替代 `LayoutInput.decorations` 或 `LayoutInput.ruby_spans`。前者
继续决定着重号的 Latin italic shaping、示亡号断行限制、行距下限与 CLREQ 几何；后者继续决定注文文本
的 shaping、基字扩张、断行限制、行高、复制语义和最终 placement。layer 只决定已经生成的可绘制对象使用
什么 paint。builder 为每个可绘制对象从内向外查找当前声明中类型匹配的 layer：正文查找 `Text`，
decoration 查找同 kind 的 `Decoration`，ruby / bopomofo 注文查找同 kind 的 `Annotation`。找到时，
该对象使用该 layer 的 paints；找不到时，使用当前 paint 栈的最近值。

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
layer 的查找规则取 paint，不从 `Annotation` layer 取得 paint。需要统一覆盖未匹配 layer 的 paint 时，调用方
以 `with_paints` 或 `try_with_paints` 包围对应内容。

`push_rich_text`、`with_rich_text` 与 `try_with_rich_text` 删除旧的 `RichTextRole` 参数，统一改为接收
`&[RichTextLayer]`。它们只更新当前 layer 声明；每次 `push()` 以文本的实际 range 快照当前 `Text`、
`Background`、`Underline` 与 `LineThrough` 声明。未查找到 `Text` 时，builder 为该 range 生成使用当前 paint
栈最近值的回退 `Text` layer。相邻且等价的文本输出记录按 `TextSpan` 的既有规则合并。`color`、`with_color`、
`try_with_color` 与 `push_color` 保留为调用方便利方法，生成仅含 `Fill { argb }` 的 paint scope。

`RichTextSemantic` 保存不直接定义可见绘制的范围语义：

```text
RichTextSemantic
├─ Link { target: String }
└─ TechnicalInline
```

两个变体都不保存 paint 或 layout 参数。link 显示内容为地址时，builder 仍额外生成已有的
`LineBreakSpan`；`TechnicalInline` 的既有 technical layout 输入由构建 API 生成；这些规则不由
rich-text renderer 决定。

旧 `RichTextRole::InlineCode` 不迁移为同名 layer 或 semantic。它当前的实际绘制职责是选择背景范围
几何并使用旧 `RichTextPaint`，因此迁移为同 range 的 Background layer、`RichTextBackgroundPaint` 与
`Fill`、`Stroke`、`Shadow`。`inline_code()` 保留为构建 API：其 `TextStyleOverride` 继续生成
`LayoutInput.content.spans` 中的 `TextSpan`，同时生成 `TechnicalInline` semantic、已有的
`ProgressiveTechnical` `LineBreakSpan`、自动间距抑制范围和 source boundary。`push_inline_code`、
`with_inline_code`、`try_with_inline_code` 与 `inline_code` 的旧 `RichTextPaint` 参数改为
`RichTextBackgroundPaint`：它只提供背景几何。InlineCode 内容对象的完整范围确定时，若当前声明不存在
`Background` layer，builder 使用该背景几何和当时的当前 paints 生成 Background layer；存在声明时按一般
Background layer 查找规则处理。需要专用背景 paint 时，调用方使用 `with_paints` / `try_with_paints` 或外层
`Background` layer 声明。

### Renderer 责任

renderer 根据自身能力选择 layer 的绘制顺序、同类 paint 的合成方式及不支持效果的处理方式。core
提供 source range、最终 layout 几何和调用方提供的绘制定义，不规定全局 z-order。

renderer 对每个 paint 独立判断是否支持。不支持的 `Fill`、`Stroke` 或 `Shadow` 只跳过该 paint；同一 layer
中其余受支持的 paint 继续绘制。renderer 不得以另一种效果替代不支持的 paint，也不得因此跳过整个 layer
或使整段绘制失败。

调用方提供的顺序仍保留为同类重叠声明的输入顺序，以支持现有嵌套颜色范围的后声明覆盖行为。该顺序不用于
规定不同 layer 之间的绘制顺序。

`Stroke` 与 `Shadow` 是纯渲染参数：它们不改变 `LayoutInput` 的度量、断行或范围几何，不改变
`LayoutResult.size`、line box、cluster box 或 glyph bounds。renderer 和宿主应在自身可见区域内避免
额外裁切这些效果；宿主 clip 仍由宿主控制。彩色 emoji 的 stroke 和 shadow 只在 renderer 能可靠重放时
提供。

### 迁移边界

本次变更调整输入构造与 layout 后的范围绘制旁路模型。`LayoutInput` 仍是 layout pipeline 的唯一输入；
统一 rich-text 记录不新增或改变 layout 算法规则。`Decoration` 与 `Annotation` layer 通过 range 与 kind
引用既有 layout 输入产生的结果。`LayoutResult` 暂不持有 rich-text 记录，调用方继续在 renderer 调用时
同时持有 `LayoutResult` 与构造输出中的 `rich_text`。

本迭代实现 core 类型、builder lowering、现有颜色和 rich-text 迁移、查询适配与测试。demo 对新增的
stroke 和 shadow 可暂时不绘制，但不得用另一种视觉效果替代或将其作为 layout 参数处理。

### Invariants

- source text 与 Unicode scalar range 语义保持不变；
- 视觉 layer 与语义可在同一 range 或相交 range 上并存；
- `RichTextLayer.paints` 的顺序不定义 renderer 的全局绘制顺序；
- `Fill`、`Stroke` 与 `Shadow` 可同时作用于一个 layer；
- 缺少 `Fill` 只表示不绘制填充；renderer 不得添加隐式颜色或填充 fallback；
- `Stroke` 的颜色与宽度由声明提供；line cap、line join 与 miter limit 不属于公共 rich-text 数据；
- `Shadow` 的颜色、位移、blur 与 spread 由声明提供；缺少 `Shadow` 时不绘制阴影；
- `RichTextBackgroundPaint` 与 `RichTextLinePaint` 只描述其对应 layer 的对象几何和相邻范围间距，
  不重复定义填充、边框或阴影；
- `RichTextLinePaint.thickness` 是实线、虚线与点线的基础尺寸；点线以它作为圆点直径，
  `Stroke.width` 描述已生成线条形状的外缘描边；
- `RichTextLinePaint` 的 `Default` 为 `thickness = 1.0`、`Solid` 与零 clearance；renderer 不从字号推导
  line thickness；
- `RichTextBackgroundPaint` 的 `Default` 与 builder 默认值只解析背景对象几何，不生成或补充 paint；
- Background layer 的非零 `horizontal_padding` 继续生成同 range 的 `InlineBoxSpan`，使水平 padding 参与既有布局输入；
- `inline_code()` 的视觉数据使用 Background layer；其等宽等局部样式继续使用 `TextSpan`，技术范围语义
  使用 `TechnicalInline`，并继续生成既有 technical layout 输入；
- `inline_code` 系列接收 `TextStyleOverride` 与 `RichTextBackgroundPaint`；背景通用 paint 在 InlineCode
  内容对象的完整范围确定时从当前声明取得；
- 可绘制对象从内向外查找类型和 kind 匹配的 layer；找不到时使用当前 paint 栈的最近值；
- 最近含匹配 layer 的 scope 完整覆盖外层同类声明；同 scope 的同类 layer 按输入顺序全部保留；
- 仅相邻且 layer 列表完全相同的文本输出记录可以合并；合并后的 `RichTextSpan` 首尾是对应的
  `source_boundaries`，内部 `push()` 分界不保留；
- 完全相同 range 的 layers 与语义归并为一个 `RichTextSpan`；不同 range 保持独立并允许相交；
- `*_rich_text` 只更新当前 layer 声明；`push()` 追加文本时按其实际 range 快照文本相关 layer；
- Decoration、Ruby、InlineBox、Link、Technical 与 InlineCode 沿用现有 layout 记录或 semantic 的范围确定时机；
  每次 `push()` 为活跃 Decoration 与 Ruby scope 记录局部 layer。未查找到匹配 layer 时，Decoration 与 Ruby
  使用当时的当前 paints 生成同 kind 的回退 layer；consumer 以 range 交集和 kind 关联局部 layer 与 layout 输出；
- `.paints(...)`、`with_paints` 与 `try_with_paints` 的当前 paints 完整替换外层集合；未查找到 `Text` layer
  的正文 range 使用当时的副本生成 `Text` layer；
- `*_rich_text` 使用 `&[RichTextLayer]`，不再接收 `RichTextRole`；
- rich-text layer 自身不新增或改变 layout 规则；`Decoration` 与 `Annotation` 的布局效果仍仅由对应的
  `LayoutInput` 记录产生；
- renderer 必须基于 `LayoutResult` 的现有几何重放范围，不能重新 shaping 或自行推导断行；
- `source_boundaries` 继续为纯渲染和交互范围提供精确几何边界。

## Consequences

### 正面影响

- 颜色与其他富文本旁路记录使用同一 range 输出和 builder 范围生成规则；
- 一项 range 可直接表达文本、背景、下划线、删除线、CLREQ decoration 和 ruby / bopomofo 的组合，并允许
  各对象使用不同绘制定义；
- stroke 与 shadow 的模型可先进入 core，不依赖某一 renderer 已实现全部绘制能力；
- renderer 能按平台能力和策略决定 layer 的绘制及合成顺序；
- 后续若为富文本增加 core 查询，查询可以面向统一 `RichTextSpan`，不需要同时接受颜色与富文本列表。

### 需要接受的变化

- `ColorSpan`、旧的单 role `RichTextSpan` 和旧 `RichTextPaint` 字段模型将被替换；
- 所有 renderer、demo 和测试须改为消费统一 `rich_text` 列表；
- 现有背景、下划线和删除线查询须从 role 单值模型调整为按 layer 筛选；
- decoration 与 annotation renderer 须按 range 和 kind 从统一 layer 取得 paint，同时继续消费 core 已解析的
  装饰几何和注文 glyph placement；
- renderer 需要按 paint 独立跳过不支持的 stroke、shadow 或彩色 emoji 效果。

## Alternatives considered

- **保留 `ColorSpan` 并只扩展旧 `RichTextPaint`。** 否决：颜色与富文本范围仍有两套输出列表，且
  `RichTextPaint` 会继续混合多个 role 专属字段。
- **让 `RichTextSpan` 只保留一个 layer。** 否决：同一范围的文本、背景、下划线和删除线需要重复
  range，无法直接表达各对象不同的绘制定义。
- **以 `fill`、`stroke` 和 `shadow` 的固定字段代替 paint 列表。** 否决：不能表达多个同类绘制定义，
  并会将 renderer 的合成策略固定到 core 数据结构中。
- **由 core 规定 layer 与 paint 的全局绘制顺序。** 否决：不同 renderer 的能力和策略不同，core 只提供
  调用方提供的绘制定义与布局几何。
- **将 rich-text 记录直接加入 `LayoutInput`。** 否决：这些记录不影响当前 layout 算法；保留在构建输出
  可维持 layout 输入边界。

## Verification

实施完成时至少验证：

1. `ParagraphBuilder` 的颜色便利方法生成文本 layer 的 `Fill`，且不再生成 `ColorSpan`；
2. builder 对所有非空 rich-text layer 和语义范围维护 `source_boundaries`；
3. 嵌套和相交 span 可保留多个 layer，文本 fill 的后声明覆盖语义与当前颜色范围一致；连续且相同的文本
  layer 收敛后只保留合并范围的 `source_boundaries`；
4. 背景、下划线、删除线继续从 `LayoutResult` 的范围几何生成逐行绘制片段；
5. decoration 与 ruby / bopomofo 可分别取得不同于正文和通用线条的 paint，且其 layout 结果保持不变；
6. `Stroke` 与 `Shadow` 不改变 layout dump、`LayoutResult.size`、line、cluster 或 glyph bounds；
7. demo 继续重放现有 fill、背景、线条、CLREQ decoration 和注文；尚未支持的 stroke 与 shadow 只跳过
  对应 paint，同 layer 的受支持 paint 继续绘制；
8. `cargo test --all-targets`、`cargo check`、`git diff --check` 和相关文档风格检查通过。
