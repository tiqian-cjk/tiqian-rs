# 2026-09-14 布局与文本索引性能优化迭代

## 目标

集中记录 2026-09-14 这一轮与文本索引、断行决策和行内边界统计相关的性能工作：

- 优化长技术词的连字符断行扫描；
- 合并 progressive density 计算中的重复区间遍历；
- 减少 progressive tier boundary 查找中的排序和反向扫描；
- 评估 Lookahead 断行的 boundary 前缀索引；
- 实验 UTF-8 scalar boundary 构建的 x86_64 SSE2 实现。

本轮的共同约束是保持 source coordinate、公开 API、断行选择、layout dump 和 fixture golden 不变。局部 benchmark 用于定位热点和比较实现，不把人工压力输入的结果直接解释为完整页面的端到端收益。

## 范围与总体结论

本轮涉及以下四个已保留的标量断行提交，以及一个未保留生产代码的 SIMD 实验：

| 提交或实验 | 内容 | 结论 |
|---|---|---|
| `74cded2` | 合并 hyphen break 中的区间扫描 | 保留 |
| `aa6d3c8` | 合并 progressive density 中的区间扫描 | 保留 |
| `6de2b57` | Lookahead 的 gap boundary 前缀索引 | 保留；关注额外临时内存 |
| `520e735` | progressive tier boundary 查找优化 | 保留 |
| SIMD 实验 | `Text::index` 的 SSE2 boundary 构建 | 不保留生产路径，仅保留实验记录 |

前四项优化没有改变公开参数或布局结果。它们的收益主要出现在长技术 span、长连字符候选区间或 Lookahead 重复评分等特定路径中；普通页面的完整 layout benchmark 只用于确认没有稳定退化，不能据此宣称相同幅度的整体提速。

## 共同验证方法

专用 benchmark 使用 Windows x86_64 release 构建，通常采用 20 轮预热、200 个样本和每样本 100 次调用。构造输入不计入测量，结果输出包含 median、p95 等统计值。完整 layout benchmark 使用 20 轮预热、200 轮测量、宽度 672/360/960 和缩放 1。

语义验证包括：

- 相关断行器定向测试；
- `cargo test` 全量测试；
- layout fixture golden 测试，未更新 golden；
- 编辑器诊断检查；
- `git diff --check` 和文档风格检查。

全量 Rust 测试通过 1,349 项，layout fixture golden 通过 52 项。四个标量优化提交和 SIMD 实验期间记录的测试结果均未出现断行或布局输出回归。

## 1. 合并 hyphen break 扫描

### 目标与现状

`decide_hyphen_break` 在确认候选为连字符断点后，需要在同一个 `[line_start, whole_word_end)` 区间内计算：

- word width；
- Sino-Western gap 数；
- 必要时的 CJK gap 数。

原实现先后进行 width 和 Sino-Western gap 的独立扫描。

### 实现

提交 `74cded2` 使用一次按 cluster index 递增的循环，同时累加 width 和 Sino-Western gap 数。width 仍按原始 cluster 顺序逐项以 `f32` 累加；CJK gap 统计仍仅在 Sino-Western capacity 未填满 deficit 时执行。公开的 `HashSet` 参数和函数接口保持不变。

### 性能与成本

专用基准使用 8,192 个 Latin cluster、一个末端连字符断点和人工设置的 boundary：

| 场景 | 两次扫描 | 合并扫描 | 变化 |
|---|---:|---:|---:|
| Sino-Western capacity 填满 deficit | 10.680 µs/call | 9.141 µs/call | -14.4% |
| CJK gap 填充剩余 deficit | 19.125 µs/call | 17.736 µs/call | -7.3% |

第一次 B 测量为 9.082 µs/call 和 17.104 µs/call，方向一致。该优化仍为 $O(n)$，额外空间为 $O(1)$，没有引入新的缓存或长期数据结构。

完整 layout benchmark 的 median 为 2.277 ms、2.353 ms 和 2.256 ms，相对本轮开始前的 2.291 ms、2.356 ms 和 2.260 ms，变化为 -0.61%、-0.13% 和 -0.18%。普通页面没有足够多的长连字符候选区间，因此端到端数据只说明没有观察到稳定退化。

### 验证与结论

- `progressive_break_decisions`：19 项通过；
- `decide_hyphen_break`：2 项通过；
- 全量测试：1,349 项通过；
- layout fixture golden：52 项通过，未更新 golden。

结论：**保留。**这是低复杂度、无额外内存的常数优化。性能结论应限定为“长 hyphen 区间中的局部收益”，不应外推为普遍的页面提速。

