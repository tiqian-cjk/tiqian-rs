# Text：Rust 核心的 Kotlin UTF-16 `String` 镜像

## 1. 本迭代的规则

`tiqian-rs` 是 Kotlin engine 的严格镜像。Kotlin 核心实现中类型为 `String` 的值，语义就是
UTF-16 code-unit string；Rust 标准库 `String` 的索引单位却是 UTF-8 byte。两者不能混用。

本迭代只做一件事：

> Rust 核心镜像中，凡是对应 Kotlin `String` 的字段、参数、返回值和局部值，统一改为
> `Text`。现有函数职责、调用关系、布局规则与阶段顺序不重组。

`Text` 是 Kotlin `String` 在 Rust 核心中的对应文本类型。它对算法提供 UTF-16
长度、code unit、code point、切片和 offset 转换；内部仍保留 UTF-8 文本供 Rust shaping/font
backend 借用。它不是新的 layout policy、rope、全局缓存或文本编辑器数据结构。

本文件是该迭代唯一的实施说明；实施时不依赖聊天记录或临时调查。

## 2. 不变的外部契约

以下内容不改名、不换单位、不改变行为：

| 契约 | 保持内容 |
| --- | --- |
| `core/Geometry.rs::TextRange` | `start`、`end` 仍是 UTF-16 code-unit `i32`。 |
| Kotlin engine / Compose | `String`、`TextRange`、selection、copy 与 accessibility 继续按 UTF-16。 |
| JavaScript / Web | JS `String.length`、slice 与 FFI source offset 继续按 UTF-16。 |
| Apple | `NSRange`、`UITextInput`、Swift `String.utf16` 的 source offset 继续按 UTF-16。 |
| Native ABI | `tiqian_layout_abi.h` 的 span、boundary、inline box、输出 range 全部保持 `i32` UTF-16。 |
| source text | 不改写输入文本；display substitution、cluster、debug 仍按现有规则生成。 |
| layout pipeline | `source → fallback → shaping → metrics → punctuation/glue → line break/repair → adjustment → LayoutResult → render` 的职责和顺序不变。 |

`Text` 只消除 Rust 端“UTF-16 offset 每访问一次就重新从 UTF-8 开头扫描”的实现差异。

## 3. 术语与坐标

同一段文本存在四种不同单位，实施中必须明确使用对应 API：

| 单位 | 例子 | 用途 |
| --- | --- | --- |
| UTF-8 byte | `"中".len() == 3` | Rust `&str` 借用、平台 shaping backend。 |
| UTF-16 code unit | `"🙂".encode_utf16().count() == 2` | `TextRange`、Kotlin mirror、ABI、selection。 |
| Unicode scalar | `中`、`🙂` 各一个 scalar | Unicode property、字体角色、emoji/脚本判定。 |
| source interaction unit | `👩🏽‍💻`、CRLF、Hangul 序列等 | 现有 `SourceInteractionBoundaries` 的规则结果。 |

UTF-16 code unit offset 可以落在代理对中间。例如 `"🙂"` 的 offset `1` 不是 Rust `&str`
的合法 byte 切点，但它仍是 Kotlin 可读取的一个低代理 code unit。`Text` 必须能读取它，
使 `codePointAtOrNull`、反向扫描、quote 分析等保持 Kotlin 行为。

当前 Rust pipeline 已要求所有需要借用 `&str` 的 source range 两端是 scalar boundary：各处
`TextIndex` helper 都以 `expect("... scalar boundary")` 执行此约束。本迭代保留该现有约束；
`Text::slice` 接替这些 helper 并保留相同错误消息语义。它不在本迭代中扩展 Rust
shaping backend 对半个代理项的表现。

## 4. 目标类型

### 4.1 位置与公开性

新建 `src/org/tiqian/core/Text.rs`，并在 `src/lib.rs` 的
`org::tiqian::core` 下导出 `pub mod Text;`。

类型对 crate 使用者公开，因为 `TiqianTextContent.text` 等既有公开字段会改为它；但其索引实现
字段保持私有。

### 4.2 数据表示

```text
Text
└── Arc<TextInner>
    ├── utf8: String
    └── index: OnceLock<Utf16Index>

Utf16Index
├── units: Vec<u16>
├── utf16_to_utf8: Vec<Option<usize>>
└── utf8_to_utf16: Vec<(usize, i32)>
```

`Text` 自身就是唯一的共享句柄：它内部持有 `Arc<TextInner>`，外部字段、参数、
collection 和返回值一律使用 `Text`，不得再包成 `Arc<Text>`。其 `Clone` 实现等价于
`Arc::clone(&self.inner)`。同一段文本从 `LayoutInput → WidthIndependentAnnotationKey →
WidthIndependentParagraphAnnotation → ParagraphLayoutPrep → LayoutResult` 复制时，所有副本指向
同一个不可变 UTF-8 文本和同一份 `OnceLock`。不建立以全文 hash 为 key 的全局缓存。

`Utf16Index` 首次调用任何 UTF-16 定位 API 时构建，构建一次后只读：

