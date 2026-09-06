# 2026-09-05 段落输入构造 API（讨论中）

> 状态：已完成实现，等待提交
> 最后更新：2026-09-06

## 文档用途

本文档是 tiqian-rs 段落输入构造 API 的设计与实施依据。后续讨论应直接更新本文档，使背景、已确认决策、待讨论问题、实施步骤和验证要求始终可以独立阅读。

文中的 Rust 代码用于说明 API 语义。除“已确认决策”明确列出的内容外，类型名、方法名、参数顺序和错误变体仍可调整。

## 背景

当前公开输入以 `LayoutInput` 和 `TiqianTextContent` 为中心。调用方先准备完整 source text，再分别构造下列 range 型数据：

- `TiqianTextContent.spans`；
- `source_boundaries`、`line_break_spans` 和 `auto_space_suppressed_ranges`；
- `decorations` 和 `ruby_spans`；
- `inline_boxes` 和 `inline_objects`。

所有范围均使用 Unicode scalar offset。该表示适合现有 layout pipeline，但手工构造段落时，调用方需要重复完成以下工作：

1. 先声明完整文本；
2. 查找目标子串或自行计算 scalar offset；
3. 创建 `TextRange`；
4. 将范围分别写入多个 span 数组；
5. 为需要精确几何的范围同步维护 `source_boundaries`。

`examples/paragraph_demo/sample.rs` 集中体现了这些重复操作。Kotlin 侧的 `buildAnnotatedString`、`append`、`withStyle` 和 ruby、着重号等辅助方法则允许调用方按阅读顺序追加文本，并在追加时声明当前作用域。Kotlin adapter 最终仍将内容转换为 range 型核心输入，因此调用体验的差异来自构造 API，无需更换 layout pipeline 的输入表示。

除手工构造外，还存在 parser 驱动的使用方式。嵌套语法可以先生成树，再递归访问；也可以产生开始标签、文本和结束标签组成的事件流。前者适合作用域闭包，后者适合显式开始和结束作用域。新的构造 API 需要同时支持这两类调用方。

## 目标

本迭代提供一层位于现有输入模型之上的段落构造 API，并满足以下要求：

1. 调用方按内容顺序追加文本，无需计算 `TextRange` 或维护 scalar offset；
2. builder 自动生成 `LayoutInput`、`TiqianTextContent` 及现有各类 span；
3. builder 同时输出当前渲染仍需独立传递的颜色和富文本范围；
4. 手工构造可使用闭包表达嵌套作用域；
5. parser 和事件流可使用显式的 `push_*` 与 `pop`；
6. 两种调用方式共享同一个 builder 状态和同一套范围生成规则，并可在严格嵌套的前提下混用；
7. 构建失败时不返回部分有效的构建输出；
8. 现有 layout pipeline、range 型输入和已有公开 API 在本迭代中保持不变。

## 范围

### 包含

- 在 `crate::api` 中新增高层 `ParagraphBuilder`；
- 按顺序追加 source text；
- 段落默认配置和局部作用域的构造接口；
- 闭包式与显式栈式作用域；
- 自动生成现有 range 型字段和 `source_boundaries`；
- 作用域配对与最终构建状态检查；
- 在迭代最后使用 builder 改写 Rust paragraph demo 的全部段落，包括颜色和 rich-text 绘制范围；
- builder 的单元测试、集成测试和必要文档更新。

### 不包含

- 修改现有 layout、shaping、fallback、断行或绘制算法；
- 将新 builder、构造期配置类型或构造逻辑混入既有核心输入模型模块；
- 新增公开的段落内容树、文本片段模型或 annotation ID；
- 改变 `LayoutInput`、`TiqianTextContent` 或现有 span 的语义与可见性；
- 支持非 LIFO 的交叉作用域；
- 决定旧 range 型构造 API 的长期公开状态；
- 修改其他仓库的 parser、输入模型或迁移代码。

其他仓库只用于说明可能的调用形式，不构成本迭代的实现约束。

## 已确认决策

### 1. 在现有输入模型上增加构造层

在 `crate::api` 中新增 `ParagraphBuilder`。其最终产物为 `ParagraphBuildOutput`，统一携带现有 `LayoutInput` 与当前 renderer 仍需的绘制范围：

```text
按顺序追加文本并声明作用域
    ↓
ParagraphBuilder
    ↓ 自动生成全文、scalar range、各类 span 和精确几何边界
ParagraphBuildOutput
    ├─ input: LayoutInput
    ├─ colors: Vec<ColorSpan>
    └─ rich_text: Vec<RichTextSpan>
    ↓
现有 layout pipeline 与 renderer
```

`ParagraphBuilder` 只在构造期间存在。`ParagraphBuildOutput.input` 是现有 layout pipeline 的唯一输入；`colors` 与 `rich_text` 由 renderer 按 source range 消费。

`ParagraphBuildOutput` 公开 `input`、`colors` 和 `rich_text` 字段，调用方可直接读取或解构。builder 的便利在于按顺序追加 source 时自动保持范围与 `source_boundaries` 同步。后续输入汇聚迭代若将颜色和 rich text 纳入 `LayoutInput`，应收缩该输出类型，并保持 builder 调用不变。

现有 `LayoutInputBuilder` 接收已经构造完成的 `TiqianTextContent`，并为 `LayoutInput` 的字段赋值。新 builder 负责从文本追加操作生成这些字段。两者职责不同，本迭代不修改或替换 `LayoutInputBuilder`。

`ParagraphBuilder`、`ParagraphBuildOutput`、`TextStyleOverride`、`RubyAnnotation`、`InlineBoxStyle`、`InlineObjectMetrics`、`ParagraphBuildError`、`ParagraphScopeKind`、`ParagraphPositionInsertionKind` 及其内部辅助类型都定义在新增的 `crate::api` 模块。它们依赖并产出现有 core 类型，但不放入 `core::text_model` 或其他现有核心模块。除添加模块声明和必要的公开重导出外，原有核心代码尽量不因构造 API 调整。

