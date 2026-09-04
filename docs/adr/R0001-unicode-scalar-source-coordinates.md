# ADR R0001: 核心 source coordinate 使用 Unicode scalar value

- Status: Accepted
- Date: 2026-09-03
- Implementation: Completed 2026-09-04
- Relates: [2026-09-03 Unicode scalar source coordinate 迁移](../iteration/2026-09-03-unicode-scalar-source-coordinates.md)

## Context

当前 Rust 核心将 `TextRange`、span、cluster、glyph source mapping、line range、断行 offset
与交互查询统一表示为 UTF-16 code-unit offset。这一选择用于对应 Kotlin `String` 的坐标语义。

Rust 的 `Text` 实际保存 UTF-8 文本。为支撑 UTF-16 range，它在需要时建立 UTF-16 code unit、
UTF-16 到 UTF-8 byte 和 UTF-8 byte 到 UTF-16 的索引。补充平面字符占两个 UTF-16 code unit，
于是 range 可以表示代理对中点；该位置不能用于 Rust `str` 的子串范围。

现有实现还存在坐标语义不一致：`Hyphenator` 按 Unicode scalar 返回断词位置，paragraph shaping
阶段将该值直接加到 UTF-16 range 起点。当前西文测试输入通常位于 BMP，因此未暴露该问题。

本仓库处于开发阶段。layout fixture 与 golden 已由 Rust 仓库在 `tests/fixture_layout/` 本地维护，
日常验证不依赖 Kotlin fixture、golden 或 checkout。

## Decision

Rust 核心和公开 Rust API 的 source coordinate 统一使用 Unicode scalar value 的零基 offset。
所有 source range 使用半开区间 `[start, end)`。

公开 API 新增 `ScalarOffset(i32)`，避免以原始 `i32` 混用 source coordinate、UTF-8 byte offset
和容器下标。保留 `TextRange` 名称，其端点和公开访问器使用 `ScalarOffset`；不改名为
`ScalarRange`。

`geometry` 提供 `scalar_offset(i32)` 与 `text_range(start, end)` 两个具名构造器。后者构造半开
scalar source range `[start, end)`。它们在调用点明确标识 Unicode scalar source coordinate，并保持
`ScalarOffset` 与 `TextRange` 的强类型边界。

`Text` 继续以 UTF-8 `String` 保存文本，并提供 scalar offset 与 UTF-8 byte offset 的集中映射。
Rust `str` 的子串范围、HarfBuzz cluster 和其他第三方文本接口继续使用 UTF-8 byte offset；它们不属于
source coordinate。

`Text` 提供 `chars()` 用于 Unicode scalar 顺序遍历，提供 `scalar_indices()` 返回当前文本视图内的
`(ScalarOffset, char)`。它们不暴露 UTF-8 byte 下标。`byte_len()` 明确返回当前视图的 UTF-8 byte
长度，不能作为 source coordinate；不提供语义不清的 `len()` 或 `str_len()`，也不以广泛转发方式
重建 `str` API。

selection、emoji shaping、紧急断行和东亚间距继续使用 source interaction boundary。interaction
boundary 由 Unicode scalar 序列计算，但不等同于 scalar boundary：组合标记、CRLF、区域指示符对、
Hangul 序列和 ZWJ emoji 序列仍保持各自的不可分割规则。

不在生产 API 或核心 pipeline 中保留 UTF-16 coordinate、旧 range 类型、双坐标字段或自动转换。
Rust 自有 fixture、golden、测试和 debug dump 在迁移中直接改为 scalar coordinate，并以更新后的
本地预期输出验证。

### Invariants

- source text 保持原样；display substitution 不改变 source range 的含义。
- layout 与 paint 继续使用同一 shaping 结果和 glyph source mapping。
- UTF-8 byte offset 仅在 Rust 文本和 shaping 边界使用，不进入 source API。
- `Text::chars()` 的遍历序号和 `Text::scalar_indices()` 的 offset 均相对于当前文本视图；写入全文
	source range 前由调用方按已有 range 关系平移。
- interaction boundary 的 Unicode 规则在改坐标时保持，不退化为逐 scalar 的选择或断行。
- fixture golden 继续比较完整 deterministic layout dump，并由 Rust 仓库维护。

## Consequences

### 正面影响

- source range 始终对应 Rust `str` 的 scalar 边界，不再表示代理对中点。
- 字体角色、Unicode 属性、断词和 source range 使用同一基本坐标单位。
- `Hyphenator` 的返回位置可直接参与 paragraph shaping，不再依赖 ASCII/BMP 前提。
- UTF-16 索引表、代理对中点标记和多处 surrogate 特判可以从核心路径移除。
- Rust 调用方不必了解 Kotlin 的 UTF-16 `String` 下标语义。

### 需要接受的变化

- 补充平面字符之后的 range、break offset、line range、cluster range、caret 与 dump 数字都会变化。
- emoji 代理对中点不再是可表示的普通 caret 或 selection offset。
- 现有本地 fixture golden、单元测试和示例中的 UTF-16 坐标断言需要整体更新。
- Kotlin 上游若修改 UTF-16 source coordinate 的行为，只作为差异审计输入，不直接决定 Rust 实现。

## Alternatives considered

- **保留 UTF-16 source coordinate。** 否决：补充平面字符会在 Rust source API 中留下不能作为
	Rust `str` 子串范围端点的代理对中点，且继续要求 `Hyphenator` 与 pipeline 在不同坐标单位之间转换。
- **使用 UTF-8 byte offset 作为 source coordinate。** 否决：byte offset 是 Rust 字符串和 shaping
	边界的实现细节，不能表达面向调用方的字符位置语义。
- **使用 grapheme offset。** 否决：span、cluster、glyph 与断行需要保留 Unicode scalar 范围；caret
	与 selection 的不可分割语义由独立的 source interaction boundary 表示。
- **同时保留 UTF-16 与 scalar coordinate，或提供自动兼容转换。** 否决：这会允许同一 pipeline 混用
	两种单位，并使 Rust API 的坐标语义不明确。仓库处于开发阶段，应一次性切换本地 API 和验证资产。

## Verification

已完成的验证：

1. `Text` 覆盖 scalar 与 UTF-8 byte 的双向映射，包括 BMP、补充平面字符、组合标记和 ZWJ emoji。
2. 独立测试覆盖 selection、caret、word selection、emergency break 与 interaction boundary。
3. 独立测试和 fixture dump 覆盖 hyphenation、cluster、line、glyph 与 debug range 的 scalar 一致性。
4. `unicode-scalar-source-coordinates` fixture 覆盖补充平面字符、组合标记、emoji style variation、
	 emoji modifier、ZWJ、区域指示符、keycap 与 emoji tag sequence，并固定三种断行器的完整 dump。
5. `cargo test --all-targets` 通过；独立测试 1327 项、`paragraph-demo` 测试 19 项；53 项 fixture
	 golden 通过，且已运行 `git diff --check`。