1. `units` 是 `utf8.encode_utf16().collect()`，因此 `units[index]` 与 Kotlin `text[index].code`
   对应。
2. `utf16_to_utf8` 长度为 `units.len() + 1`。每个 scalar 开始和文本结尾存对应 UTF-8 byte；
   supplementary scalar 的两个 UTF-16 unit 中间存 `None`。
3. `utf8_to_utf16` 按 UTF-8 byte 升序记录每个 scalar 开始和文本结尾的 UTF-16 offset，用于
   HarfBuzz/demo 回传的 byte cluster offset。

索引构建复杂度为 $O(n)$；之后 UTF-16→byte、byte→UTF-16、UTF-16 code-unit 随机访问都是
$O(1)$ 或对有序边界表的 $O(\log n)$ 查找。不得再用 `encode_utf16().nth(...)`、
`encode_utf16().count()` 或从文本开头扫描的 `TextIndex` helper。

### 4.3 trait 与构造约定

`Text` 必须实现或显式提供：

| 项目 | 要求 |
| --- | --- |
| `Clone` | 克隆内部 `Arc<TextInner>`，不复制 UTF-8 文本或已建索引。 |
| `Debug` | 输出 source text，便于既有 `Debug` 派生结构保持可读。 |
| `PartialEq` / `Eq` | 只比较 UTF-8 source 内容，绝不比较 `Arc` 地址或 `OnceLock` 状态。 |
| `Hash` | 只 hash UTF-8 source 内容，使 `WidthIndependentAnnotationKey` 继续按文本内容命中。 |
| `Default` | 空文本。 |
| `From<String>`、`From<&str>` | I/O 边界和现有构造器可直接转换。 |
| `Display`、`AsRef<str>` | 保持 dump、格式化和平台 API 的非索引文本使用。 |
| `Deref<Target = str>` | 仅保留 `chars`、`contains`、`starts_with`、`split` 等不以 index 为语义的既有 Rust 文本操作；禁止利用它做 byte index 或 `.len()` 作为 source length。 |

`TiqianTextContent::new` 与 `builder` 改为接收 `impl Into<Text>`，因此 fixture、demo、
测试和 FFI 仍可传入 `String`，无需在 I/O 层提前建索引。

### 4.4 固定 API

以下 API 名称、返回值和边界行为是本迭代实现契约；除非 Kotlin 对应代码改变，不在实施中另起
helper 类型或把判断转移到调用方。

| API | 行为与 Kotlin 对照 |
| --- | --- |
| `as_str(&self) -> &str` | 原始有效 UTF-8，供平台/font/shaping、JSON、format、非索引字符串算法使用。 |
| `utf16_len(&self) -> i32` | Kotlin `String.length`。空文本为 `0`。 |
| `is_empty(&self) -> bool` | Kotlin `isEmpty()`。 |
| `utf16_code_unit_at(&self, offset: i32) -> i32` | Kotlin `text[offset].code`；负数或 `offset >= utf16_len()` 直接 panic。代理对中间返回低代理值。 |
| `utf16_code_unit_at_or_none(&self, offset: i32) -> Option<i32>` | Kotlin `getOrNull(offset)?.code`；仅越界返回 `None`。 |
| `code_point_at_compat(&self, offset: i32, end: i32) -> i32` | 复用 Kotlin 各文件的 high/low-surrogate 拼合规则；`offset` 不合法直接 panic，`end` 限制能否读取低代理。 |
| `code_point_at_or_none(&self, offset: i32) -> Option<i32>` | Kotlin `codePointAtOrNull`；越界返回 `None`，在低代理位置返回该 code unit。 |
| `code_point_before(&self, offset: i32) -> Option<i32>` | Kotlin `codePointBefore` / `previousCodePointBefore`；从前一个 code unit 反向识别代理对。 |
| `utf8_byte_index_at(&self, utf16_offset: i32) -> Option<usize>` | 只对 scalar boundary 返回 byte；代理对中间和越界返回 `None`。替代旧 `utf16_offset_to_utf8_byte_index`。 |
| `utf16_offset_at(&self, byte_offset: usize) -> Option<i32>` | 只对 UTF-8 scalar boundary 返回 UTF-16 offset；替代旧反向 helper 与 demo 的线性 scan。 |
| `slice(&self, range: TextRange) -> &str` | UTF-16 source range 的零拷贝 slice；两端必须是 scalar boundary，否则 panic。替代所有 `source_slice`。 |
| `slice_offsets(&self, start: i32, end: i32) -> &str` | `slice(TextRange::new(start, end))` 的直接形式，供现有 start/end 局部变量调用点使用。 |

`char_count_compat(code_point)` 继续是现有算法的局部纯计算：scalar 大于 `0xFFFF` 为 `2`，否则为
`1`。它不属于索引缓存，不应被改造成 layout policy。

## 5. 机械迁移范围

### 5.1 判定方法

范围不是按“段落正文”或“是否热路径”筛选。判定只有一条：Rust 镜像文件中对应 Kotlin
`String` 的位置，改为 `Text`。这包括 source text、display text、font 名、locale、
debug reason、ruby text 和所有 Kotlin `String` 的 collection element；即使当前调用点没有 UTF-16
索引，也不能在 Rust 核心保留一个语义不同的 `String`。