crate 根只新增 `pub mod api`。调用方从 `tiqian::api` 导入本迭代新增类型，不在 crate 根重导出 `ParagraphBuilder` 等名称。`tiqian::api` 不重导出 `LayoutInput`、`TextStyle`、`RichTextSpan` 或其他既有 core 类型；调用方继续从其原有模块路径导入这些类型。

### 2. 不公开第二套内容描述类型

本迭代不新增要求调用方直接构造的 `Paragraph`、`InlineContent`、`InlineFragment`、`TextRun` 或 annotation 引用表。builder 内部可以保存尚未结束的作用域和待生成 span，但这些状态不进入稳定公开模型。

公开新增类型限于构造过程所需的 builder、构建输出、错误类型及错误上下文类别。`ParagraphBuildOutput` 公开其输出字段。`ParagraphScopeKind` 与 `ParagraphPositionInsertionKind` 只描述错误相关的公开类别。显式作用域不公开 identifier。

### 3. 闭包与显式作用域共用一个 builder

同一个 `ParagraphBuilder` 提供两种调用方式：

- 闭包方法供 demo、测试和手工构造使用；
- `push_*` 与无参数 `pop` 供 parser、事件流和动态遍历使用。

两者操作同一个内部作用域栈，使用相同的当前 scalar offset，并在作用域结束时生成相同的现有 span。闭包方法是自动配对的便利接口，不建立独立实现路径。

### 4. 显式作用域严格按 LIFO 关闭

显式方法按以下形式工作：

```rust
builder.push_text_style(style)?;
builder.push("OpenType");
builder.pop()?;
```

`pop()` 关闭当前栈顶作用域。显式作用域只接受严格嵌套：

```text
开始 A
  开始 B
  结束 B
结束 A
```

以下交叉范围不属于当前能力：

```text
开始 A
  开始 B
结束 A
结束 B
```

当前已确认的手工构造、嵌套语法树和有效嵌套标签事件流均可使用 LIFO。若以后出现必须构造交叉范围的实际需求，应为该需求单独设计带 identifier 的范围 API，不改变普通作用域的含义。

### 5. `push_*` 不返回公开 identifier

`push_*` 返回成功或错误，`pop()` 不接收参数。理由如下：

- 严格 LIFO 下，唯一允许关闭的是栈顶作用域；
- 已验证的事件流在 parser 阶段已经保证标签匹配；
- identifier 不能防止调用方忘记关闭，最终仍需由 `build()` 检查；
- identifier 只有在允许关闭非栈顶范围时才增加表达能力，而该能力不在当前范围内。

builder 内部仍可为每个作用域分配私有序号，用于检查闭包开始的作用域是否由闭包自身正确结束。该序号不通过公开 API 暴露。

### 6. 错误由显式操作和 `build()` 共同报告

显式 `pop()` 在作用域栈为空时立即返回错误。`build()` 至少检查：

- 是否存在未关闭作用域；
- builder 是否已经记录闭包边界错误；
- 是否发生阻止生成一致构建输出的构造错误。

发生错误时不自动关闭作用域，不猜测目标范围，也不返回部分构造结果。最终签名方向为：

```rust
pub fn build(self) -> Result<ParagraphBuildOutput, ParagraphBuildError>;
```

`ParagraphBuildError` 是 `crate::api` 的公开枚举，使用可 match 的结构性类别而非仅保存错误字符串。它实现 `Display` 与 `std::error::Error`，使 parser 可按错误类别处理或向上层传播。`ParagraphScopeKind` 与 `ParagraphPositionInsertionKind` 为错误变体提供作用域和位置插入操作的公开类别。

普通 `with_*` 闭包不返回 `Result`。闭包内出现只能延迟发现的构造错误时，builder 记录第一个错误，继续执行闭包，并由最终 `build()` 返回该错误。`try_with_*` 与显式 `push_*`、`pop()` 保持即时 `Result`，调用方可用 `?` 在调用点传播错误。

错误类别包括：

- 空作用域栈上的 `pop()`；
- `build()` 时仍存在未关闭作用域；
- 闭包返回时作用域栈未恢复到进入闭包前的边界；
- ruby 或 inline box 的空范围；
- 行内对象的空替代文本；
- ruby 范围内插入 hard break 或 inline object。

枚举变体携带相关作用域类别或操作类别，但不暴露内部私有序号、文本缓冲区或已生成 span。记录首个错误后，普通闭包中的后续文本与 scope 操作继续执行；无论其后状态如何，`build()` 都返回已记录的错误，不返回构建输出。

### 7. 颜色与 rich-text 使用统一作用域接口

颜色使用 core 已有的 `i32` ARGB 值，生成 `ParagraphBuildOutput.colors` 中的 `ColorSpan`。它不使用平台 renderer 的颜色类型：

```rust
builder.with_color(0xFF2563EB_u32 as i32, |builder| {
    builder.push("待校");
});
```

background、underline 和 line-through 等 rich-text role 共用 `RichTextRole` 与 `RichTextPaint`，生成 `ParagraphBuildOutput.rich_text` 中的 `RichTextSpan`：

```rust
builder.with_rich_text(
    RichTextRole::Underline,
    RichTextPaint::builder()
        .line_pattern(RichTextLinePattern::dashed(1.0, 3.0, 2.0))
        .build(),
    |builder| {
        builder.push("存疑内容");
    },
);
```

两类作用域同时提供 `with_*` / `try_with_*` 与 `push_*` / `pop()` 形式，并复用普通作用域的栈与闭包边界检查。每个非空范围的起止位置自动写入 `source_boundaries`。后续可增加 `background`、`underline` 等便利方法；便利方法只调用这一组通用操作，不建立第二套 scope 语义。

