# 2026-09-14 连字符断行边界查询性能

## 目标

测量并优化 `decide_hyphen_break` 在长技术词上的重复区间扫描和边界集合查询，同时保持断行结果与 layout dump 不变。

## 范围与设计

`decide_hyphen_break` 在确认候选为连字符断点后，会在同一个 `[line_start, whole_word_end)` 范围内依次计算宽度、Sino-Western gap 数与 CJK gap 数。当前每个 gap 统计都逐项查询 `HashSet`。

本轮先添加独立 release example。输入使用一个长的 Latin cluster 序列、一个末端连字符断点和稠密 CJK boundary，直接调用 `decide_hyphen_break`。构造输入不计入测量；输出包含每次调用的 min、median、p95、mean、max。

在获得基线后，再评估是否为断行器构造按 cluster index 对齐的边界前缀。实现必须保留现有 `HashSet` 公开参数与逐项浮点宽度累加顺序，避免改变断行阈值语义。

## 验证

1. 在 release 下记录专用基准基线；
2. 运行相关 hyphen 与 progressive 断行测试；
3. 运行 layout fixture golden，禁止更新 golden；
4. 对候选实现执行交替 A/B 测量；
5. 检查编辑器诊断、`git diff --check` 和本文档风格。

## 基线

Windows x86_64 release 下，`hyphen-break-bench` 使用 8,192 个 Latin cluster、20 轮预热、
200 个样本、每样本 100 次调用。构造输入不计入测量。

| 场景 | median | p95 |
| --- | ---: | ---: |
| Sino-Western capacity 填满 deficit | 11.090 µs/call | 11.473 µs/call |
| CJK gap 填充剩余 deficit | 19.364 µs/call | 19.654 µs/call |

## 候选改动

`decide_hyphen_break` 使用一次按 index 递增的循环同时计算 word width 与 Sino-Western gap 数。
width 仍按原始 cluster 顺序逐项以 `f32` 累加；CJK gap 的统计仍仅在 Sino-Western capacity 未填满
deficit 时执行。下一步使用同一基准与完整断行回归验证此改动。

## 验证结果

专用基准使用 A/B/A/B 交替测量。第二个 A 与 B 的 median 如下：

| 场景 | A：两次扫描 | B：合并扫描 | 变化 |
| --- | ---: | ---: | ---: |
| Sino-Western capacity 填满 deficit | 10.680 µs/call | 9.141 µs/call | -14.4% |
| CJK gap 填充剩余 deficit | 19.125 µs/call | 17.736 µs/call | -7.3% |

第一次 B 测量的 median 为 9.082 µs/call 与 17.104 µs/call，方向与第二次 B 一致。
专用基准的收益来自少一次完整区间遍历；CJK 统计路径保留原有扫描。

语义验证结果：

1. `cargo test --test tiqian progressive_break_decisions`：19 项通过；
2. `cargo test --test tiqian decide_hyphen_break`：2 项通过；
3. `cargo test`：1,349 项通过；
4. `cargo test --test tiqian layout_fixture_golden_test`：52 项 fixture 通过，未更新 golden；
5. 编辑器诊断无错误。

端到端基准使用 20 轮预热、200 轮测量、宽度 672/360/960、缩放 1。layout median 为
2.277 ms、2.353 ms 和 2.256 ms；相对本轮开始前的 2.291 ms、2.356 ms 和 2.260 ms，变化为
-0.61%、-0.13% 和 -0.18%。普通 demo 页面没有足够的长连字符候选区间，端到端变化不能单独作为
本轮保留依据；保留依据是专用基准中可重复的局部收益，以及完整布局回归通过。
