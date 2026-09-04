# 关键差异文档

本文档记录 tiqian-rs 实现与 Kotlin 上游在行为、实现和测试上的关键差异。

这些差异可能来源于：

1. 因代码简介性、架构设计、语言特性等技术决策原因造成。其并非疏忽或错误。这些差异不应作为同步时的差异结论。
2. Rust 版本先于 Kotlin 版本调整了其算法或架构，或提前修复了上游的错误。这些差异应当在同步时被指出，但不应作为需同步的项目。

该文档为手工或 AI 维护，每当决策认为某个差异应当出现时，其应当被记录在当前文档中。

## 关键差异列表（技术决策）

### Source coordinate 使用 Unicode scalar value

R0001 已决定 tiqian-rs 的核心和公开 Rust API 使用 Unicode scalar value 表示 source offset 与
range。Kotlin 上游继续使用 UTF-16 code unit；这一差异是 Rust 的有意架构决策，不属于待同步项。

Rust `Text` 保持 UTF-8 存储，Rust `str` 与 HarfBuzz 等边界继续使用 UTF-8 byte offset。selection、
emoji shaping 和紧急断行使用独立的 source interaction boundary，不能退化为任意 scalar boundary。

实施完成后，Rust 本地 fixture、golden、debug dump 与测试预期同步使用 scalar coordinate；Kotlin
fixture 和 golden 不参与 Rust 日常验证。

### Unicode 数据源

不同于 Kotlin 版本在仓库中直接维护 Unicode 属性表，tiqian-rs 依赖 ICU4X 的 Unicode 数据源。当前两边均使用对应 Unicode 17 的数据，其不应出现差异。若将来上游版本更新为 Unicode 18，tiqian-rs 也应当同步更新到 ICU4X 的 Unicode 18 版本。

## 关键差异列表（实现差异）

### 单点交互查询统一到 interaction boundary

Kotlin 的 `getCursorRect`、`getLineForOffset`、`getBoundingBox` 与
`getOffsetForPosition` 保留原始 UTF-16 offset；只有
`coerceSelectionOffset` 和 `getSelectionOffsetForPosition` 保证返回 interaction boundary。

Rust 的对应单点查询采用更严格的公开 API 契约：

- `get_cursor_rect`、`get_line_for_offset` 与单点 `get_bounding_box` 收到 interaction unit
	内的 scalar offset 时，先按最近边界归一化；等距时取后一个边界；
- `get_offset_for_position` 返回按最近边界归一化后的 offset；
- `coerce_selection_offset` 与 `get_selection_offset_for_position` 继续提供显式方向或几何距离驱动的
	caret 吸附；
- `get_selection_word_boundary` 保留原始 scalar offset，用它定位所属 interaction unit 后再返回完整
	unit 或扩展后的词范围。这一点与 Kotlin 的双击选词语义一致，不属于该差异。

因此 Rust 不会通过普通单点查询暴露组合标记、CRLF、区域指示符对、Hangul 序列、emoji modifier
或 ZWJ emoji 序列内部的位置。该取舍已由
`docs/iteration/2026-09-03-unicode-scalar-source-coordinates.md` 的交互查询设计确认，作为 Rust
scalar source coordinate API 的安全不变量，不是待同步项。

## 关键差异列表（其他）

### Rust 本地 layout fixture 与 golden

Rust 在 `tests/fixture_layout/` 维护 52 项 deterministic stub fixture 与完整 layout dump golden。
它们由 `cargo test --test tiqian layout_fixture_golden_test` 执行，并作为 Rust 日常排版回归测试。
Kotlin fixture、golden、recorded shaping evidence 和相邻 checkout 不属于 Rust 测试依赖。

### Kotlin Paragraph-DP 实验与调优探针

Kotlin `ParagraphDpReferenceExperiment.kt` 保留为上游的算法实验，不迁移到 tiqian-rs。它包含 reference DP、fixture 扫描和基准输出，不属于 Rust 排版引擎的生产行为或默认回归测试。Rust 的 Paragraph-DP 正确性由独立的line-breaker、coverage 与 tier-pool 测试覆盖。

Kotlin `ParagraphDpTuningProbe.kt` 的两个函数使用 JVM `AwtTextShaper` 和宿主字体 advance 评估中文正文的拉伸与压缩观感。tiqian-rs 没有同源 AWT 字体 shaping 后端，stub 的测量值不等价，因此这两个 AWT 调优 probe 不适用。
