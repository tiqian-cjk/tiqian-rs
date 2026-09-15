# ADR R0003: 统一字体后端

- Status: Accepted
- Date: 2026-09-15
- Implementation: Complete
- Relates: [2026-09-15 统一字体后端与选择 shaping 一体化](../iteration/2026-09-15-feat-unified-font-backend.md)

## Context

当前 `ParagraphLayoutEngine` 分别注入 `FallbackResolver`、`TextShaper` 和
`FontMetricsResolver`。这三个 trait 本来都属于字体后端能力，但调用边界将字体候选决定、实际 glyph
mapping/OpenType shaping 和 metrics 查询拆成了独立阶段。`FallbackResolver` 在 shaping 前返回候选字体决定，
`TextShaper` 随后对该候选执行 shaping，`FontMetricsResolver` 又可根据字体族和选择文本解析字体。

这三个接口没有强类型方式要求它们使用同一个 physical face。调用方若要在候选字体之间按 glyph coverage
fallback，只能额外查询 cmap 或执行其他独立映射；该查询不能复用为最终 shaping 结果，也不能证明 GSUB、
variation selector 或 emoji ZWJ 序列不会产生 `.notdef`。

Kotlin 上游仍使用这三个独立接口。Rust 需要让已解析字体、glyph、metrics 和 glyph replay 使用同一个
face identity，因此采用不同的接入边界。

## Decision

`ParagraphLayoutEngine` 的普通接入路径只注入一个主字体后端。后端接收由 layout 确定的 shaping range、
source/display text、样式、font role、locale 与 OpenType feature；候选 face 的顺序由后端按这些输入和受控目录内部确定。

每次 `FontBackend::shape` 请求都是一次字体选择与完整 shaping 的原子操作。段落 shaping 为断行、标点
替换或 retry 产生的不同 request 使用各自的 range 和各自的候选尝试；一个旧的 layout range 被拆成多个
request 时，允许这些 request 独立得到不同的最终 face。`FontResolution` 保持各自的 request range，多个
request 不聚合为旧的 layout range。

后端依次对候选 face 执行完整 shaping：

1. 第一个不含 glyph id `0` 的结果成为最终结果；
2. 后端停止后续候选的 shaping；
3. 所有候选均含 glyph id `0` 时，返回首选 face 的 shaping 结果，并保存每个候选的缺字数量。

后端结果包含最终 `FontFaceId`、glyph、cluster、advance、bounds、metrics 与候选尝试信息。metrics 和 glyph
replay 只使用已解析的 `FontFaceId`，不再接收用于再次选择字体的文本或字体族列表。

`FontFaceId` 位于 `core` 的中立模块，标识 physical SFNT face、collection index 和 variation instance。
字号属于样式请求，不属于 face identity。强制换行、软断行和行内对象等合成 cluster 显式表示无字体。

本决策适用于简体中文横排中的 CJK、Latin 和 emoji。每个 shaping request 只产生一个最终 face；emoji
完整 grapheme 交给同一个受控 face，保留该次 shaping 产生的全部 glyph。宽度无关注释缓存保存该次
shaping 的最终 face 与候选尝试信息。

## Consequences

### 正面影响

- fallback 判断与实际 shaping 使用同一次候选尝试的 glyph 结果；
- layout、metrics 与 glyph replay 可引用同一 `FontFaceId`；
- 全部候选缺字时保留可解释的受控 `.notdef` 输出与候选尝试信息；
- cache 命中时可直接复用已经解析的最终 face 与 shaping 结果。

### 需要接受的变化

- `FallbackResolver`、`TextShaper` 和 `FontMetricsResolver` 不再组成普通平台接入路径；
- deterministic stub、HarfRust/SkRifa 示例和测试实现需要迁移到统一后端；
- 公开 API、fixture dump 和结构化 debug 需要使用 `FontFaceId` 与候选尝试信息。

## Alternatives considered

- **保留三个独立 trait 并加强调用约定。** 否决：类型仍不能限制三个组件解析不同 face，额外 coverage
  查询仍不能复用为 shaping 结果。
- **在调用方执行 cmap 预检查。** 否决：它重复 glyph mapping，且不代表 OpenType substitution、variation
  selector 与 emoji sequence shaping 成功。
- **一次 shaping request 输出多个 face。** 不纳入本决策：当前目标语料没有要求跨 face 拼接；该能力需要独立的
  cluster、metrics 与重放语义设计。

## Verification

实施完成时至少验证：

1. 首选候选不含 `.notdef` 时，后端只对该候选执行布局使用的 shaping；
2. 首选候选缺字且后续候选完整覆盖时，最终 glyph、metrics 与 replay 指向同一 fallback face；
3. 所有候选缺字时，结果保留首选 face 的 `.notdef` glyph 与完整候选尝试信息；
4. Latin 组合文本 `é`、emoji modifier、ZWJ、旗帜与 keycap 保持 source/display range 和单 face replay 语义；
5. annotation cache 命中时，复用的 shaping 与 metrics 使用同一最终 face；
6. `cargo test --all-targets`、`cargo check`、fixture golden、桌面字体示例、`git diff --check` 与文档风格检查通过。

本 ADR 的方案 A 已完成实现与验证：统一 backend request 的字体选择粒度已固定，fixture golden 已同步，
布局几何和断行结果保持稳定。