`std::string::String` 只允许留在核心之外的输入/输出边界，或作为 `Text::as_str()` 调用的
第三方 API 临时返回值：

- `examples/fixture-layout-dump.rs` 的 `serde` wire structs；
- `examples/paragraph_demo/*` 的 CLI/UI/第三方 shaping glue；
- `tests/**` 中构造输入和断言字面量；
- `serde_json`、font library、系统 API 所要求的 `String`。

这些边界在进入 core 时转换为 `Text`，离开 core 时用 `as_str()` 或显式
`to_owned()` 转换。不得让外部 `String` 再进入 core helper。

### 5.2 已调查的直接 UTF-16 访问点

下表列出必须删除的现有重复索引实现。每一行保留原函数职责和调用位置，只替换文本参数类型与
具体读取表达式。

| Rust 文件 | 当前行为 | 迁移后的直接调用 |
| --- | --- | --- |
| `core/TextIndex.rs` | 两个从头线性扫描的转换函数 | 删除模块与 `lib.rs` 导出；功能由 `Text::utf8_byte_index_at`、`utf16_offset_at` 承担。 |
| `core/SourceInteractionBoundaries.rs` | `utf16_length`、`utf16_code_unit_at`、surrogate 拼合 | 参数改 `&Text`；调用 `utf16_len`、`code_point_at_compat`、`utf16_code_unit_at`。emoji/CRLF/Hangul 规则原样保留。 |
| `core/EastAsianSpacing.rs` | `encode_utf16`、`nth`、boundary 到 slice | `utf16_len`、`code_point_at_compat`、`slice_offsets`；UTR #59 规则不变。 |
| `core/LayoutQueries.rs` | copy slice、selection word、range clamp 的长度重复计算 | 读 `result.input.content.text` 的 `utf16_len`；copy 用 `slice_offsets`；interaction helper 接收 `&Text`。 |
| `font/FontPolicy.rs` | 每次 `code_point_at_compat`/previous 都新建 `Vec<u16>` | trait 的 `text` 改 `&Text`；调用类型方法，字体角色判断不变。 |
| `layout/ClusterRoleResolution.rs` | `utf16_length`、`nth`、`source_slice` | `utf16_len`、`code_point_at_compat`、`utf16_code_unit_at`、`slice`。cluster 聚合和 emoji promotion 原样保留。 |
| `layout/ContextualQuoteRoleResolver.rs` | quote 对内/外层正反扫描、低代理检测 | struct 字段改 `&Text`；所有 Kotlin code-unit 行为由类型方法提供。 |
| `layout/QuotePairAnalyzer.rs` | quote stack 与 apostrophe 的 code point before/at | 参数改 `&Text`；保留现有配对和 context resolver 调用。 |
| `layout/ParagraphShapingStage.rs` | source slice、code point at/before、UTF-16 offsets | 参数改 `&Text`；`slice`、`code_point_at_or_none`、`code_point_before`；hyphenation/token 规则不变。 |
| `layout/UnicodePunctuationBoundaryResolver.rs` | cluster slice、前后 scalar、authored-boundary reverse scan | 参数改 `&Text`；`cluster_text` 用 `slice`；现有 `Option` 分支保留。 |
| `layout/WidthIndependentAnnotationCache.rs` | `text` clone、长度、debug source slice | annotation key、annotation、prep 的 `text` 改为 `Text`；切片改 `slice`。 |
| `layout/LineBreakPlanningStage.rs` | prep source slice | `ParagraphLayoutPrep.text` 改为 `Text`；metric/debug source 使用 `slice`。 |
| `layout/LayoutDebugAssembly.rs` | decision source slice | `LayoutDebugStageInput.text` 改为 `&Text`；`source_slice` 删除，直接 `slice`。 |
| `shaping/TextShaper.rs` | `ShapingInput` source slice | `ShapingInput.text` 改 `Text`；`display_text` 也改 `Text`；stub 在需要 `&str` 时用 `as_str()`。 |
| `examples/paragraph_demo/font_backend.rs` | source/display slice、HarfBuzz byte→UTF-16 | core 输入改 `&Text`；source/display 调 `slice`，byte cluster 调 `utf16_offset_at`。 |
| `tests/org/tiqian/layout/UnicodePunctuationBoundaryTest.rs` | line range 转 slice | 测试局部 `String` 先 `Text::from`，再用 `slice`；测试断言含义不变。 |

### 5.3 全部 Kotlin `String` 镜像位置

下表是逐文件机械替换清单。实施时先将列出的 `String`、`&str`、`Vec<String>`、
`Option<String>`、`HashMap<..., String>`、返回 `String` 改为相应 `Text` 形式，再按
编译器报出的第三方边界显式使用 `as_str()` 或 `Text::from(...)`。不要为了减少报错保留
core 中的 `String`。

