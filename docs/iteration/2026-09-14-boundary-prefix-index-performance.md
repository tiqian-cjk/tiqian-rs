# 2026-09-14 断行边界前缀索引性能

## 目标

评估 Greedy 与 Lookahead 断行中为 CJK、Sino-Western 和合并 gap boundary 执行区间计数时，
前缀索引能否覆盖构造成本并带来可重复收益。

## 现状

`plan_paragraph_lines()` 在每次段落规划中构造 `cjk_inter_char_boundaries` 与
`sino_western_boundaries`。Greedy 与 Lookahead 在 `break_lines()` 内克隆并合并这两个集合，
随后通过逐项 `HashSet::contains` 统计 line gap。DP 已在自己的 `DpContext` 中建立三类前缀计数，
因此本轮不把 DP 的既有实现重复计算为收益。

## 范围与设计

本轮先新增独立 release example，模拟一次断行策略调用的完整边界计数成本：

1. 现有路径复制 CJK 集合并合并 Sino-Western 集合，再对每个区间逐项查询；
2. 候选路径一次遍历两个原始集合并建立 gap、Sino-Western、CJK 三组 prefix，再以两次数组读取完成每个区间计数。

基准提供 greedy-like 与 lookahead-like 查询密度。构造输入不计入测量；每次调用都包含当前路径的
合并集合构造，或候选路径的三组 prefix 构造。它只衡量可替代的 boundary count 工作，不能代表完整布局。

只有候选路径在包含构造成本时稳定获益，并且后续 `paragraph-layout-bench` 没有退化，才会考虑在内部断行路径引入共享 index。

## 验证

1. 在 release 下记录专用基线；
2. 评估构造与查询密度的摊销点；
3. 若候选值得保留，再决定是否改动生产路径；
4. 若实施，运行三类断行器测试、全量测试和 fixture golden；
5. 检查编辑器诊断、`git diff --check` 和本文档风格。

## 摊销结果

Windows x86_64 release 下，`boundary-prefix-index-bench` 使用 8,192 个 boundary 位置，
每次调用包含当前路径的集合合并，或候选路径的 prefix 构造。

单一合并 gap prefix 在 32 个 64-cluster 区间的 greedy-like 工作量中为 13.617 µs/call，
当前逐项集合查询为 8.651 µs/call。默认引擎使用 Greedy，因此不为默认路径加入该索引。

同一输入在 lookahead-like 工作量中有 256 个区间查询：gap prefix 为 13.371 µs/call，
当前查询为 21.387 µs/call。查询密度为 128 个区间时两者接近持平：13.465 µs/call 与
14.358 µs/call。三组 prefix 的构造成本约为 30 µs/call，当前范围不采用。

## 候选改动

`LookaheadLineBreaker::break_lines()` 在已经构造合并 `gap_boundaries` 后，为当前 cluster 数建立一个
私有 `GapPrefix`。`score_candidate`、`badness` 和 committed density 使用 prefix 的两次数组读取统计
line gap。fill PushIn 继续使用原有 `HashSet`，Greedy、DP、公开 `LineBreakerConfig` 和 trait 签名不变。

## A/B/A/B 测量

`lookahead-boundary-index-bench` 直接运行默认 Lookahead（window=2、future horizon=2）的
`break_lines`。输入为 8,192 个 CJK cluster、稠密 CJK/Sino-Western boundary、行宽 64，产生 128 行。

| 版本 | median |
| --- | ---: |
| 初次 A：逐项集合查询 | 433.400 µs/call |
| 初次 B：GapPrefix | 200.260 µs/call |
| 第二个 A：逐项集合查询 | 413.980 µs/call |
| 第二个 B：GapPrefix | 194.965 µs/call |

第二个 B 相对第二个 A 改善 52.9%。收益来自 Lookahead 候选评分与未来行评分反复查询相同行区间时，
从逐项 `HashSet::contains` 改为 `$O(1)$` prefix 差值；每次 `break_lines` 的 prefix 构造成本已计入。

## 验证结果

1. `cargo test --test tiqian greedy_line_breaker`：12 项通过；
2. `cargo test --test tiqian lookahead_line_breaker`：13 项通过；
3. `cargo test --test tiqian paragraph_dp_line_breaker`：7 项通过；
4. `cargo test`：1,349 项通过；
5. `cargo test --test tiqian layout_fixture_golden_test`：52 项 fixture 通过，未更新 golden；
6. 编辑器诊断无错误。

`paragraph-layout-bench` 增加 `--strategy greedy|lookahead` 参数，默认仍为 Greedy。字体 backend 下，
Lookahead 使用 20 轮预热、200 轮测量、宽度 672/360/960、缩放 1 的 layout median 为
2.349 ms、2.479 ms 和 2.298 ms。普通页面的候选评分密度低于专用输入，端到端数据只验证最终路径稳定，
本轮保留依据是包含 prefix 构造成本的策略级 A/B/A/B 测量。