## 2. 合并 progressive density 扫描

### 目标与现状

`progressive_candidate_stretch_density` 需要计算 width、technical whitespace capacity、Sino-Western gap 数、terminal technical source unit 数，以及必要时的 CJK gap 数。原实现对同一候选区间分开执行多次扫描。

### 实现

提交 `aa6d3c8` 使用一次按 index 递增的循环，按原顺序累计：

- cluster width；
- technical whitespace capacity；
- Sino-Western gap 数；
- terminal technical source unit 数。

只有 terminal technical gap 不可用时，才继续执行原有 CJK gap 扫描。

该优化减少的是同一个 candidate 内的重复遍历。不同 tier candidate 之间仍可能分别扫描各自区间，因此整体渐进复杂度仍可表示为 $O(Tn)$；当前 tier 数量最多为 5，额外空间仍为 $O(1)$。

### 性能与基准边界

专用基准使用 8,192 个 Latin cluster、5 个 tier 和人工设置的稠密 boundary：

| 场景 | 多次扫描 | 合并扫描 | 变化 |
|---|---:|---:|---:|
| Terminal technical tracking | 172.138 µs/call | 152.931 µs/call | -11.2% |
| CJK gap fallback | 194.687 µs/call | 141.704 µs/call | -27.2% |

第一次 B 测量为 153.522 µs/call 和 143.782 µs/call，方向一致。

CJK fallback fixture 的 `span_range` 与尾部 opportunity 的布局并不完全符合生产数据形状，会强制大量调用进入 CJK fallback 扫描。因此 -27.2% 仅适用于这组压力测试。当前生产中的 technical span 通常只有十几个到几十个字符，长度远小于 8,192 个 cluster，不能直接用这组数据估计整体 layout 收益。

完整 layout benchmark 的两次结果与本轮开始前记录基本一致，未观察到可重复退化。

### 验证与结论

- `progressive_break_decisions`：19 项通过；
- `progressive_technical_break`：4 项通过；
- 全量测试：1,349 项通过；
- layout fixture golden：52 项通过，未更新 golden。

结论：**保留实现。**实现减少了 candidate 内部的常数成本，没有引入额外缓存。性能记录应限定在当前测试输入和函数路径内。后续如需继续优化，应使用 shaping 生成的 progressive opportunity、不同 token 长度和 Greedy / Lookahead / Paragraph DP 三类策略重新测量。

## 3. progressive tier boundary 查找

### 目标与现状

原实现先把 active span 内的 tier priority 收集到 `Vec`，再排序去重；随后针对每个 priority 反向扫描候选区间寻找最右 boundary。density 比较完成后，还需要额外寻找 Emergency boundary 和最终选择的 boundary。

### 实现

提交 `520e735` 在一次按 boundary 递增的扫描中，用固定的五槽数组记录 active span 内每个 tier 的最右 boundary：

```text
[Option<i32>; 5]
```

随后按既有 priority 顺序评估 density，直接返回选中的 priority 和 boundary。该实现移除了：

- priority `Vec` 的堆分配；
- 排序和去重；
- 每个 priority 的反向查找；
- 调用方对最终 boundary 的额外反向查找。

当前实现依赖 `ProgressiveBreakTier` 始终只有五个值，且 `priority()` 返回连续的 `0..4`。这是当前内部实现成立所需的维护约束。

### 性能与内存

专用基准使用 8,192 个 Latin cluster、5 个 tier 和最长反向查找路径：

| 版本 | median |
|---|---:|
| `Vec`、排序和反向查找 | 64.553 µs/call |
| 固定数组记录最右 boundary | 37.654 µs/call |

局部 median 改善约 41.7%。新实现把 tier 查找的临时空间从随 opportunity 数量增长的 $O(M)$ 降为固定 $O(1)$；这项优化同时减少了时间和临时内存。

该数字只代表长 technical span、五个 tier 和特定 opportunity 分布下的函数级收益，不能直接解释为完整页面或所有 technical layout 的提速比例。

### 验证与结论

- `progressive_break_decisions`：19 项通过；
- `progressive_break_decisions_tail`：2 项通过；
- `progressive_technical_break`：4 项通过；
- 全量测试：1,349 项通过；
- layout fixture golden：52 项通过，未更新 golden。

结论：**保留。**当前实现与既有选择语义一致，综合收益明确。后续新增 tier 时必须同步检查数组长度、priority 连续性以及 tier 选择顺序。

