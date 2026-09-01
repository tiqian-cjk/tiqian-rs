# ICU4X Unicode 数据替换迭代

## 目标

将 `tiqian-rs` 中**自行维护的标准 Unicode 属性表**和以编码范围表达的**标准 Unicode 属性判断**迁移到 ICU4X `icu_properties` 2.3.x。依赖采用与该 release 绑定的编译期 Unicode 17 属性数据；不使用运行时下载或宿主 Unicode 表。

本迭代只替换 Unicode 数据来源，不改变 Tiqian 的布局算法、字体角色策略、断行策略、UTF-16 ABI、source range 或 renderer 边界。原有的局部映射和具名 policy 必须保留，不能把 ICU4X 原始属性值直接当成 Tiqian 的排版决策。

## 不替换的两项

以下两项继续由本仓库维护，是本迭代唯一保留的本地 Unicode 数据集：

1. `src/org/tiqian/font/UnicodeEmojiStyleVariationData.rs` 的 **Emoji style variation base 集合**。
   它来自 emoji variation sequences 的 base 标量集合，不等同于 `Emoji`、`EmojiPresentation` 或 ICU4X 的通用 emoji sequence set；继续用于精确判定 Tiqian 的 emoji-style variation promotion。
2. `src/org/tiqian/core/EastAsianSpacingData.rs`。
   它是 UTR #59 `East_Asian_Spacing` 草案数据，不等同于 `EastAsianWidth`，不得以 ICU4X `EastAsianWidth` 替换。

这两个集合仍应保留其数据版本、来源与测试说明。

## 依赖与访问约定

1. 在 `Cargo.toml` 增加 `icu_properties` 2.3.x，并选择编译期数据配置，使所用属性固定为 Unicode 17。
2. 所有 Unicode 标量查询经一个窄的 crate-internal 访问约定进行：
    - 二元属性对现有 Rust `char` 使用 `CodePointSetData::<props::Property>::new().contains(char)`，
       对 UTF-16 code point 使用 `contains32(u32)`；
    - 枚举属性对现有 Rust `char` 使用 `CodePointMapData::<props::Property>::new().get(char)`，
       对 UTF-16 code point 使用 `get32(u32)`。
3. 调用方仍先处理 UTF-16 surrogate 和无效标量；只有可转换为 Rust `char` 的 Unicode scalar 才交给 ICU4X。
4. 迁移完成后移除 `unicode-general-category` 与 `unicode-properties`，不得留下版本不一致的第二套 Unicode 属性源。
5. 不引入 Unicode 数据生成脚本、下载步骤或新的 runtime provider。ICU4X release 升级时，Unicode 版本随依赖升级，在该次升级中重新跑本文件规定的验证。

## 替换清单

### 生成表与直接消费者

| 现有文件 | ICU4X 属性 | 保留的本地语义 | 完成后的处理 |
| --- | --- | --- | --- |
| `core/UnicodeEmojiModifierBaseData.rs` | `props::EmojiModifierBase` | 无 | 删除表、模块导出和直接调用，改查属性。 |
| `core/UnicodeExtendedPictographicData.rs` | `props::ExtendedPictographic` | ZWJ 序列组合规则 | 删除表、模块导出和直接调用，改查属性。 |
| `font/UnicodeEmojiData.rs` | `props::Emoji` | Emoji sequence promotion 的后续条件 | 删除表、模块导出和直接调用，改查属性。 |
| `font/UnicodeEmojiPresentationData.rs` | `props::EmojiPresentation` | `FontRole::Emoji` 的角色策略 | 删除表、模块导出和直接调用，改查属性。 |
| `core/UnicodeWordCharacterData.rs` | `props::GeneralCategory` | `$L \cup M \cup N$` 的成员判断 | 用字母、标记、数字的 general-category 值集合实现，删除表。 |
| `core/UnicodeScriptEvidenceData.rs` | `props::Script` | `Neutral` / `EastAsian` / `Other` evidence 映射 | 保留 mapping 文件或函数，删除 Script range 表。 |
| `linebreak/UnicodePunctuationLineBreakData.rs` | `props::LineBreak` | Tiqian 消费的有限标点类与 `UnicodePunctuationLineBreakClass` 映射 | 保留枚举和 mapping，删除表。 |

`UnicodeEmojiStyleVariationData.rs` 不在此表内：它属于前述保留项，不能与 `UnicodeEmojiData.rs` 一并删除。

### `SourceInteractionBoundaries.rs`

只替换其中的标准 Unicode 属性范围；交互边界算法及 UTF-16 编解码保持不变。

| 当前判断 | 替换为 | 继续本地保留 |
| --- | --- | --- |
| BMP 与 supplementary variation-selector 范围 | `props::VariationSelector` | `ZWNJ` 是否作为交互 extender 的 Tiqian 规则。注意该属性覆盖范围可能宽于旧的两段范围；实现和测试按 ICU4X 属性语义固定。 |
| Emoji modifier 范围 | `props::EmojiModifier` | modifier 必须跟随 modifier-base 的组合条件。 |
| Regional Indicator 范围 | `props::RegionalIndicator` | 两个 RI 配成一个交互单元的规则。 |
| Hangul L/V/T/LV/LVT 范围及 `% 28` 推导 | `props::HangulSyllableType`（必要时以 `props::GraphemeClusterBreak` 取同类标准分类） | L/V/T/LV/LVT 的组合循环。 |
| `Mn` / `Mc` / `Me` 的 `unicode-general-category` 查询 | `props::GeneralCategory` | extender 的消费顺序。 |

