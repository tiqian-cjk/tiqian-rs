# Kotlin 上游同步状态

本文件是 Rust 跟进 Kotlin 上游的长期入口，记录跟进策略、当前状态和日期归档索引。每次
完成一轮同步时更新本页；完整调研、对照表、决策、实施记录和验证证据存入
`docs/tracking/<日期>-<起始短commit>-<结束短commit>.md`。

Kotlin 仓库的 ADR 是同步 Kotlin 算法与架构取舍的重要来源。Rust 的有意架构差异使用 Rust ADR
记录，并同步更新 `docs/key-differences.md`。

## 跟进目标与边界

目标是让 Rust 排版核心与 Kotlin 的算法和结构化 decision 保持一致，同时保留语言实现差异，
例如所有权模型、缓存 API 或以固定版本 ICU4X 属性数据替代 Kotlin 仓库内的标准 Unicode 属性表。
已接受的 Rust 架构差异记录在 `docs/key-differences.md` 与对应 ADR。

每次对照前，必须先阅读 `docs/key-differences.md`，了解已知的关键差异、分类和处理策略。

每次对照默认关注：

- `docs/` 中与 engine 取舍直接相关的 ADR、规则审计和路线记录；
- `engine/src/` 中 `commonMain` 的排版核心与 `commonTest` 的行为测试；
- 对应的 Rust `src/`、`tests/fixture_layout/` 与 `tools/`。

默认不把前端、demo、平台 adapter、发布脚本和无关文档带入 Rust 核心同步。上游变动如果影响
输入 wire、fixture、shaping 约定或 renderer 可重放证据，则在对应日期归档中记录扩大后的范围。

## 工作方式

### 建立审计范围

对新的 Kotlin 上游终点 `UPSTREAM_HEAD`，先运行：

```shell
git -C <tiqian-kotlin-repository> diff --stat <已审计终点> UPSTREAM_HEAD -- docs engine/src
git -C <tiqian-kotlin-repository> diff --name-status <已审计终点> UPSTREAM_HEAD -- docs engine/src
```

按功能、测试、fixture/wire 工具和文档决策归类变更。改变既有取舍前阅读相关 Kotlin ADR，
不得仅按文件名或代码相似度判断同步范围。

### 映射 Rust 对应物

逐项找到 Rust 实现、测试和本地 fixture golden，并使用以下状态记录在日期归档中：

| 状态 | 含义 | 后续动作 |
| --- | --- | --- |
| 已同步 | 行为和必要测试已经等价存在 | 记录路径和证据，不重复移植 |
| 有意差异 | Rust 有已接受的不同决策 | 写明 ADR、原因和验证方式 |
| 待补测试 | 运行语义已存在，缺少上游新增的回归范围 | 由用户决定是否补测 |
| 待同步 | Rust 缺实现、测试或必要的工具能力 | 形成小范围实施计划 |
| 不适用 | Kotlin 改动属于 Rust 当前边界之外 | 写明边界 |

代码结构不同不等于不一致。Unicode 数据源、Kotlin `Any` 缓存与 Rust `Arc` 缓存、测试目录
布局不同，都按实际行为、输入输出和 debug decision 判断。

### 实施与验证

一次同步只处理可独立验证的一组改动。行为变更同步对应测试；只补测试时也在日期归档中写明
它固定的上游行为。不要为了追赶上游引入第二套排版规则或临时兼容层。对有意改变 Rust 架构或
行为的决策，创建 Rust ADR 并同步更新关键差异文档。

根据改动范围运行：

```shell
cargo test
cargo test --test tiqian layout_fixture_golden_test
git diff --check
```

`tests/fixture_layout/` 保存 Rust 本地 52 项 deterministic stub fixture 与每项完整 dump golden。测试入口同时检查 fixture ID 与 golden 文件名集合，新增或移植 fixture 时必须同步更新两者，避免清单漂移。

涉及 Unicode 属性源时，标准 Unicode 属性固定使用 `icu_properties` 2.3.x 的编译期 Unicode 17
数据；emoji style variation base 集合和 UTR #59 `East_Asian_Spacing` 仍是 Rust 本地数据。

### 处理大跨度同步

1. 当发现 commit 之间跨度很大，如包含上百个 commit 或有大量文件变更时。不试图一次性了解全部，而是先获得文件 diff stat，将变更的文件分成多个批次来检查，期间可以多次编辑日志文档。
2. 应当在第一次查询 diff 后就把完整的变更文件列表写入文档，之后再对其分组。
3. 所有的要点必须经常更新到文档中，不要使用上下文或记忆维持。
4. 避免一次性读取太多代码导致上下文压缩死循环。
5. 不要使用 subagent 进行调研，但可以用其做复核。而 subagent 所说并非一定正确，需要对其进行验证。

## 记录规则

每轮审计创建
`docs/tracking/<日期>-<起始短commit>-<结束短commit>.md`，其中起始与结束短 commit 是本轮
Kotlin 审计区间的两端。文件记录审计范围、上游依据、映射表、用户选择、实现变动、验证结果
和未解决差异。同一天的多次同步使用各自的起止短 commit 创建独立文件。

仅在完成一轮同步后更新本页：更新“当前状态”，并在“日志摘要”顶部新增一行链接和结果概述。
日期归档中的详细记录不复制到本页。只有已实施并通过目标验证的上游 commit 才能推进
“已跟进终点”；只有审计不能推进该终点。

日期归档使用以下模板：

```markdown
# YYYY-MM-DD <起始短commit>..<结束短commit> Kotlin 上游跟进

## 审计范围

| 项目 | 值 |
| --- | --- |
| Kotlin 审计区间 | `<from>..<to>` |
| Rust 基线 | `<commit>` |

## 审计结论

<!-- 映射表、用户选择与决策记录。 -->

## 实施记录

<!-- Rust 变动、验证命令和未解决差异。 -->
```

## 当前状态

| 项目 | 值 |
| --- | --- |
| 上游仓库 | `<tiqian-kotlin-repository>` |
| Rust 仓库 | 本仓库 |
| 最近审计区间 | `eb26f889c57d50e52e41d3a76185cdb6a3bdba45..2fae0df461819932dc9ef0153b79be9ad0038959` |
| Rust 基线 | `b8ed5d75c646053f7aad0fdf3ca2af4c96586736` |
| 同步状态 | 同步完成 |
| 已跟进终点 |  |

## 日志摘要

- [2026-08-31 `2fae0df..59fca35`](tracking/2026-08-31-2fae0df-59fca35.md)：同步算法改动和大量测试用例。

- [2026-08-31 `eb26f889..2fae0df`](tracking/2026-08-31-eb26f889-2fae0df.md)：同步五项回归测试并修正 NaN 压缩行为；记录后续上游造成的 fixture parity 差异。