## 4. Lookahead gap boundary 前缀索引

### 目标与现状

Greedy 与 Lookahead 会使用 CJK、Sino-Western 以及合并 gap boundary 统计行区间内的可调整 gap 数。Greedy 的查询密度较低，而 Lookahead 会在候选评分、未来行评分和 committed density 计算中反复查询相同或相近区间。Paragraph DP 已有自己的前缀计数，本轮不重复计算 DP 的收益。

### 实现

提交 `6de2b57` 在 `LookaheadLineBreaker::break_lines()` 中构造私有 `GapPrefix`，用按 cluster index 对齐的 `Vec<i32>` 保存合并 gap boundary 的累计数量。之后 `score_candidate`、`badness` 和 committed density 使用前缀差值完成区间计数；fill PushIn 继续使用原有 `HashSet`。

Greedy、DP、公开 `LineBreakerConfig` 和 trait 签名均未改变。默认 Greedy 路径不构造该索引。

### 性能与内存

独立边界基准包含集合合并或 prefix 构造成本：

- 32 个 64-cluster 区间的 greedy-like 工作量：逐项查询 8.651 µs/call，gap prefix 13.617 µs/call；
- 128 个区间查询时接近持平：逐项查询 14.358 µs/call，gap prefix 13.465 µs/call；
- 256 个 Lookahead-like 区间查询：逐项查询 21.387 µs/call，gap prefix 13.371 µs/call；
- 三组 prefix 的构造成本约 30 µs/call。

直接运行默认 Lookahead 的 A/B/A/B 测量结果为：

| 版本 | median |
|---|---:|
| 逐项 `HashSet::contains` | 413.980 µs/call |
| `GapPrefix` | 194.965 µs/call |

在该长段落和高查询密度输入上，局部改善约 52.9%。

代价是增加一份按段落长度增长的 `Vec<i32>`：原始数据约为 $4(n+1)$ bytes，8192 个 cluster 约 32 KB，100,000 个 cluster 约 390 KB，另有少量 capacity 开销。当前实现没有把这笔临时内存保留到段落之外。

短段落或查询密度较低时，prefix 构造和分配成本可能超过查询节省。该优化适用于 Lookahead 重复区间统计足以覆盖构造成本的场景，其他场景仍需按输入长度测量。

### 验证与结论

- `greedy_line_breaker`：12 项通过；
- `lookahead_line_breaker`：13 项通过；
- `paragraph_dp_line_breaker`：7 项通过；
- 全量测试：1,349 项通过；
- layout fixture golden：52 项通过，未更新 golden。

Lookahead 完整 layout benchmark 的 median 为 2.349 ms、2.479 ms 和 2.298 ms。普通页面的候选评分密度低于专用输入，端到端数据只验证最终路径稳定。

结论：**保留。**该优化在 Lookahead 长段落中有明确局部收益，额外内存通常可接受。后续可以用不同段落长度测量摊销点，并考虑在低查询密度场景延迟或跳过 prefix 构造；当前不回滚实现。

## 5. UTF-8 scalar boundary 的 SIMD 实验

### 目标与范围

验证 `Text::index` 的 UTF-8 scalar boundary 构建是否适合使用 SIMD，同时保持现有 Unicode scalar source coordinate、子视图映射和布局输出不变。

本轮只处理有效 Rust `str` 中 UTF-8 continuation byte 的识别。x86_64 使用 SSE2 一次加载 16 个字节：

1. 对每个字节执行 `AND 0xC0`；
2. 与 `0x80` 并行比较；
3. 通过字节掩码输出非 continuation byte 的位置；
4. 尾部不足 16 字节的部分使用标量路径。

非 x86_64 目标继续使用 `char_indices()` 标量实现。输出仍为完整 UTF-8 存储上的 `u32` byte boundary 数组，顺序和末尾长度项保持不变。

### 局部结果

独立基准使用 Windows x86_64 release 工具链、20 轮预热、100 个样本和每样本 100 次索引构建：

| 输入 | 标量基线 | SSE2 | 变化 |
|---|---:|---:|---:|
| ASCII 256 | 174 ns | 125 ns | -28.2% |
| CJK 256 | 323 ns | 139 ns | -57.0% |
| 混合 256 | 149 ns | 100 ns | -32.9% |
| ASCII 4096 | 1639 ns | 1081 ns | -34.1% |
| CJK 4096 | 5046 ns | 1831 ns | -63.7% |

库级 release 汇编出现 `movdqu`、`pcmpeqb` 和 `pmovmskb`，确认实验性 x86_64 产物实际使用了 SSE2 字节向量指令。