| 模块 | 文件 | Kotlin `String` 对应值 |
| --- | --- | --- |
| core | `TextModel.rs` | `TiqianTextContent.text`、`TextStyle.font_families`、`TextStyle.locale`、`RubySpan.text`、`RubySpan.font_families`、`RubySpan.locale`、`LayoutProfileId.value`、rich-text link target 等。 |
| core | `LayoutModel.rs` | `Cluster.text`、`Cluster.display_text`、所有 `source_text`/`display_text`、font key、reason、notes、role 名和 structured debug string 字段。 |
| core | `LayoutQueries.rs` | copy projection、rich-text/debug 的 Kotlin string 局部值和参数。 |
| core | `EastAsianSpacing.rs`、`SourceInteractionBoundaries.rs`、`UnicodeScriptEvidence.rs`、`UnicodeWordCharacter.rs` | 文本参数、locale 与数据描述 string。 |
| clreq | `BopomofoReading.rs`、`ClreqProfile.rs`、`NumberSymbolCohesion.rs` | reading/text 参数、glyph substitution 的 `source_text`/`display_text`、policy reason 与名称。 |
| font | `FontMetrics.rs`、`FontPolicy.rs` | face selection text、字体 candidate key/family、request locale、font role name 与所有文本参数。 |
| linebreak | `EnglishHyphenation.rs`、`Hyphenation.rs`、`LineBreak.rs`、`UnicodePunctuationLineBreak.rs` | word/text 参数、hyphenation table 结果、break reason。 |
| shaping | `TextShaper.rs`、`ReplayableFontBackend.rs` | shaping input text/display text、font key、capability issue、backend selection text。 |
| layout | `AnnotationGeometryStage.rs`、`ClusterRoleResolution.rs`、`ContextualQuoteRoleResolver.rs`、`LayoutDebugAssembly.rs` | source/display/debug text、reason、quote context 与所有 text 参数。 |
| layout | `LineAdjustmentStage.rs`、`LineBreakPlanningStage.rs`、`LineGeometryStage.rs`、`LineOptimization.rs`、`LineRepair.rs` | prep text、metric source text、repair/decision reason。 |
| layout | `ParagraphLayoutEngine.rs`、`ParagraphShapingStage.rs`、`PreparedParagraph.rs`、`ProgressiveBreakDecisions.rs` | input source text、segment/token text、numeric formatting intermediate、break reason。 |
| layout | `PunctuationGeometryLedger.rs`、`PunctuationGeometryStage.rs`、`PunctuationModel.rs`、`UnicodePunctuationBoundaryResolver.rs` | cluster text/display text、punctuation text、side/role/reason、source text 参数。 |
| layout | `QuotePairAnalyzer.rs`、`WidthIndependentAnnotationCache.rs` | quote source/reason、cache key text、annotation text、rollback reason、prep text。 |

Rust string literals remain literals. A literal passed to a now-`Text` field/parameter uses
`Text::from("...")`; a `format!`/`to_owned` result uses `.into()`. A temporary passed to a
third-party Rust API stays a `String` only at that call boundary.

## 6. 实施 phase

phase 按依赖方向安排，而不是按“每改一点就恢复编译”安排。Phase 1 到 Phase 4 可以处于无法编译、
无法运行和无法测试的状态；不为中间状态添加 compatibility wrapper、临时 `String` overload 或
过渡 `TextIndex`。只有 Phase 5 要求 crate、既有测试与 fixture 恢复可用。

每个 phase 开始前只阅读该 phase 涉及文件的当前内容。若发现 Kotlin 源与本文件的范围清单不一致，
或第三方 API 不能接受 `Text` 且无法明确归入 I/O 边界，停止修改：说明差异、给出可选处理
方式并询问用户；不得自行扩大或缩小第 5 节的规则。

### Phase 1：建立基础类型，移除旧索引入口

**目标**：提供唯一的 Kotlin UTF-16 string mirror，使后续所有文件都有确定的替换目标。

1. 新增 `core/Text.rs`，实现第 4 节的数据表示、内部 `Arc<TextInner>`、
   `OnceLock<Utf16Index>`、traits 和固定 API。
2. 在 `lib.rs` 导出 `core::Text`。
3. 删除 `core/TextIndex.rs` 及其 module export；不保留转发函数。
4. 在新类型内保留现有 source-slice 错误的表达能力：调用方仍可为不合法 scalar boundary 使用
   `expect("... scalar boundary")`。

**允许的中间状态**：所有现有 `TextIndex` import 和 `String` 模型字段均可报错。此 phase 不修
调用方，不运行测试。

### Phase 2：替换核心数据模型与跨模块签名

**目标**：先把 Kotlin `String → Text` 的类型边界推到核心模型、trait 和 stage input/output；
这是整个迭代唯一的数据类型，不在各模块自行定义文本包装。

1. 按第 5.3 表替换 `core/TextModel.rs`、`core/LayoutModel.rs`、`clreq`、`font`、`linebreak`、
   `shaping`、`layout` 中的 struct field、enum payload、collection element、trait parameter、trait
   return value 和 builder field。
2. 优先完成 `LayoutInput`、`LayoutResult`、`ShapingInput`、font/line-break trait、
   `WidthIndependentAnnotationKey`、`WidthIndependentParagraphAnnotation`、
   `ParagraphLayoutPrep` 的文本字段，使同一段 source text 的 `Clone` 共享内部 `Arc`。
