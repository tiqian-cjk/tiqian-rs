# 2026-09-14 渐进断行候选密度性能

## 目标

测量并优化 `progressive_candidate_stretch_density` 对同一候选区间的重复扫描，同时保持渐进断行选择、layout dump 和公开 API 不变。

## 范围与设计

该函数依次扫描 cluster 或 boundary 区间以计算 width、技术 whitespace capacity、Sino-Western gap 数、
terminal technical source unit 数和可选的 CJK gap 数。`decide_progressive_break` 会对不同 tier 的右侧候选重复调用该函数。

本轮添加独立 release example。输入使用长 Latin cluster 序列、五个 progressive tier、稠密 CJK 与
Sino-Western boundary。两个场景分别覆盖 terminal technical tracking density 和 CJK gap density。
构造输入不计入测量；输出每次 `decide_progressive_break` 调用的 min、median、p95、mean、max。

在取得基线后，候选实现会在一个按 cluster index 递增的循环中按原顺序累加 width，并同时计算其余
可在同一区间取得的统计。CJK gap 统计继续仅在 terminal technical gap 不可用时执行。

## 验证

1. 在 release 下记录专用基准基线；
2. 运行相关 progressive 断行测试；
3. 运行 layout fixture golden，禁止更新 golden；
4. 对候选实现执行交替 A/B 测量；
5. 检查编辑器诊断、`git diff --check` 和本文档风格。

## 基线

Windows x86_64 release 下，`progressive-break-density-bench` 使用 8,192 个 Latin cluster、
5 个 tier、20 轮预热、200 个样本、每样本 100 次调用。构造输入不计入测量。

| 场景 | median | p95 |
| --- | ---: | ---: |
| Terminal technical tracking | 162.012 µs/call | 164.063 µs/call |
| CJK gap fallback | 179.989 µs/call | 182.776 µs/call |

## 候选改动

`progressive_candidate_stretch_density` 使用一次按 index 递增的循环，按原顺序累计 width、
technical whitespace capacity、Sino-Western gap 数和 terminal technical source unit 数。
只有 terminal technical gap 不存在时，才继续执行原有的 CJK gap 扫描。

## A/B/A/B 测量

第二个 A 与 B 的 median 如下：

| 场景 | A：多次扫描 | B：合并扫描 | 变化 |
| --- | ---: | ---: | ---: |
| Terminal technical tracking | 172.138 µs/call | 152.931 µs/call | -11.2% |
| CJK gap fallback | 194.687 µs/call | 141.704 µs/call | -27.2% |

第一次 B 的 median 为 153.522 µs/call 与 143.782 µs/call，方向与第二次 B 一致。
专用基准的收益来自候选 tier 评分减少对同一区间的重复遍历；CJK fallback 仍保留一次 boundary 扫描。

## 验证结果

1. `cargo test --test tiqian progressive_break_decisions`：19 项通过；
2. `cargo test --test tiqian progressive_technical_break`：4 项通过；
3. `cargo test`：1,349 项通过；
4. `cargo test --test tiqian layout_fixture_golden_test`：52 项 fixture 通过，未更新 golden；
5. 编辑器诊断无错误。

端到端基准使用 20 轮预热、200 轮测量、宽度 672/360/960、缩放 1。第一次 B 的 layout median 为
2.396 ms、2.483 ms 和 2.353 ms；最终 B 源码直接重跑的结果为 2.297 ms、2.367 ms 和 2.268 ms。
第二次结果与本轮开始前记录的 2.291 ms、2.356 ms 和 2.260 ms 基本一致，未观察到可重复退化。
普通页面不含长 progressive technical span，端到端数据不作为本轮局部收益的依据。