空颜色或 rich-text 作用域被忽略，不生成 span 或 `source_boundaries`。这与空 `TextStyleOverride`、`DecorationKind`、`LineBreakPolicy` 和自动间距抑制作用域一致。

### 8. 链接使用专用作用域，并自动应用地址断行规则

链接使用 `with_link(target, ...)`、`try_with_link(target, ...)` 和 `push_link(target)`，调用方不必手工构造 `RichTextRole::Link`：

```rust
builder.with_link("https://tiqian.org", |builder| {
    builder.push("tiqian.org");
});
```

链接 scope 结束时生成 `ParagraphBuildOutput.rich_text` 中的 `RichTextSpan { role: RichTextRole::Link { target }, .. }`，并自动加入 `source_boundaries`。当前 renderer 不为 link role 指定视觉 fallback 或导航行为；链接 target 供后续 frontend 与 accessibility 消费。

builder 以该 scope 的完整可见 source text 调用现有 `link_address_display::displays_address`。结果为 true 时，builder 同时生成同范围的 `LineBreakSpan { policy: LineBreakPolicy::ProgressiveTechnical }`；普通链接文字只生成链接范围，保持正文断行规则。空链接 scope 被忽略。

### 9. 技术文本与 inline code 分别表达布局策略和绘制语义

技术文本使用 `with_technical(...)`、`try_with_technical(...)` 与 `push_technical()`。非空技术范围自动生成同范围的 `LineBreakSpan { policy: LineBreakPolicy::ProgressiveTechnical }` 和 `auto_space_suppressed_ranges` 条目：

```rust
builder.with_technical(|builder| {
    builder.push("Machine2Machine");
});
```

技术作用域只表达现有 layout 所需的断行与自动间距策略，不生成 `RichTextSpan`，也不指定字体或视觉样式。空技术作用域被忽略。

不公开通用的 `with_line_break(policy, ...)` 或 `push_line_break(policy)`。当前 `LineBreakPolicy` 仅有 `ProgressiveTechnical`，因此以 `with_technical` 表达该既有语义；未来新增断行策略时，按实际调用需求单独扩展 builder 接口。

inline code 使用 `with_inline_code(style, paint, ...)`、`try_with_inline_code(style, paint, ...)` 与 `push_inline_code(style, paint)`。调用方以 `TextStyleOverride` 提供 monospace 字体族及其他局部样式，builder 不固化字体选择；作用域生成解析后的 `TextSpan` 与 `RichTextRole::InlineCode`：

```rust
builder.with_inline_code(
    TextStyleOverride::builder()
        .font_families(vec!["monospace".to_owned()])
        .build(),
    code_paint,
    |builder| {
    builder.push("editorial-notes.md");
    },
);
```

inline code 不隐式生成技术断行或自动间距抑制。调用方需要该布局策略时，将 `with_inline_code` 嵌套在 `with_technical` 中，或反向嵌套；两者对同一非空 source range 分别生成各自既有输入字段。

自动间距抑制也提供 `with_auto_space_suppressed(...)`、`try_with_auto_space_suppressed(...)` 与 `push_auto_space_suppressed()`。非空作用域生成 `auto_space_suppressed_ranges` 中的一个范围，供不需要 `ProgressiveTechnical` 断行策略的逐字文本使用。空作用域被忽略。

## 方案概要

### 对外职责

调用方负责：

- 配置段落默认样式、约束和 profile；
- 按逻辑顺序追加文本或行内内容；
- 通过闭包或显式栈声明局部样式和附加语义。

builder 负责：

- 拼接 source text；
- 维护当前 Unicode scalar offset；
- 记录每个作用域的开始位置和配置；
- 在作用域结束时创建现有 range 型 span；
- 为没有其他 layout span 的精确几何范围汇总 `source_boundaries`；
- 构造 `TiqianTextContent` 和 `LayoutInput`；
- 拒绝未闭合或状态不一致的构建结果。

### 内部状态

概念上的内部状态如下，具体字段可在实现时按现有类型调整：

```text
ParagraphBuilder
├─ source text 缓冲区
├─ 当前 scalar offset
├─ 段落默认 TextStyle
├─ ParagraphStyle / constraints / profile
├─ open scopes
├─ TextSpan / line-break range / autospace range
├─ DecorationSpan / RubySpan
├─ InlineBoxSpan / InlineObjectSpan
├─ source boundaries
└─ 第一个构造错误（如采用延迟报告）
```

每个内部作用域至少保存：

- 作用域类别；
- 开始 scalar offset；
- 创建目标 span 所需的数据；
- 供闭包边界检查使用的私有序号。

## 方案细节

### 文本追加与 scalar offset

基础操作按调用顺序追加 source text：

```rust
builder.push("中文书刊经常夹用 Latin letters、");
builder.push("OpenType");
builder.push(" 字体名称。");
```

追加后，builder 按 Unicode scalar 数量更新当前位置。UTF-8 byte length 不得作为 source offset 使用。最终正文转换为现有 `Text`，所有生成范围继续使用半开 scalar 区间 `[start, end)`。

基础文本追加方法采用 `push(&str)`。它不要求调用方构造 `Text` 或发生隐式分配；持有 `String` 的 parser 可传入 `&text`。不为 `String`、`Cow` 等额外文本类型扩大核心签名。

### 段落默认配置

`ParagraphBuilder` 需要收集现有 `LayoutInput` 的段落配置字段：

- `constraints`；
- `text_style`；
- `paragraph_style`；
- `profile_id`。

采用创建可变 builder、按需配置和追加内容、最后调用 `build()` 的主流程：

```rust
let mut builder = ParagraphBuilder::new(constraints);
builder
    .text_style(body_style)
    .paragraph_style(paragraph_style)
    .profile_id(profile_id);

builder.push("正文");
let output = builder.build()?;
```

