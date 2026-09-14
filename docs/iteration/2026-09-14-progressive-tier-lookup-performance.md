# 2026-09-14 渐进断行 tier 查找性能

## 目标

测量并优化 `decide_progressive_break` 在长 technical span 内收集 tier、寻找各 tier 最右 boundary、
寻找 Emergency boundary 和最终 boundary 时的重复区间扫描。

## 范围与设计

当前实现先把 active span 内的 tier priority 收集到 `Vec` 并排序去重，再针对每个 priority 反向扫描
整个候选区间寻找最右 boundary。完成 density 比较后，还会分别反向扫描寻找 Emergency boundary 与最终选择的
boundary。tier 数量固定为 5，因此这些扫描的上界是常数倍区间长度。

本轮新增独立 release example。输入使用 8,192 个 Latin cluster，把 Whitespace、Structural、Syllable 和
WholeToken boundary 放在区间开头，将 active Emergency boundary 放在 overflow 位置。有限 line limit 强制评估
所有 tier density；每次调用都覆盖最长的反向 tier 查找路径。构造输入不计入测量。

候选实现只在 `progressive_break_priority_for_line` 的现有区间扫描中，用固定大小数组记录每个 priority 的
最右 boundary。它保留 priority 的既有顺序、density 计算和 Emergency fallback；`decide_progressive_break`
直接使用已记录的最终 boundary，避免再次反向扫描。

## 验证

1. 在 release 下记录专用基准基线；
2. 运行渐进断行定向测试；
3. 对候选实现执行交替 A/B 测量；
4. 运行全量测试和 layout fixture golden，禁止更新 golden；
5. 检查编辑器诊断、`git diff --check` 和本文档风格。

## 基线

Windows x86_64 release 下，`progressive-tier-lookup-bench` 使用 8,192 个 Latin cluster、
5 个 tier、20 轮预热、200 个样本、每样本 100 次调用。构造输入不计入测量。

| 版本 | median |
| --- | ---: |
| 初次 A：Vec、排序和反向查找 | 65.820 µs/call |
| 初次 B：固定数组记录最右 boundary | 37.526 µs/call |
| 第二个 A：Vec、排序和反向查找 | 64.553 µs/call |
| 第二个 B：固定数组记录最右 boundary | 37.654 µs/call |

第二个 B 相对第二个 A 改善 41.7%。

## 候选改动

`progressive_break_selection_for_line` 在一次按 boundary 递增的扫描中，以固定 5 槽数组记录 active span 内每个
tier 的最右 boundary。随后按既有 priority 顺序评估 density，直接返回选中的 priority 和 boundary。
这移除了 `Vec` 分配、排序、对每个 priority 的反向查找，以及调用方对最终 boundary 的额外反向查找。

固定数组只覆盖 `ProgressiveBreakTier` 的五个已知值。priority 顺序、density 计算、最松 tier 比较和 Emergency
fallback 条件保持不变。

## 验证结果

1. `cargo test --test tiqian progressive_break_decisions`：19 项通过；
2. `cargo test --test tiqian progressive_break_decisions_tail`：2 项通过；
3. `cargo test --test tiqian progressive_technical_break`：4 项通过；
4. `cargo test`：1,349 项通过；
5. `cargo test --test tiqian layout_fixture_golden_test`：52 项 fixture 通过，未更新 golden；
6. 编辑器诊断无错误。

默认 Greedy 的 `paragraph-layout-bench` 使用 20 轮预热、200 轮测量、宽度 672/360/960、缩放 1。
layout median 为 2.298 ms、2.369 ms 和 2.268 ms，与本轮前同条件记录基本一致。普通页面缺少长
technical span，端到端数据只验证默认路径稳定；本轮保留依据是 tier 专用基准的 A/B/A/B 测量。
