# ADR R0006：字体合成策略与最终字体实例身份

- Status: Accepted
- Date: 2026-09-21
- Implementation: Complete
- Relates: [2026-09-21 字体合成策略与最终实例身份](../iteration/2026-09-21-feat-font-synthesis-contract.md)

## Context

Kotlin 上游的 `TextStyle` 只表达 `fontWeight` 与 `italic` 请求，`FontFaceId` 是由平台字体目录定义的
不透明字符串。上游没有表达调用方是否允许仿粗或仿斜的公共策略，也没有把实际采用的软件合成参数作为
可重放字体实例身份的一部分。

Rust 的统一 `FontBackend` 要同时为 shaping、metrics、glyph replay 和平台绘制返回同一个最终
`FontFaceId`。若后端在缺少物理静态 face 或标准 OpenType variation axis 时采用软件合成，只有请求的
字重或斜体信息不足以区分物理字体实例与合成实例；图集、ink bounds、glyph replay 与调试输出可能把它们
错误地视为同一个实例。

## Decision

Rust 将字体合成拆分为调用方策略和后端最终实例两个层次：

- `TextStyle.font_synthesis` 使用 `FontSynthesis` 表示允许的合成能力：`WEIGHT`、`STYLE`、`NONE`；默认值为
  `ALL`；
- `TextStyleOverride.font_synthesis` 未指定时继承基础样式，指定时整体替换该策略；
- `FontSynthesisInstance` 只记录 backend 实际采用的仿粗 em 相对外扩量和仿斜角度；
- `FontFaceId` 将 `FontSynthesisInstance` 与物理资源、collection member、实际采用的
  `FontVariationInstance` 一起作为最终身份；
- 后端先选择物理静态 face 和标准 OpenType variation axis，仍无法满足请求且策略允许时才生成带合成参数的
  最终 `FontFaceId`；合成参数不写入 `FontVariationInstance`；
- 排版核心只传递和比较最终 identity。字形 advance、metrics、ink bounds、轮廓与绘制由提供该 identity 的
  backend 使用同一实例产出。

本 ADR 不规定特定平台的仿粗量、仿斜角度或栅格化方法。平台 backend 负责这些参数及其几何实现；Huozi
当前实现使用 SDF 阈值仿粗和轮廓 shear 仿斜。

## Consequences

### 正面影响

- 调用方可以在段落样式和局部样式上明确允许或关闭 weight、style 合成；
- 物理 face、实际 variation 与不同合成参数不会共享 `FontFaceId`、glyph cache 或 replay 结果；
- 后端选择、度量和渲染可围绕一个最终 identity 保持一致，`LayoutResult` 仍可解释与重放；
- 后续平台 adapter 可采用各自适合的软件合成实现，而不改变 Tiqian 的输入和 replay 约定。

### 需要接受的变化

- `FontFaceId` 的 display 会包含无合成或实际合成状态，基于字符串的测试预期需同步更新；
- 支持软件合成的 backend 需要在候选选择、几何度量、轮廓或栅格化及 replay 路径中一致处理最终 identity；
- Kotlin 上游新增等价能力前，这一输入与 identity 模型属于 Rust 的有意差异；同步审计必须保留本 ADR 的
  约束，或通过新的 ADR 明确修改决定。

## Alternatives considered

- **只将字重和斜体请求交给平台后端。** 否决：调用方无法关闭软件合成，且无法标识最终采用的合成实例。
- **将合成参数编码为 OpenType variation axis。** 否决：软件合成不对应物理字体的 variation axis，会混淆
  物理字体能力与后端绘制行为。
- **只在 Huozi 内部保留合成状态。** 否决：Tiqian 无法把策略随样式继承，也无法把最终实例作为通用 replay
  identity 传递给其他 backend。

## Verification

1. `TextStyle` 默认启用 weight 与 style 合成，`TextStyleOverride` 的未指定继承与整体覆盖均有回归测试；
2. 相同物理资源和 variation、不同 `FontSynthesisInstance` 的 `FontFaceId` 不相等，display 可区分；
3. Huozi 验证物理字体优先、策略关闭、仿粗、仿斜、组合实例及最终 descriptor 回放；
4. Tiqian-rs 已执行 `cargo test`（1341 个测试通过）与 `cargo check --all-targets`；
5. Huozi 已执行完整 `cargo test`、`cargo check --all-targets --features woff`、render example 运行与
   `git diff --check`。