段落配置方法接收 `&mut self` 并返回 `&mut Self`，使调用方可以连续配置或按条件修改。首次向 source 追加内容后，调用 `text_style`、`paragraph_style` 或 `profile_id` 会 panic；`push`、`hard_break` 和 `inline_object` 都属于 source 追加。constraints 只在 `new(constraints)` 中设置。这避免新默认配置回溯影响先前已追加的文本。parser 可以在整个事件处理期间持有同一个 builder。当前不增加仅用于缩短手工构造的闭包式总入口。

### 局部文本样式

局部文本样式作用域接收公开的 `TextStyleOverride`。该类型的每个字段表示“覆盖当前值”或“保持继承”，不能以 `TextStyle::default()` 代替未填写的字段：

```rust
builder.with_text_style(
    TextStyleOverride::builder()
        .font_families(vec!["Inter".to_owned()])
        .build(),
    |builder| {
        builder.push("OpenType");
    },
);
```

进入作用域时，builder 将 override 应用到当前生效的完整 `TextStyle`；退出作用域时恢复外层完整样式。每次 `push(&str)` 使用当时的完整样式生成现有 `TextSpan`。因此，`TextStyleOverride` 只表达构造过程中的继承规则，最终 `LayoutInput` 仍只保存完整 `TextStyle`。

`TextStyleOverride` 是 `crate::api` 中的公开 struct。它为 `TextStyle` 的每个当前字段保存 `Option<T>`：`None` 继承当前完整样式，`Some(value)` 覆盖当前值。它覆盖字体族、字号、locale、字重、italic、baseline shift 和 inline attachment。调用方可用 `Some(false)`、`Some(400)`、`Some(Vec::new())` 或 `Some(InlineAttachment::None)` 等值显式恢复字段的默认语义。

该类型提供 builder 构造入口。builder 在打开文本样式或 inline code scope 时，将 override 应用到当前完整样式；文本追加仍按已确认规则生成完整 `TextSpan`，不保留 override 本身或嵌套历史。

builder 输出的 `TextSpan` 表示每段文本的完整生效样式，而不保留嵌套作用域历史。每次追加文本时：

1. 当前完整样式等于段落默认 `text_style` 时，不生成 `TextSpan`；
2. 当前完整样式与前一个 builder 生成的相邻 `TextSpan` 相等时，扩大前一个 span 的结束位置；
3. 其他情况生成不与已有 builder 生成 span 重叠的新 `TextSpan`。

因此，builder 生成的 `content.spans` 不依赖数组写入顺序来表达样式覆盖。调用次数不同但文本和最终样式相同的构造结果保持一致。

### 闭包式作用域

手工构造的目标形式如下：

```rust
builder.push("中文书刊经常夹用 Latin letters、");

builder.with_text_style(inter_style, |builder| {
    builder.push("OpenType");
});

builder.push(" 字体名称。");
```

闭包方法执行以下内部步骤：

1. 记录进入闭包前的栈深度；
2. 打开对应作用域并记录私有序号；
3. 执行调用方闭包；
4. 检查闭包返回时栈顶仍为该作用域；
5. 关闭作用域并生成 span，或记录第一个构造错误。

若闭包内部使用显式 `push_*` 打开了子作用域却未关闭，闭包方法不得用自己的结束操作误关该子作用域。builder 应记录边界错误，并由 `build()` 返回。

### 显式栈式作用域

parser 或线性事件流可将事件直接映射到 builder：

```rust
match event {
    Event::StartEmphasis => builder.push_decoration(DecorationKind::Emphasis)?,
    Event::StartStyle(style) => builder.push_text_style(style)?,
    Event::Text(text) => builder.push(text),
    Event::End => builder.pop()?,
}
```

该接口假设上游事件表示严格嵌套结构。`pop()` 只关闭栈顶，不按类型搜索栈，不自动关闭多个作用域。空栈上的 `pop()` 返回错误；输入结束后仍有作用域则由 `build()` 返回错误。

若 parser 先生成嵌套树，也可以在递归访问中使用闭包方法。构造 API 不要求 parser 采用其中一种中间表示。

### 两种方式混用

闭包和显式作用域可以嵌套，但闭包返回时不得遗留在该闭包内显式打开的作用域。

显式外层、闭包内层：

```rust
builder.push_inline_box(box_style)?;
builder.push("欢迎使用");

builder.with_ruby(ruby, |builder| {
    builder.push("提椠");
});

builder.pop()?;
```

闭包外层、正确关闭的显式内层：

```rust
builder.try_with_inline_box(box_style, |builder| {
    builder.push("欢迎使用");
    builder.push_ruby(ruby)?;
    builder.push("提椠");
    builder.pop()?;
    Ok(())
});
```

第二段使用 `try_with_inline_box`，以便闭包内的显式操作通过 `ParagraphBuildError` 向外传播。

### Ruby

Ruby 作用域接收轻量的 `RubyAnnotation` 配置值。该类型只描述注文，不描述 base source range：

```rust
builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
    builder.with_text_style(name_style, |builder| {
        builder.push("提椠");
    });
});
```

`RubyAnnotation` 承载现有 `RubySpan` 的注文文本、`RubyKind`、字体族和 locale。它应提供 `pinyin` 与 `bopomofo` 构造入口，以及设置字体族和 locale 的 builder 方法。Ruby 作用域结束时，builder 以该作用域的非空连续 source range 和 annotation 配置生成现有 `RubySpan`。

`with_ruby`、`try_with_ruby` 与 `push_ruby` 都接收 `RubyAnnotation`，不接收带占位 range 的 `RubySpan`。单段文本便利方法使用 `ruby(annotation, text)`，配置在前、base source text 在后：

