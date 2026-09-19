# ADR R0005：运行时布局的局部回退

- Status: Accepted
- Date: 2026-09-19
- Implementation: Complete
- Relates: [2026-09-18 运行时布局局部回退](../iteration/2026-09-18-feat-runtime-layout-local-fallback.md)

## Context

段落布局会消费调用方输入、可插拔 `FontBackend` 输出和 renderer 查询参数。此前这些路径中的
完整输入检查、构造器断言和局部 `expect` 会将单个异常值扩大为进程 panic 或整段布局中止；部分
检查还在每次 layout 或 technical tier retry 中扫描、clone 或排序全部输入。

布局核心需要优先产出当前可用结果：异常数据可以降低当前 cluster、line 或 layer 的几何质量，
但不能阻止后续 source range、后续行或下一次 layout。

## Decision

运行时布局采用局部回退模型：

- 不为确认完整数据约束而预扫描、排序、clone 或 validate 输入；
- 在实际读取、计算或推进循环的位置使用有界访问、范围收缩、默认值、截断或跳过，使当前操作继续；
- 调用方输入、backend 输出和策略数据异常只影响当前 span、cluster、line、decision 或绘制 layer；
- 缺失辅助数据采用字段中性语义，额外辅助数据不产生效果；
- 进入异常回退时以 `log` facade 输出固定、简短的 `warn` 文本；正常路径不为日志产生额外扫描、格式化或分配；
- 已由局部控制流或固定结构证明的关系不保留运行时 `expect`；复杂内部算法关系仅在 debug 构建中检查；
- 测试辅助对象不属于生产容错边界。没有生产调用点的测试 helper 保持在 `tests/`，不为它扩展正式 API。

## Consequences

### 正面影响

- 单个异常输入或 backend 结果不再中止整段渲染；
- 布局路径删除重复的完整输入扫描、clone 与排序；
- renderer 查询错误只影响当前调用，后续 layer 和 frame 可继续处理；
- 后端和平台接入可从结构化 debug 与固定日志观察局部退化。

### 需要接受的变化

- 异常数据得到局部退化的布局或绘制结果；
- 调用方应安装 logger，决定固定 `warn` 的收集和限频方式；
- 上游同步若引入更严格的运行时检查，必须按本 ADR 将其映射为消费点的局部回退，或新增 ADR 修改本决定。

## Alternatives considered

- **完整输入 validate 后返回 `Result`。** 否决：会重复扫描输入，并让单个异常值阻止整个段落输出。
- **在构造器和入口统一规范化全部数据。** 否决：会把消费语义集中为新的完整校验层，且增加热路径成本。
- **以 panic 保留内部关系的检查。** 否决：调用方数据和可插拔 backend 输出可到达这些关系，运行时不应因此终止。

## Verification

1. 每类回退都有异常输入或异常 backend 输出回归，验证后续 cluster、line 和独立 layout 继续执行；
2. 相关 normal fixture 与 layout golden 保持无差异；
3. `cargo test --test tiqian`、`cargo test --test tiqian layout_fixture_golden_test` 与
   `cargo test --all-targets` 通过；
4. `git diff --check` 与文档风格检查通过。