以下内容是序列/协议语法或 UTF-16 机制，不是手工维护的 Unicode 属性表，继续使用本地常量与现有逻辑：CRLF、ZWJ/ZWNJ、emoji tag 范围、keycap 语法、高低 surrogate 范围与 UTF-16 code-unit 操作。

### `ClusterRoleResolution.rs`

- `UnicodeEmojiData` 改为 `props::Emoji`。
- `UnicodeEmojiModifierBaseData` 改为 `props::EmojiModifierBase`。
- variation selector 范围改为 `props::VariationSelector`。
- `Mn` / `Mc` / `Me` 查询改为 `props::GeneralCategory`。
- Emoji modifier 的标量范围改为 `props::EmojiModifier`。
- keycap base、U+FE0F 作为 emoji-style 选择符、U+20E3，以及 `UnicodeEmojiStyleVariationData` 的 base-set 判定继续是本地 sequence grammar；不得改用 ICU4X emoji sequence API 扩大或缩小该集合。

### `FontPolicy.rs`

替换标准脚本/字类判断，但保留字体角色政策的优先级和选择逻辑：

- Bopomofo、Han、Latin 等脚本成员判断改查 `props::Script`；仅需统一表意文字时使用 `props::UnifiedIdeograph`。
- `is_cjk_code_point` 继续把 Bopomofo 与 Han 的结果合成为 `CjkText`，这是 Tiqian font-role policy，不由 ICU4X 代替。
- `is_symbol_code_point` 的 `MathSymbol` / `CurrencySymbol` / `ModifierSymbol` / `OtherSymbol` 判断改为 `props::GeneralCategory`。
- `is_emoji_code_point` 改查 `props::EmojiPresentation`。
- CJK 标点、弯引号、所有可打印 ASCII、以及字体 fallback 优先级仍是 Tiqian 的显式政策或文本语法；不把这些选择泛化成 Unicode 属性。

### `LayoutQueries.rs`

- `is_han_ideograph` 的手工 CJK Unified Ideographs 与 compatibility ideographs 范围改为 `props::UnifiedIdeograph`。
- 不使用更宽泛的 `Ideographic` 属性替代。
- 选词算法、空白/连接符规则和 mandatory-break 字面集合保持原样。

### `UnicodePunctuationLineBreak.rs` 与 `LineBreak.rs`

- `UnicodePunctuationLineBreakData.rs` 的 `lookup` 改由 `props::LineBreak` 读取，再映射到现有 `UnicodePunctuationLineBreakClass`；仅映射 Tiqian 当前实际消费的 BA、BB、CL、CP、EX、HY、IN、IS、NS、OP、QU、SY 等类别，其余仍为 `Other`。
- `LineBreak::UnambiguousHyphen`（短名 `HH`）直接映射为 `HyphenHH`；已对全部 Unicode scalar 与旧范围表逐项核对，结果为零差异。
- `UnicodePunctuationLineBreakClass`、`UnicodePunctuationBoundaryResolver` 的有限 UAX #14 标点子集和 CLREQ tailoring 均不改变。
- `LineBreak.rs` 中 CR、LF、VT、FF、NEL、LS、PS、U+200B、WORD JOINER 与 BOM 的处理是 mandatory/soft-break 文本语法；不以泛化属性查询替换，保留当前字面规则与 CRLF 合并行为。

## 实施顺序

1. **接入**：加入 ICU4X 依赖并实现最小内部属性访问入口；调用方继续在交给 ICU4X 前处理 UTF-16 surrogate 和无效标量。
2. **迁移生成表消费者**：按上表逐项改掉 9 份生成数据集中的 7 份可替换表及其 module export；先迁移 emoji 和 word/script/line-break 查询，再删除已无消费者的文件。
3. **迁移手写范围**：处理 `SourceInteractionBoundaries.rs`、`ClusterRoleResolution.rs`、`FontPolicy.rs` 与 `LayoutQueries.rs` 中等价的标准属性范围；不改变 sequence grammar、font policy 或 interaction algorithm。
4. **收口现有依赖**：将所有 remaining `unicode_general_category`、`unicode_properties` 使用替换为 ICU4X 后，从 manifest 与 lockfile 移除二者；确认没有生成表模块、数据文件或 `lookup` 消费者遗留。
5. **回归与清理**：只删除已被替代的表、常量和注释；保留两项例外数据文件及其引用。不要重排无关代码，也不要顺便改造断行、分词、字体 fallback 或 emoji sequence 模型。

每一步应以独立、可回退的提交组织；若任何 ICU4X 属性无法精确表达现有“标准属性”意图，停止该项并整理为决策点，不猜测替代语义。

## 验收

实现时至少完成以下检查：

1. 运行 `cargo check` 与 `cargo test`。
2. 运行 `bash tools/verify-all-fixtures.sh`，与 Tiqian fixture dump 做字节级比较；若 Unicode 属性升级导致有意差异，先形成决策并更新对照基线，不允许静默修改 golden。
3. 运行 `git diff --check`，审查 `Cargo.lock`、删除的表文件和例外文件未被误删。
4. 搜索确认：除两项保留数据集外，不再有 `Unicode*Data` 标准属性表、手工 Script/Han/emoji-property/LineBreak-property 区间或 `unicode-general-category` / `unicode-properties` 依赖使用。

完成条件是：所有替换对象由同一 ICU4X Unicode 17 属性源提供；两项明确例外仍以本地数据集存在；Tiqian 的 policy、sequence grammar 与 UTF-16 source 语义没有被 ICU4X API 意外接管。