```rust
builder.ruby(RubyAnnotation::pinyin("tíqiàn"), "提椠")?;
```

`RubyAnnotation::pinyin(&str)` 与 `RubyAnnotation::bopomofo(&str)` 接收借用的注文文本，并在构造时转换为 core `Text`。字体族与 locale 通过 `RubyAnnotation` 的 builder 设置，调用方不需手工构造 `Text`。

Ruby base 允许普通文本、`TextStyleOverride`、decoration、技术文本、自动间距抑制和 inline box。Ruby base 不允许 hard break 或 inline object；若在 Ruby 作用域内调用这些位置插入操作，builder 返回 `ParagraphBuildError`。这保持 base 为可按现有规则布局的连续 source 范围。

### 嵌套作用域生成重叠 range

严格嵌套可以自然生成多个覆盖同一文本的 span。例如：

```rust
builder.with_inline_box(box_style, |builder| {
    builder.with_ruby(ruby, |builder| {
        builder.push("提椠");
    });
});
```

若“提椠”位于 scalar range `0..2`，builder 生成：

```text
content.text = "提椠"
InlineBoxSpan.range = 0..2
RubySpan.base_range = 0..2
```

这类重叠不要求交叉关闭，也不需要公开片段节点或 annotation identifier。内部 range 模型继续负责表达多个 span 对同一 source 范围的覆盖。

### 文本型便利方法

常见的“打开作用域、追加一段文本、关闭作用域”可以提供更短的方法：

```rust
builder.ruby(RubyAnnotation::pinyin("tíqiàn"), "提椠")?;
builder.emphasis("斤斤计较");
builder.proper_noun("北京大学");
```

为所有常用语义提供单段文本便利方法，包括局部文本样式、四种 `DecorationKind`、ruby、颜色、通用 rich text、链接、技术文本、inline code、inline box 和自动间距抑制。每个便利方法都只调用对应的 scope 操作与 `push(text)`，不生成独立 range、边界或 span 路径。

便利方法采用无后缀的语义名称，例如 `color`、`rich_text`、`link`、`technical`、`inline_code`、`ruby`、`inline_box`、`auto_space_suppressed`、`emphasis`、`mourning`、`proper_noun` 和 `book_title`。每个便利方法接收对应 scope 的配置与文本；`rich_text` 同时接收 `RichTextRole` 与 `RichTextPaint`。

局部文本样式便利方法命名为 `styled(override, text)`，避免与段落默认配置的 `text_style(...)` 冲突。便利方法收到空字符串时，按对应 scope 的既有规则处理：普通范围不生成输入，ruby 与 inline box 返回 `ParagraphBuildError`。`hard_break()` 与 `inline_object(text, metrics)` 已是当前位置插入操作，不另设文本型便利方法。

### 现有字段的生成

builder 应生成当前 `LayoutInput` 字段与绘制范围，不增加调用方需手工构造的中间内容模型：

| 构造操作 | 生成结果 |
| --- | --- |
| `push` | 追加 `content.text`，更新 scalar offset。 |
| 文本样式作用域 | `TextSpan`。 |
| 技术断行作用域 | `LineBreakSpan`。 |
| 自动间距抑制作用域 | `auto_space_suppressed_ranges`。 |
| decoration 作用域 | `DecorationSpan`。 |
| ruby 作用域 | `RubySpan`。 |
| inline box 作用域 | `InlineBoxSpan`。 |
| inline object 操作 | `InlineObjectSpan` 及其 source 表示。 |
| 颜色作用域 | `ParagraphBuildOutput.colors` 中的 `ColorSpan`。 |
| rich-text 作用域 | `ParagraphBuildOutput.rich_text` 中的 `RichTextSpan`。 |

现有 layout pipeline 已将 `TextSpan`、`DecorationSpan`、`RubySpan`、`InlineBoxSpan`、`InlineObjectSpan` 和 `LineBreakSpan` 的起止位置汇入 cluster 边界。builder 为这些类型生成对应 span 后，不再重复写入 `content.source_boundaries`。

`source_boundaries` 用于没有对应 layout span、但需要精确范围几何的内容。builder 为颜色和 rich-text 作用域自动加入其起止位置；调用方不再手工维护这些边界。

builder 不公开无语义的通用 `source_boundary` 操作。rich text、颜色、链接等纯绘制或交互范围应使用具名构造方法；这些方法由 `crate::api` 维护构造期范围和必要的 `source_boundaries`。颜色与 rich text 先保存至 `ParagraphBuildOutput`，不修改 core 的 range 型表示或 layout pipeline。

### 单点内容与持续作用域

持续覆盖一段 source 的内容使用作用域，例如文本样式、decoration、ruby 和 inline box。

硬换行和行内对象属于当前位置插入操作，不使用 `push_*` 与 `pop`：

```rust
builder.hard_break()?;
builder.inline_object("E = mc²", metrics)?;
```

行内对象要求调用方提供非空替代文本和 `InlineObjectMetrics`：

```rust
builder.inline_object(
    "E = mc²",
    InlineObjectMetrics::new(48.0, 18.0, 6.0),
)?;
```

builder 将替代文本追加到 source，以其自动生成的非空 scalar range 创建现有 `InlineObjectSpan`。layout 使用 metrics 的 `advance`、`ascent`、`descent` 和边界调整占位，不对这段 source 做字体 shaping；替代文本保留复制、搜索、选择和无障碍语义。调用方不手写 `INLINE_OBJECT_REPLACEMENT_CHAR`，也不提供没有替代文本的对象入口。

`InlineObjectMetrics` 是一次对象插入的配置值，包含现有 `InlineObjectSpan` 除 range 外的字段：`advance`、`ascent`、`descent`、`leading_boundary` 和 `trailing_boundary`。`InlineObjectMetrics::new(advance, ascent, descent)` 使用现有 fixed boundary 默认值；需要两侧调整规则时使用其 builder。它不保存对象身份、renderer key 或 source range。