3. 文字常量、`format!`、`to_owned()` 的结果按第 5.3 节转换为 `Text`；暂不解决第三方
   函数的 `&str` 参数错误。

**允许的中间状态**：所有调用点、方法体、`String` API 使用和 trait implementation 可以报错。
本 phase 不添加 `String` 版本的重载，也不运行编译或测试。

### Phase 3：迁移 UTF-16 文本算法

**目标**：消除每个算法文件中重复的 UTF-16 编码和 byte offset 扫描，同时保持 Kotlin 原函数的
控制流和排版决策。

1. 按第 5.2 表逐个改造 `SourceInteractionBoundaries`、`EastAsianSpacing`、`FontPolicy`、
   `ClusterRoleResolution`、`ContextualQuoteRoleResolver`、`QuotePairAnalyzer`、
   `ParagraphShapingStage`、`UnicodePunctuationBoundaryResolver`、`LayoutQueries`。
2. 以 `utf16_len`、`utf16_code_unit_at`、`code_point_at_compat`、`code_point_at_or_none`、
   `code_point_before`、`slice`、`slice_offsets`、`utf16_offset_at` 直接替换局部 helper；删除被替代的
   局部 `utf16_length`、`utf16_code_unit_at`、`code_point_at_compat`、`source_slice`。
3. `Vec<u16>` 仅在它是为了 source code-unit 随机访问而存在时删除；`NumberSymbolCohesion` 和
   `LineBreak` 中若仍以 Kotlin 的完整 UTF-16 code-unit 序列为算法输入，则改为从
   `Text` 的内部索引读取或由类型提供只读单位视图，不能把算法改成 UTF-8 byte 语义。

**允许的中间状态**：layout pipeline、cache、demo 和 fixture 仍可能因参数类型不匹配而报错。
本 phase 不重写任何 emoji、quote、line-break、font 或 punctuation 规则。

### Phase 4：贯通 pipeline 与外部边界

**目标**：将 `Text` 贯通从输入、缓存、layout、查询到平台调用，完成所有 Kotlin string
mirror 的机械替换。

1. 修改 `WidthIndependentAnnotationCache`、`LineBreakPlanningStage`、`LayoutDebugAssembly`、
   `ParagraphLayoutEngine`、`LineAdjustmentStage`、`AnnotationGeometryStage` 和其相邻调用点，
   使 cache、annotation、prep、result 传递 `Text` 而非重新构造文本。
2. 完成第 5.3 表中尚未迁移的 `clreq`、`font`、`linebreak`、`shaping`、`layout` 文件；`String`
   只在 JSON、serde、font/system/HarfBuzz 等 crate 外 API 的调用点短暂出现。
3. 修改 `examples/fixture-layout-dump.rs`、`examples/paragraph_demo/font_backend.rs` 与现有 tests：
   输入 wire/CLI/测试字面量转换为 `Text`，第三方 API 参数使用 `as_str()` 或明确的
   `.to_owned()`。
4. 检查 `Text` clone 路径，确保 core 内没有 `Arc<Text>`，也没有以内容 hash 为
   key 的全局索引缓存。

**允许的中间状态**：直到本 phase 末尾，crate 仍可有少量编译错误；只修复实现此 phase 所需的
类型和边界错误，不执行测试来追逐中间绿灯。

### Phase 5：收口、验证与人工性能对比

**目标**：这是唯一要求可编译、可运行、可测试的 phase。

1. 在 `src/org/tiqian/**` 逐项搜索并处理残留：`TextIndex::`、
   `utf16_offset_to_utf8_byte_index`、`utf8_byte_index_to_utf16_offset`、
   `encode_utf16().nth`、`encode_utf16().count`、仅为随机访问创建的 `Vec<u16>`、
   `text: String`、`text: &str`。每个命中必须归类为遗漏、Rust/第三方 I/O 边界、string literal
   或与 UTF-16 无关的 byte API；无法归类时停止并询问用户。
2. 统一修复最终编译错误；不以重新引入 `TextIndex`、临时 adapter 或 core 内 `String` 字段为解法。
3. 运行第 8 节的既有验证命令，处理本迭代引入的失败。不得新增专门测试或 benchmark。
4. 用户用同一机器、构建模式、字体、文本和约束完成第 8 节规定的 demo 前后性能对比，记录首次
   layout 与复用同一 `Text` 后的 layout 数据。

## 7. 不得改变的局部逻辑

实施时以下模式只能替换文本访问表达式，不得“顺手优化”或合并：

