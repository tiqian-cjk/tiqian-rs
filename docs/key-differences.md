# 关键差异文档

本文档记录 tiqian-rs 实现与 Kotlin 上游在行为、实现和测试上的关键差异。

这些差异可能来源于：

1. 因代码简介性、架构设计、语言特性等技术决策原因造成。其并非疏忽或错误。这些差异不应作为同步时的差异结论。
2. Rust 版本先于 Kotlin 版本调整了其算法或架构，或提前修复了上游的错误。这些差异应当在同步时被指出，但不应作为需同步的项目。

该文档为手工或 AI 维护，每当决策认为某个差异应当出现时，其应当被记录在当前文档中。

## 关键差异列表（技术决策）

### Unicode 数据源

不同于 Kotlin 版本在仓库中直接维护 Unicode 属性表，tiqian-rs 依赖 ICU4X 的 Unicode 数据源。当前两边均使用对应 Unicode 17 的数据，其不应出现差异。若将来上游版本更新为 Unicode 18，tiqian-rs 也应当同步更新到 ICU4X 的 Unicode 18 版本。

## 关键差异列表（实现差异）

暂无

## 关键差异列表（其他）

### Fixture 额外覆盖已转为常规回归测试

2026-09-03 的 LLVM 覆盖率对照显示，`cargo test --all-targets` 后再运行全部普通与 recorded fixture，仅额外覆盖两个 Rust 源文件中的七行。该增量不表示 Kotlin 与 Rust 的排版行为差异：普通 fixture 继续用于跨仓库 layout dump 对照，recorded fixture 继续验证 shaping evidence 回放。

其中 `ShapingDecisionInfoBuilder::script` 的 recorded shaping script 元数据，以及 `ParagraphDpLineBreaker` 对连续合成连字符的固定和连续惩罚，均已由 Rust 常规回归测试覆盖。后续日常 `cargo test --all-targets` 无需依赖 fixture 流程即可验证这两项局部行为。

### Kotlin Paragraph-DP 实验与调优探针

Kotlin `ParagraphDpReferenceExperiment.kt` 保留为上游的算法实验，不迁移到 tiqian-rs。它包含 reference DP、fixture 扫描和基准输出，不属于 Rust 排版引擎的生产行为或默认回归测试。Rust 的 Paragraph-DP 正确性由独立的line-breaker、coverage 与 tier-pool 测试覆盖。

Kotlin `ParagraphDpTuningProbe.kt` 的两个函数使用 JVM `AwtTextShaper` 和宿主字体 advance 评估中文正文的拉伸与压缩观感。tiqian-rs 没有同源 AWT 字体 shaping 后端，stub 的测量值不等价，因此这两个 AWT 调优 probe 不适用。