inline box 使用 `InlineBoxStyle`，包含 `InlineBoxSpan` 除 range 外的 `inline_start`、`inline_end` 和 `outer_spacing`。`with_inline_box`、`try_with_inline_box` 与 `push_inline_box` 都接收该配置，builder 在 scope 关闭时生成 range。单段文本便利方法使用 `inline_box(style, text)`，配置在前、正文在后。

`hard_break()` 始终向 source 追加单个 LF 字符 `\n`，并使用现有 mandatory-break pipeline。该操作的 scalar offset 固定增加 1。调用方通过 `push(&str)` 传入的 LF、CR 和 CRLF 保持原样，以便 parser 保留外部输入；`hard_break()` 只表达 builder 作者 API 的规范换行。

### 空作用域

builder 按作用域类别处理开始位置等于结束位置的情况，不能让空 range 流入下游：

| 类别 | 空作用域处理 |
| --- | --- |
| `TextStyleOverride`、`DecorationKind`、`LineBreakPolicy`、自动间距抑制 | 忽略，不生成 range。 |
| Ruby | 返回 `ParagraphBuildError`。注文没有 base text 时无法生成 `RubySpan.base_range`。 |
| inline box | 返回 `ParagraphBuildError`。空范围没有首尾 cluster，无法施加盒边 advance。 |
| inline object、hard break | 不适用；它们属于当前位置插入操作。 |

parser 若要把空标签视为输入错误，应在 parser 自己的语法或 AST 校验阶段处理。builder 的错误只针对无法生成现有 `LayoutInput` 语义的 Ruby 和 inline box。

### 相邻作用域与顺序

`TextSpan` 在追加文本时生成最终生效样式范围，不依赖数组顺序表达覆盖。其他构造输出按 scope 打开顺序保存：外层先、内层后；同一层按调用顺序保存。

`ParagraphBuildOutput.colors` 采用此顺序后，现有 renderer 的逆序 `color_at` 查询会优先命中后打开的内层颜色。`ParagraphBuildOutput.rich_text` 保留所有非空范围，包括相交和相同范围；renderer 按输出顺序绘制，因此后打开的 rich-text 在视觉上位于后绘制层。

`line_break_spans` 与 `auto_space_suppressed_ranges` 不使用覆盖语义。builder 保留每个非空作用域生成的范围，嵌套相同策略也不合并或去重。不为减少 span 数量引入额外规范化。

## 错误处理

### 必须报告的结构错误

至少包括：

- 空栈上调用 `pop()`；
- 调用 `build()` 时仍有未关闭作用域；
- 闭包返回时存在闭包内未关闭的显式子作用域；
- 已记录结构错误后尝试生成 `LayoutInput`。

错误信息应包含足以定位问题的作用域类别和构建阶段。内部私有序号只用于确定作用域身份，不需要出现在稳定 API 中。

### 闭包错误传播

同时提供普通闭包和可失败闭包入口：

```rust
builder.with_text_style(inter_style, |builder| {
    builder.push("OpenType");
});

builder.try_with_inline_box(box_style, |builder| {
    builder.push_ruby(ruby)?;
    builder.push("提椠");
    builder.pop()?;
    Ok(())
})?;
```

`with_*` 用于不需要立即向外传播错误的常规手工构造，保持 Kotlin 风格的闭包写法。它记录第一个构造错误，最终由 `build()` 返回。`try_with_*` 接收返回 `Result` 的闭包，用于嵌套 helper、条件分支或在闭包中使用可失败显式操作的场景。两者共用相同的内部开始、边界检查和结束操作。

`try_with_*` 的闭包和方法本身都使用 `ParagraphBuildError`：

```rust
builder.try_with_text_style(inter_style, |builder| {
    builder.push_text_style(nested_style)?;
    builder.push("OpenType");
    builder.pop()?;
    Ok(())
})?;
```

不使用泛型调用方错误，也不引入组合错误类型。调用方自己的领域错误应在调用 `try_with_*` 之前处理，或映射为 `ParagraphBuildError` 后再进入 builder。

`try_with_*` 的闭包返回 `ParagraphBuildError` 时，builder 记录首个错误并立即返回该错误。之后 `build()` 必定返回已记录的错误；builder 不回滚文本、span 或作用域栈，也不恢复到进入闭包前的状态。

显式 `pop()` 继续立即返回错误。无论闭包使用哪种入口，`build()` 都检查未关闭作用域和 builder 已记录的结构错误，拒绝生成不一致的构建输出。

builder 不捕获闭包 panic。panic 沿调用栈传播；如果调用方自行使用 `catch_unwind` 恢复执行，必须丢弃发生 panic 的 builder，不得继续追加内容或调用 `build()`。`try_with_*` 只处理 `ParagraphBuildError`，不转换 panic payload，也不尝试回滚文本、span 或作用域状态。

## 公开 API 草案

以下仅固定职责和关系：