### 端到端结果与结论

完整 layout benchmark 使用与基线相同的 20 轮预热、200 轮测量、宽度 672/360/960 和缩放 1。实验版本 layout median 为 2.293/2.364/2.265 ms，汇总为 2.311 ms；标量基线为 2.291/2.356/2.260 ms，汇总为 2.304 ms。对应变化为 +0.09%、+0.34%、+0.22% 和 +0.30%，total 汇总变化为 +0.53%。

这说明 SSE2 内核有局部收益，但没有证明完整布局的稳定端到端收益。原因包括索引构建的结果数组写入成本、短文本比例以及字体后端、断行、debug 输出和结果析构等其他成本。

结论：**不保留 SIMD 生产路径。**生产代码已回退到原有 `char_indices()` 实现；本节和相关 benchmark 仅保留为实验记录。回滚内容包括删除 `build_scalar_boundaries`、`build_scalar_boundaries_sse2` 以及本轮专用测试和生产 SIMD 路径。

### 历史提交复测

为确认前四次标量性能提交是否造成端到端退化，曾在优化前 `5cdc0a3` 和四个提交上运行完整 Rust 测试；五个版本均通过 1,349 项测试。

Greedy 完整布局使用相同参数进行多次测量：优化前版本五次汇总 layout median 为 2.284、2.294、2.318、2.309、2.367 ms；最终标量优化提交 `520e735` 的五次结果为 2.305、2.289、2.281、2.320、2.295 ms。两组结果区间重叠。交错运行时，同一版本的结果可相差约 50--70 µs，单次测量不足以判断千分之几到百分之几的变化来自某个提交。

`6de2b57` 只影响 Lookahead 路径，默认 Greedy 基准不能评价它。使用 8,192 个 cluster、64 px 行宽、20 轮预热、200 个样本和每样本 100 次调用，父提交 `aa6d3c8` 的三次 median 为 436.750、429.602、428.338 µs/call；优化提交的三次 median 为 197.171、197.384、196.399 µs/call，约快 54%。该结果支持它在目标 Lookahead 路径上的局部收益，但不代表默认 Greedy 完整布局会出现相同幅度的变化。

## 统一取舍与后续工作

### 当前保留项

1. 保留 `74cded2` 的 hyphen 扫描合并；
2. 保留 `aa6d3c8` 的 progressive density 扫描合并；
3. 保留 `6de2b57` 的 Lookahead gap prefix，同时关注短段落的构造摊销和长段落的临时内存；
4. 保留 `520e735` 的固定数组 tier lookup；
5. 不保留 SIMD 生产实现，仅保留实验数据和回退记录。

### 证据边界

本轮 benchmark 主要用于比较局部实现，完整工作负载仍需单独评估。当前证据的主要限制是：

- 若干输入使用 8,192 个 cluster，远长于常见段落或 technical token；
- boundary 密度和 opportunity 分布部分由 benchmark 人工构造；
- 专用 benchmark 不包含完整 shaping、字体度量和布局结果处理；
- A/B 数据不能消除 CPU 调度、频率变化和运行批次漂移；
- 普通 layout benchmark 主要覆盖 Greedy，对 Lookahead 和 Paragraph DP 的覆盖有限。

因此，文档中的百分比仅表示指定 fixture 上的 median 变化，不能直接作为所有页面的预期收益。

### 后续验证

后续如继续做性能调整，优先补充：

1. 使用 shaping 结果生成 boundary 和 progressive opportunity；
2. 覆盖 16、32、64、128、512、2048、8192 等多种输入长度；
3. 分别测量 Greedy、Lookahead 和 Paragraph DP；
4. 增加新旧实现的随机 differential test，验证断行选择逐项一致；
5. 测量 `GapPrefix` 的查询摊销点与最大临时内存；
6. 将当前极长输入明确标注为扫描压力测试。

## 回滚

- 回滚 hyphen 扫描合并：恢复 `decide_hyphen_break` 中原有的独立扫描；
- 回滚 progressive density 合并：恢复 `progressive_candidate_stretch_density` 中的多次区间扫描；
- 回滚 tier lookup：恢复 priority `Vec`、排序、去重和反向查找；
- 回滚 gap prefix：删除 `GapPrefix`，恢复 Lookahead 中的 `HashSet` 区间统计；
- 回滚 SIMD 实验：删除 SIMD boundary 构建函数、专用测试和实验 benchmark，恢复 `Text::index` 的 `char_indices()` 实现。