| 位置 | 必须保留的行为 |
| --- | --- |
| `SourceInteractionBoundaries` | CRLF、emoji modifier、ZWJ、regional indicator、Hangul、variation selector、emoji tag 的边界规则。 |
| `ClusterRoleResolution` | mandatory break、zero-width break、Latin/coalesced punctuation、emoji promotion、span boundary 的优先级。 |
| `ContextualQuoteRoleResolver` / `QuotePairAnalyzer` | quote stack、嵌套对跳过、apostrophe、段落语言 fallback 的顺序。 |
| `FontPolicy` | CJK/Latin/symbol/emoji 角色判定与 fallback 选择。 |
| `ParagraphShapingStage` | segment 分割、hyphenation、progressive technical tier、emergency tracking、substitution rollback。 |
| `UnicodePunctuationBoundaryResolver` | UAX #14 punctuation、quote direction、authored-break 检查。 |
| `WidthIndependentAnnotationCache` | LRU key 的值相等语义、cache eviction、宽度无关 annotation 的内容。 |
| `LayoutQueries` | selection/caret 的 UTF-16 range、interaction boundary coercion、copy ruby 插入规则。 |

特别地，`Text` 不吸收 `UnicodeEmojiStyleVariationData`、`EastAsianSpacingData`、ICU4X
property 查询、grapheme policy 或 font/shaping 规则；这些仍由现有文件拥有。

## 8. 编译与行为检查

用户不要求为本迭代新增测试或 benchmark。不得添加只测试 wrapper getter 的新测试，也不得加入
自动 benchmark。完成实现后只运行已有验证：

1. `cargo test`
2. `bash tools/verify-all-fixtures.sh`
3. `git diff --check`

编译错误处理顺序：先修核心模型类型传播，再修 trait implementation，再修第三方 API 边界；
不以保留 `String` 或重新引入 `TextIndex` 作为临时兼容层。

性能由人工比较 paragraph demo 的前后 layout 数据。比较必须使用同一机器、构建模式、字体、
文本与约束，至少记录：普通中文正文、emoji/ZWJ 密集文本、组合标记文本、长 Latin token。记录
首次 layout（包含 `OnceLock` 建表）和后续 layout（复用同一 `Text`）的结果；不预设提升
幅度。

## 9. 完成判据

本迭代完成时必须同时满足：

- `src/org/tiqian/**` 中所有作为 layout/shaping 文本并需要 UTF-16 source-coordinate 语义的 Kotlin
   `String` mirror 已为 `Text`；第 10 节列出的高置信非文本字符串保持 Rust `String`；
- `TiqianTextContent.text`、cache key、annotation、prep、`LayoutResult.input` 的同一 source text
  clone 共享一个 `Arc<TextInner>`；
- 所有 UTF-16 长度、code unit、code point、source range slice 和 byte offset 转换经由
  `Text`；
- `TextIndex.rs`、重复 `encode_utf16().nth(...)`、仅为随机访问创建的 `Vec<u16>` 均已移除；
- `TextRange`、fixture JSON、FFI ABI、Kotlin/JS/Compose/Apple UTF-16 契约没有改动；
- 现有 `cargo test`、fixture golden、`git diff --check` 均通过；
- 用户已完成同环境 demo layout 性能对比。


## 10. 收敛回退

当前以完成上述 Phase 1-5，但经调查发现，部分 `Text` 的使用场景并不需要 UTF-16 索引缓存。

应把 `Text` 收敛为：

> **会作为 layout/shaping 文本使用，且需要 UTF-16 code-unit、`TextRange`、切片或 code point 操作的值。**

当前 `Text.rs` 的额外价值正是 UTF-16 索引缓存与 UTF-16/UTF-8 边界换算。  
**“会影响排版”不等于“是被排版文本”。** 例如 locale、字体 family、feature 和 profile 会影响结果，但无需 UTF-16 文本容器。

### 可回归 `String`：高置信

| 类别 | 典型字段 / 位置 | 为什么不需要 `Text` | 建议类型 |
|---|---|---|---|
| 调试原因与规则代码 | `LayoutModel.rs` 中绝大多数 `reason`、`reason_code`、`repair`、`notes`、`tier`、`kind`、`source`、`mode`、`side` | 固定标签或 `format!("{:?}", enum)` 结果；只写入 dump、比较或筛选 | `String` / `Option<String>` / `Vec<String>` |
| 排版决策 evidence | `forbidden_position`、`boundary_role`、`geometry_source`、`glyph_placement_reason`、`ink_bounds_fallback`、`halt_validation` | 是策略、回退或诊断标签，不是 glyph source | `String` |
| 断行、修复与调整标签 | `Justifier.rs`、`LineOptimization.rs`、`LineBreak.rs` | DP/repair 的解释信息；不做 UTF-16 操作 | `String` |
| 字体 key 与 family | `Cluster.font_key`、`GlyphRun.font_key`、`Glyph.render_font_key`、`FontCandidate.key/family`、`FontMetricsRequest.font_key/font_families` | 字体 catalog / renderer replay 的身份，不是输入文本 | `String` / `Vec<String>` |
| OpenType feature | `GlyphRun.open_type_features`、`ShapingInput.open_type_features`、`open_type_features_by_cluster_range` | 如 `halt=1`、`vert=1`；是 shaping 配置，不是 shape 的内容 | `Vec<String>` |
| 字体 backend 元数据 | `ReplayableFontBackend.rs` 的 `FontFaceId`、family aliases、axis key、source label、capability code/detail、backend/source kind | SFNT identity、font alias、axis tag、能力报告 | `String`、`HashSet<String>`、`HashMap<String, f32>` |
| Locale / region / profile | `TextStyle.locale`、`RubySpan.locale`、`FontRequest.locale`、`FontRoleContext.region_hint`、`LayoutProfileId.value`、`ClreqProfile.id` | BCP-47、region 和 profile resolver key；`EastAsianSpacing` 仅 `split` / lowercase locale | `String` / `Option<String>` |
| Link target | `RichTextRole::Link { target }` | 导航/可访问性目标，不属于 source range | `String` |
| 连字符资源与模式 | `Hyphenation.rs` 的 `patterns`、`exceptions`、TeX parser 输入 | 资源解析与 map key；不持有 `TextRange` | `HashMap<String, Vec<i32>>`、`&str` |
| substitution rollback 缓存 value | `substitution_rollbacks: HashMap<TextRange, Text>` | `TextRange` 是 key；value 是 rollback 原因码 | `HashMap<TextRange, String>` |