```rust
pub struct ParagraphBuilder {
    // 私有状态
}

impl ParagraphBuilder {
    pub fn new(constraints: LayoutConstraints) -> Self;

    pub fn text_style(&mut self, style: TextStyle) -> &mut Self;
    pub fn paragraph_style(&mut self, style: ParagraphStyle) -> &mut Self;
    pub fn profile_id(&mut self, profile_id: LayoutProfileId) -> &mut Self;

    pub fn push(&mut self, text: &str);

    pub fn push_text_style(
        &mut self,
        style: TextStyleOverride,
    ) -> Result<(), ParagraphBuildError>;

    pub fn push_ruby(&mut self, annotation: RubyAnnotation) -> Result<(), ParagraphBuildError>;

    pub fn push_inline_box(&mut self, style: InlineBoxStyle)
        -> Result<(), ParagraphBuildError>;

    pub fn push_decoration(
        &mut self,
        kind: DecorationKind,
    ) -> Result<(), ParagraphBuildError>;

    pub fn push_color(&mut self, argb: i32) -> Result<(), ParagraphBuildError>;

    pub fn push_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
    ) -> Result<(), ParagraphBuildError>;

    pub fn push_link(&mut self, target: String) -> Result<(), ParagraphBuildError>;

    pub fn push_technical(&mut self) -> Result<(), ParagraphBuildError>;

    pub fn push_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
    ) -> Result<(), ParagraphBuildError>;

    pub fn push_auto_space_suppressed(&mut self) -> Result<(), ParagraphBuildError>;

    pub fn hard_break(&mut self) -> Result<(), ParagraphBuildError>;

    pub fn inline_object(
        &mut self,
        replacement_text: &str,
        metrics: InlineObjectMetrics,
    ) -> Result<(), ParagraphBuildError>;

    pub fn pop(&mut self) -> Result<(), ParagraphBuildError>;

    pub fn with_text_style(
        &mut self,
        style: TextStyleOverride,
        content: impl FnOnce(&mut Self),
    );

    pub fn try_with_text_style(
        &mut self,
        style: TextStyleOverride,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_ruby(&mut self, annotation: RubyAnnotation, content: impl FnOnce(&mut Self));

    pub fn try_with_ruby(
        &mut self,
        annotation: RubyAnnotation,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_decoration(&mut self, kind: DecorationKind, content: impl FnOnce(&mut Self));

    pub fn try_with_decoration(
        &mut self,
        kind: DecorationKind,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_inline_box(&mut self, style: InlineBoxStyle, content: impl FnOnce(&mut Self));

    pub fn try_with_inline_box(
        &mut self,
        style: InlineBoxStyle,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_color(&mut self, argb: i32, content: impl FnOnce(&mut Self));

    pub fn try_with_color(
        &mut self,
        argb: i32,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self),
    );

    pub fn try_with_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_link(&mut self, target: String, content: impl FnOnce(&mut Self));

    pub fn try_with_link(
        &mut self,
        target: String,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_technical(&mut self, content: impl FnOnce(&mut Self));

    pub fn try_with_technical(
        &mut self,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self),
    );

    pub fn try_with_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn with_auto_space_suppressed(&mut self, content: impl FnOnce(&mut Self));

    pub fn try_with_auto_space_suppressed(
        &mut self,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError>;

    pub fn build(self) -> Result<ParagraphBuildOutput, ParagraphBuildError>;
}
```

单段文本便利方法沿用已确认的语义名称：`styled`、`ruby`、`emphasis`、`mourning`、`proper_noun`、`book_title`、`color`、`rich_text`、`link`、`technical`、`inline_code`、`inline_box` 与 `auto_space_suppressed`。除 Ruby 与 inline box 外，它们返回 `&mut Self`；Ruby 与 inline box 因空文本可能无效，返回 `Result<&mut Self, ParagraphBuildError>`。

## 不变量

实现和后续讨论必须保持：

1. `ParagraphBuilder::build()` 生成 `ParagraphBuildOutput`；
2. `ParagraphBuildOutput.input` 是现有 layout pipeline 的唯一输入，`colors` 和 `rich_text` 用于当前 renderer 重放；
3. source coordinate 继续使用 Unicode scalar offset；
4. 调用方无需创建 `TextRange` 或维护 `source_boundaries`；
5. 闭包与显式作用域生成相同的 range 型结果；
6. 显式 `pop()` 只关闭当前栈顶；
7. 闭包不能关闭或遗留不属于自己的内部作用域；
8. 构造错误不能产生部分有效输出；
9. source text 保持输入内容，ruby 注文等非 source 内容继续遵守现有模型；
10. builder 不改变 layout pipeline 的规则与结果；
11. 当前已有 range 型入口继续可用。

## 实现前核对

当前公开行为已足以开始编译型 API 草案和实现。`RubyAnnotation`、`InlineBoxStyle` 与 `InlineObjectMetrics` 的构造入口、字段默认值和校验复用对应 core 类型的现有语义；除非实现发现无法由现有语义表达的外部行为，不再单独扩展讨论。

开始实现前应完成两项核对：

1. 用编译型草案验证本节公开签名在闭包、显式栈和 parser 事件流中的借用体验；
2. 将每种 scope 和位置插入操作映射到现有 `LayoutInput` 字段或 `ParagraphBuildOutput` 字段，确认没有遗漏 paragraph demo 现有功能。

paragraph demo 的 `closing` 段落目前以两个同范围 `TextSpan` 分别写入 italic 与粗体大字号。现有 layout 和 renderer 都以数组后项覆盖前项，因此实际生效的是粗体大字号，italic 不生效。迁移时使用嵌套 `TextStyleOverride`，有意改为 italic、粗体与大字号同时生效；该段旧手写输入不作为 layout dump 或 Vello scene 编码摘要的等价基准。

## 实施计划

阶段按数据依赖实施。除最终验收外，各阶段不要求单独可编译、可运行或可测试；前一阶段留下的未完成实现由紧随其后的阶段直接补全，不提供独立的中间 API 或 demo 迁移状态。已确认的不变量在所有阶段都必须保持。

### Phase 1：建立 API 模块与构造状态

1. 新增 `crate::api` 模块和 crate 根的 `pub mod api`；
2. 定义 `ParagraphBuilder`、`ParagraphBuildOutput`、`ParagraphBuildError`、`TextStyleOverride`、`RubyAnnotation`、`InlineBoxStyle` 和 `InlineObjectMetrics`；
3. 建立 source text 缓冲、Unicode scalar offset、段落默认配置、各类待生成字段、开放作用域栈和首个构造错误的私有状态；
4. 建立段落默认配置锁定规则，以及 `TextStyleOverride` 应用到完整 `TextStyle` 的规则。

