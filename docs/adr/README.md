# tiqian-rs ADR 索引

ADR 记录 tiqian-rs 已接受的架构决定。每份记录使用 Context、Decision、Consequences、
Alternatives considered 和 Verification 说明当时的取舍与验证证据；后续改变既有决定时新增 ADR，
不改写原记录的背景与决策。

实施状态与历史过程见 `docs/iteration/`；Kotlin 上游同步状态与 Rust 有意差异见
[`docs/tracking.md`](../tracking.md) 和 [`docs/key-differences.md`](../key-differences.md)。

## 核心模型

- [R0001 核心 source coordinate 使用 Unicode scalar value](R0001-unicode-scalar-source-coordinates.md)