#### 最有代表性的错误迁移模式

以下构造点几乎可以视为“应为 `String`”的强信号：

- `Text::from("Explicit")`
- `Text::from("ForbiddenAtLineStart")`
- `Text::from(format!("{:?}", role))`
- `Text::from(format!("...:{cause}"))`
- `Text::from("halt=1")`
- `Text::from("zh-Hans")`
- `Text::from("clreq-horizontal")`
- `Text::from("cjk-primary")`

这些在当前代码中主要出现在：

- `LayoutDebugAssembly.rs`
- `AnnotationGeometryStage.rs`
- `LineRepair.rs`
- `PunctuationGeometryStage.rs`
- `FontPolicy.rs`

### 必须保留 `Text`

| 类别 | 字段 / 位置 | 原因 |
|---|---|---|
| 段落 source | `TiqianTextContent.text` | 所有 `TextRange` 都以该文本的 UTF-16 offset 为坐标。 |
| 后续 pipeline 的段落缓存 | `WidthIndependentAnnotationKey.text`、`WidthIndependentParagraphAnnotation.text`、`ParagraphLayoutPrep.text` | 会再次 cluster、断行、切片、shape。 |
| shaping 输入 | `ShapingInput.text`、`ShapingInput.display_text` | `range` 指向前者；后者直接交给 shaper。 |
| 已布局 cluster | `Cluster.text`、`Cluster.display_text` | source/display 文本、替换、renderer replay 和 source mapping 的核心数据。 |
| 注文与注音文本 | `RubySpan.text`、`RubyDecisionInfo.text`、`BopomofoDecisionInfo.text`、`BopomofoGlyphPlacement.text`、`BopomofoReading.symbols` | 虽然不是正文 source，但会被实际 shaping / 绘制。 |
| 标点替换文本 | `CjkPunctuationGlyphSubstitution.source_text/display_text` | 需要 UTF-16 code unit 检查，替换后的 display text 会进入 shaping。 |
| Hyphenator 的公共 `word` 参数 | `Hyphenator::hyphenate(&Text)` | 调用端将结果映射回 source；目前以 UTF-16 长度约束边界。 |

### 边界项：先不动更稳妥

| 字段 | 当前情况 | 建议 |
|---|---|---|
| 各 `*DecisionInfo.source_text` / `display_text` | 大多只用于 debug dump，但它们确实是从被排版 source/display 切出的快照 | **先保留**；若以后希望只让真正参与 pipeline 的文本使用 `Text`，可统一降为 `String`，并注明不得配合原 `TextRange` 再切片。 |
| `FontMetricsRequest.face_selection_text` | 不测 glyph bounds，但来自具体 run，用来选覆盖该 run 的实际 face | **先保留**，与下项统一决策。 |
| `ReplayableFontFaceRequest.selection_text` | 不输出到 `LayoutResult`，但用于判断 face 是否覆盖具体 run | **先保留**；它是 font coverage 的文本证据。 |
| debug struct 内 `font_key` | 该字段本身高置信为普通 `String`，但所在 struct 可能同时包含 source/display 快照 | **按字段拆分**，不要因 struct 混有文本而整体保留。 |

### Phase 6：高置信非文本字符串回退

**目标**：只将本节「可回归 `String`：高置信」表明确列出的值恢复为迁移前的 Rust `String` 家族。
不触及「必须保留 `Text`」和「边界项：先不动更稳妥」中的任何字段；不重组函数、规则、
调用关系或数据模型。

本 phase 覆盖并取代第 1 节和第 5 节中「所有 Kotlin `String` 一律使用 `Text`」的表述。
后续实施以本节的文本语义判定为准：Kotlin 的 `String` 是源语言表示，是否需要 Rust 的 UTF-16
索引缓存由该值是否作为 layout/shaping 文本决定。

每个子 phase 开始前，必须阅读所涉及 Rust 文件的当前内容，并查看该文件相对本迭代开始前的 diff。
对高置信表内字段，优先**直接恢复迁移前的 `String`、`Option<String>`、`Vec<String>`、
`HashMap<..., String>`、`HashSet<String>` 或 `&str` 类型及其原有调用形状**；不得为回退创建新类型、
adapter、兼容 overload 或 helper。子 phase 之间不要求 crate 可编译，也不运行测试来追逐中间绿灯。