完成条件：内部状态可以表示文档中全部构造操作及其最终输出，不要求此时公开方法已完整实现。

### Phase 2：实现统一的作用域机制

1. 定义内部作用域类别、开始 scalar offset、结束时所需配置和闭包边界检查序号；
2. 实现显式 `push_*`、无参数 `pop()`、严格 LIFO 关闭和结构错误记录；
3. 实现 `with_*` 与 `try_with_*` 共用的开始、结束和闭包边界检查；
4. 实现文本追加、完整样式解析和 `TextSpan` 的相邻合并；
5. 实现 `build() -> Result<ParagraphBuildOutput, ParagraphBuildError>` 的最终状态检查。

完成条件：所有作用域使用同一套栈和结束操作；尚未接入的 scope 可在下一阶段补齐其输出生成。

### Phase 3：补齐现有输入字段的生成

1. 接入 decoration、ruby、inline box、技术文本、自动间距抑制、颜色与通用 rich-text scope；
2. 接入 link 和 inline code 的复合输出规则；
3. 接入 `hard_break` 与 `inline_object` 当前位置插入操作；
4. 为颜色、rich text 和链接生成 `source_boundaries`，并为其余操作生成对应的现有 `LayoutInput` 字段；
5. 实现已确认的单段文本便利方法。

完成条件：每个已确认的公开构造操作都可生成其对应的 `LayoutInput` 或 `ParagraphBuildOutput` 字段，不要求 demo 已迁移。

### Phase 4：加入测试并迁移 paragraph demo

1. 为 scalar offset、空范围、显式栈、闭包边界、混用和嵌套作用域添加测试；
2. 为各类 span、当前位置插入操作、`source_boundaries` 和 `ParagraphBuildOutput` 的颜色与 rich text 添加测试；
3. 在测试中保留旧手写构造作为迁移对照，并比较 source text、layout dump 和 Vello scene 编码摘要；
4. 使用 builder 改写 paragraph demo 的全部段落，移除 demo 中的 `range_of`、`range_occurrence` 和手工 `source_boundaries` 维护；
5. 将 builder 输出的 `ColorSpan` 在 renderer 边界转换为 Vello 颜色类型，并为 `closing` 段落验证 italic、粗体与大字号同时生效。

除 `closing` 段落的有意样式修正外，迁移对照要求 source text、layout dump 和同一 renderer 重放后的 Vello scene 编码摘要相同。对照不要求 `TextSpan`、颜色或其他内部 range 数组具有相同分组；相邻 span 的等价合并可以不同。

完成条件：demo 不再查找子串或手工同步范围，builder 覆盖其全部现有输入需求；`closing` 段落按已确认的视觉修正单独断言。

### Phase 5：统一集成验收

1. 完成前四阶段遗留的实现和测试，使公开 API、测试与 demo 在同一代码状态下可编译、可运行；
2. 运行相关单元测试、集成测试、fixture golden 和 `cargo test --all-targets`；
3. 运行 paragraph demo，检查 shaping、布局和绘制路径；
4. 根据实际公开 API 更新 `README.md`、`docs/api-design-report.md`、`docs/key-differences.md` 和 `docs/tracking.md`；
5. 运行 `git diff --check`、编辑器诊断和文档风格检查，并审阅目标 diff。

完成条件：builder 可用于手工构造和 parser 驱动场景，demo 使用 builder，全部验证通过，当前维护文档与实现一致。

## 验证要求

测试至少覆盖以下行为：

- 文本追加后的 scalar offset 和范围；
- 闭包与显式写法生成相等的 `LayoutInput`；
- 多种作用域覆盖同一文本；
- 样式、ruby、decoration、inline box 与 source boundary 的生成；
- 空栈关闭、遗漏关闭和闭包内作用域遗留；
- hard break 与 inline object 的 source 表示；
- paragraph demo 的构造、布局与绘制路径；
- 迁移期间 builder 与旧手写构造的功能等价。

迁移期间的功能等价测试比较 source text、layout dump 和 Vello scene 编码摘要，不比较可等价重分组的内部 span 数组。`closing` 段落单独验证 source text 不变，以及其目标文本同时采用 italic、粗体与大字号。

任何改变断行、字体选择、标点空间、行高或行内几何的测试差异，都应先判断 builder 是否生成了与旧输入不同的数据。若输入相等而 layout 结果变化，应作为独立问题处理，不在 builder 中补偿。

## 风险与处理

| 风险 | 处理 |
| --- | --- |
| 闭包和显式 API 形成两套逻辑 | 两者共用内部开始与结束操作，并用结果等价测试固定行为。 |
| 闭包误关内部遗留的显式作用域 | 以私有序号和进入前栈深检查闭包边界，发生错误时拒绝构建。 |
| 样式嵌套改变现有覆盖顺序 | `closing` 段落有意修正为 italic、粗体与大字号同时生效；其他段落迁移期间比较旧构造与 builder 的布局和绘制功能，允许等价的 span 分组差异。 |
| 自动生成的边界不足或过多 | 按现有 pipeline 对各 span 的边界需求逐项核对，使用精确几何测试验证。 |
| 特殊内容被强行表示为普通文本作用域 | hard break 和 inline object 使用位置插入操作；ruby 注文继续与 source 分离。 |
| builder 扩展为新的内容模型 | 公开产物只承载既有 `LayoutInput` 与当前 renderer 所需的已有 core span；内部临时状态不作为可构造数据类型导出。 |

## 回滚

本迭代在现有 API 之上新增构造层。若实现无法保持与手写 `LayoutInput` 等价，可删除 `ParagraphBuilder`、相关测试和 demo 迁移，恢复 demo 的旧构造方式。回滚不需要改变 `LayoutInput`、layout pipeline、fixture 或用户数据。