若出现以下任一情况，立即停止当前子 phase，说明字段、当前/迁移前类型和调用用途，并询问用户决策：

- 待改字段不在本节「可回归 `String`：高置信」表中；
- 无法从当前代码和本迭代 diff 确认迁移前类型；
- 恢复原类型需要改变 layout/shaping 文本字段、UTF-16 API 或 `TextRange` 语义；
- 第三方 API、公开 trait 或跨 crate 合同的边界无法明确归入既有 `String` 输入/输出路径。

#### Phase 6.1：决策与调试标签

1. 在 `core/LayoutModel.rs` 恢复高置信的 reason、reason code、repair、notes、tier、kind、source、
   mode、side、role 名、枚举序列化结果和 punctuation evidence 字段及其 builder 参数的原类型。
2. 在 `layout/Justifier.rs`、`LineOptimization.rs`、`LineRepair.rs`、`LineBreaker.rs`、
   `LineAdjustmentStage.rs`、`LineGeometryStage.rs`、`LineBreakPlanningStage.rs`、
   `AnnotationGeometryStage.rs`、`LayoutDebugAssembly.rs`、`PunctuationGeometryLedger.rs`、
   `PunctuationGeometryStage.rs`、`PunctuationModel.rs`、`UnicodePunctuationBoundaryResolver.rs`、
   `QuotePairAnalyzer.rs` 和 `linebreak/LineBreak.rs` 恢复这些字段的构造值与局部值。
3. 仅移除为这些标签添加的 `Text::from(...)`；正文/annotation 文本仍保留原样。

**允许的中间状态**：`LayoutModel` 的 builder、debug assembly、fixture dump、测试和 stage 调用点可因
`String`/`Text` 不匹配而报错。

#### Phase 6.2：字体身份、feature 与 capability 元数据

1. 在 `core/LayoutModel.rs` 恢复 `Cluster.font_key`、`GlyphRun.font_key`、
   `Glyph.render_font_key`、`GlyphRun.open_type_features` 及所有对应 builder/构造参数的原类型。
2. 在 `font/FontPolicy.rs`、`font/FontMetrics.rs`、`shaping/TextShaper.rs`、
   `shaping/ReplayableFontBackend.rs`、`layout/LineBreakPlanningStage.rs` 恢复高置信表中的 font key、
   family、OpenType feature、face ID、alias、variation axis、source label、capability report 和
   `LayoutFontMetrics.reason`。
3. 只恢复身份/配置/证据值；`ShapingInput.text`、`ShapingInput.display_text`、
   `FontMetricsRequest.face_selection_text` 和 `ReplayableFontFaceRequest.selection_text` 不改。

**允许的中间状态**：fallback/metrics/shaper trait implementation、renderer replay 与 demo backend
可暂时因签名不一致而报错。

#### Phase 6.3：locale、profile、link 与 hyphenation 资源

1. 在 `core/TextModel.rs`、`clreq/ClreqProfile.rs`、`font/FontPolicy.rs`、`font/FontMetrics.rs`、
   `shaping/ReplayableFontBackend.rs` 及其直接 stage 调用点，恢复高置信表中的 locale、region hint、
   profile ID 和 font family 的原类型。
2. 恢复 `RichTextRole::Link.target` 及 `link_address_display` 的 target 参数为既有普通字符串路径；
   display 仍是被排版 source text，不得降级。
3. 在 `linebreak/Hyphenation.rs`、`EnglishHyphenation.rs` 恢复 TeX parser 输入、pattern 和 exception
   map 的原类型；`Hyphenator::hyphenate` 的 `word` 参数保持 `&Text`。

**允许的中间状态**：locale 使用点、profile resolver、fixture wire conversion 与 hyphenator 构造器
可因 collection element 类型变化而报错。

#### Phase 6.4：缓存原因值、收口与验证

1. 在 `ParagraphShapingStage.rs`、`WidthIndependentAnnotationCache.rs`、
   `LineBreakPlanningStage.rs`、`LayoutDebugAssembly.rs` 恢复 `substitution_rollbacks` 的 value 为
   `String`；`TextRange` key 和关联的 paragraph source text 保持不变。
2. 只搜索并处理本 phase 三个子 phase 已列类别中残留的 `Text::from(...)`、collection element
   和 builder/trait 类型不匹配；不要以全仓库搜索结果扩展回退范围。
3. 此时恢复最终编译；只修复由 Phase 6 已列字段引入的错误。运行第 8 节已有验证：`cargo test`、
   `bash tools/verify-all-fixtures.sh`、`git diff --check`。
4. 完成后检查：所有「可回归 `String`：高置信」字段已恢复；所有「必须保留」和「边界项」字段仍为
   `Text`；没有新增 wrapper、adapter、overload 或无关重构。

**完成判据**：Phase 6 的最终工作树可编译，既有测试和 fixture golden 通过，且只发生本节高置信表
明确允许的类型回退。
