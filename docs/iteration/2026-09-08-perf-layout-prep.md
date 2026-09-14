# 2026-09-08 排版 preparation 性能优化

## 目标与证据

优化 `src` 排版核心，保持布局结果和完整 decision dump 不变。
基线为用户提供的 `flamegraph.svg`（2026-09-08 02:19:38）。
SVG 共记录 82,184 个根样本；按同名函数聚合的包含子调用样本如下：

| 调用 | 样本 | 占排版入口 |
| --- | ---: | ---: |
| 排版入口 `layout` | 37,580 | 100% |
| `build_paragraph_layout_prep` | 19,679 | 52.4% |
| `ParagraphShapingStageResult::clone` | 7,827 | 20.8% |
| `finish_paragraph_layout` | 10,460 | 27.8% |
| `plan_paragraph_lines` | 5,495 | 14.6% |

父子调用的样本有重叠，不能相加，也不能据此承诺实际加速比例。
用户确认采样命令为 `cargo flamegraph --example paragraph-demo`，默认 release 构建。
原始图已固定保存为 `flamegraph-baseline.svg`，后续不修改。

## 范围与设计

`build_paragraph_layout_prep` 在不需要动态 shaping 时，借用
`annotation.base_shaping_stage`，省去整份 shaping results 和 segment cache 的复制。
需要动态 shaping 时，本次生成的结果由局部变量持有。
最终 preparation 需要独立拥有的元数据按字段克隆；动态路径也采用同样方式。
平铺 cluster 时直接克隆迭代器元素，避免逐个 shaping result 分配临时向量。

公开 API、排版规则和缓存身份保持一致，无数据迁移需求。
本轮不改变上游算法取舍，无需新增 ADR 或修改同步终点。

## 验证与回滚

- 运行完整 `cargo test`，包含本地 layout fixture golden，禁止更新 golden。
- 检查编辑器诊断、目标 diff 和文档格式。
- 用户使用相同构建参数和操作重采样，比较 preparation、clone 栈及绝对排版耗时。
- 如出现布局差异或性能退化，撤回本轮缓存借用与迭代器改动。

## 实施状态

已实施缓存借用与 cluster/decision 平铺迭代器优化。
`cargo test --quiet`：18 项单元测试、1 项独立集成测试、1,338 项主集成测试全部通过，
包含 layout fixture golden；未更新 golden。修改文件的编辑器诊断无错误。
测试仅报告既有未使用字段警告。

## 第一轮复测与第二轮优化

新图根样本为 77,243，排版入口为 30,755；preparation 为 12,168，
占排版入口从 52.4% 降至 39.6%。`ParagraphShapingStageResult::clone` 已无样本。
finish 阶段为 11,082，planning 为 5,440，debug assembly 为 4,351。
两次操作量未固定，样本数下降不能直接换算为耗时加速。

第二轮将 `auto_space.decisions` 直接移动到输出向量，再追加 verbatim decisions。
原代码先 clone 再 concat，导致自动间距 decision 重复复制。
保持 decision 顺序与内容一致，继续通过完整测试与 golden 验证。
第二轮 `cargo test --quiet` 全部 1,357 项测试通过，golden 无变化；
编辑器无错误，`git diff --check` 与文档风格检查通过。实际收益等待后续采样。

## 第二轮复测与第三轮优化

最新图根样本为 43,263，布局入口为 16,979；preparation 为 6,599（38.9%），
finish 为 6,046（35.6%），planning 为 3,062（18.0%）。
preparation 占比相较上轮变化较小，尚不能区分优化收益与采样波动。
`PunctuationGeometryLedger::clone` 为 1,484 个包含子调用样本，占布局入口 8.7%。

每次几何变换均复制创建后不再修改的 natural clusters 与 geometries。
第三轮将这两个私有字段分别改为 `Arc<Vec<Cluster>>` 与 `Arc<HashMap<...>>`，
构造时移动原有容器，后续 clone 共享存储。预算和各项调整仍独立复制，
所有计算、迭代顺序及公开接口保持不变。
验证覆盖共享存储与派生对象预算独立性，并运行完整测试及 golden。
回滚时恢复两个字段的原有容器类型与构造赋值即可。

第三轮全部 1,358 项测试通过，golden 未更新，修改文件无编辑器错误。
下一轮采样重点检查 ledger clone 下的 cluster 和基础几何深复制是否消失。

## 第三轮复测与第四轮优化

最新图根样本为 40,655，布局入口为 14,767；preparation 为 5,770（39.1%），
finish 为 4,936（33.4%），planning 为 2,965（20.1%）。
`PunctuationGeometryLedger::clone` 聚合样本降至 41（0.28%），上一轮为 8.7%。
操作量未固定，以上是样本分布变化，不能作为绝对耗时加速比例。

按 SVG 精确样本区间恢复调用祖先，最大的输入 clone 分支位于 finish，
为 2,189 个样本，其中 Text clone 子调用为 2,165。源码对应最终构造结果时的
`prep.input.clone()`。

第四轮让 `LineAdjustmentRequest.prep` 持有 `ParagraphLayoutPrep`，成功时将输入
移动到 `LayoutResult`。唯一生产调用方在规划结束后移交 prep；重试继续使用引擎持有的
原始输入。stage 请求字段类型变化，主排版 API 与计算顺序保持一致。
回滚时恢复借用参数及最终输入 clone。验证继续覆盖全部测试和 fixture golden。

第四轮全部 1,358 项测试通过，golden 未更新，修改文件无编辑器错误；
diff 与文档风格检查通过。下一轮检查 finish 返回结果时的输入 clone 分支。

## 第四轮复测

最新图根样本为 48,093，布局入口为 17,620；preparation 为 6,990（39.7%），
finish 为 6,371（36.2%），planning 为 3,476（19.7%）。
按精确函数名称与 SVG 样本区间检查，finish 下的输入 clone 为 0 样本。
finish 的直接析构子调用合计 598 个样本；prep 改为拥有值后，其析构归入 finish，
阶段占比不可直接用于推算本轮加速。未对比上轮析构树，尚不能确定析构解释了多少占比变化。

debug assembly 为 2,797 个样本，占布局入口 15.9%；
preparation 内 shaping decision 平铺分支为 1,071 个样本。
后续优先检查 decision 从 preparation 到最终 debug 输出的所有权转移。
本轮仅记录复测，未新增代码改动；总体耗时收益仍需固定工作量的计时证据。

## 第五轮：debug decision 向量转移

`LayoutDebugStageInput` 由 finish 唯一构造。本轮将 shaping、geometry、edge trim、
decoration decisions、decoration segments、ruby、bopomofo 与 contextual kinsoku
八个向量改为按值传入，最终 builder 直接接收原有分配，去除对应 `to_vec()`。
shaping decisions 在 finish 完成其他计算后从拥有的 prep 移出；其余向量从局部结果移出。
仍需计算的字体、度量、标点等输入继续借用。

stage 请求的八个字段由 `&[T]` 改为 `Vec<T>`，主排版 API 与完整 debug 内容、顺序不变。
缓存到 preparation 的 shaping decision 复制继续保留。
以完整测试和 fixture golden 验证输出等价；回滚恢复 `&[T]` 参数、借用和 `to_vec()`。

第五轮全部 1,358 项测试通过，golden 未更新。
下一轮重点检查 debug assembly 下八类 decision 的复制调用；实际耗时收益待采样。

## 第五轮复测

最新图根样本 40,762，布局入口 14,881；preparation 6,065（40.8%），
finish 4,988（33.5%），planning 3,135（21.1%）。debug assembly 为 1,958（13.2%），
上一轮为 15.9%。其直接 `to_vec` 子调用合计 13 个样本，
主要直接子调用已是 decision 映射生成（1,705 个样本）。
占比变化支持继续保留向量转移，绝对耗时收益仍需固定工作量测量。
本轮只更新采样记录，按用户要求不重复执行测试或 diff 检查。

## 第六轮：东亚间距解析

preparation 下 `resolved_edges` 为 907 个样本，其中交互边界计算为 660 个样本。
单 scalar 文本直接复用 `resolved_for_grapheme_cluster`，省去交互分段与临时向量。
多 scalar 继续使用现有交互边界，属性通过迭代累计首项、末项和 contains_wide，
不再收集属性向量。空文本、语言条件属性和 enclosing mark 规则保持一致。

补充单 scalar 及多字素首尾与中间 Wide 的用例，运行相关测试和 fixture golden。
回滚恢复 `resolved_edges` 原有分段与属性向量实现即可，公开 API 不变。

12 项东亚间距测试与 fixture golden 通过，未更新 golden，编辑器无错误。
新增测试最初误将独立 U+0301 预期写为 Other，按固定属性表修正为 Narrow；
enclosing mark U+20DD 仍为 Other。下一轮检查单 scalar 路径省去分段后的样本变化。

## 第六轮复测

最新图根样本 55,448，布局入口 19,808；preparation 7,687（38.8%），
finish 6,880（34.7%），planning 4,348（22.0%），debug assembly 2,547（12.9%）。
preparation 下 `resolved_edges` 为 593 个样本，占布局入口 2.99%，上轮为 6.10%；
其交互边界子调用为 181 个样本，占布局入口 0.91%，上轮为 4.44%。
目标样本占比明显减少，支持保留单 scalar 快速路径与属性累计优化。
采样工作量不同，以上不代表整体耗时加速比例。本轮未新增代码或重复运行测试。

## 第七轮：planning 字体度量请求

planning 的 `closure$0` 为 1,913 个样本，对应逐个字体 decision 的度量请求构造与归一化。
直接子调用包含请求 clone 385 个样本、builder 249 个样本和 face_selection_text setter
309 个样本。builder 默认构造空 Text，随后 setter 替换它。

直接构造完整请求，省去空 Text；将请求移动到局部归一化输入，调用后再移入 decision，
避免临时深克隆。保留独立字号回调、resolver 与 normalizer 调用顺序和全部字段值。
公开接口不变。以 fixture golden 验证完整度量输出；回滚恢复 builder 与 clone 即可。

fixture golden 通过，未更新预期，修改文件无编辑器错误。
下一轮检查 planning 度量闭包下的请求 clone、builder 和文本 setter 样本变化。

## 无界面布局基准

新增 `paragraph-layout-bench` example，复用 desktop sample 与 HarfRust 字体后端。
固定宽度序列，单个引擎依次执行首次序列、预热序列和测量序列。
每次构造完整 sample，保留列表标记测量与正文宽度计算，跳过 Section 留白。
每次 `engine.layout` 独立计时并累计为整页核心布局时间；输入构造、列表宽度计算、
结果统计与结果析构在核心计时之外。结果集中保留到整页结束后统一析构并单独计时。
输出首次序列、预热后各宽度与全部页面的 min、median、p95、mean、max，
以及布局调用数、行数、cluster 数和 glyph 数，便于确认工作量。
首次序列仅表示当前进程的引擎尚未预热，不代表操作系统字体数据冷缓存。

字体加载在所有页面计时之外。共享字体后端仍含 desktop 的绘制方法与字体资源，
基准入口不调用绘制方法，不初始化窗口或 GPU。Cargo 继续使用现有 dev-dependencies。
默认 20 轮预热、200 轮测量、宽度 672/360/960、缩放 1；可通过命令行覆盖。
公开 API 与核心算法不变，无需更新上游差异或 ADR。
验证直接运行 release example、参数错误路径与编辑器诊断；回滚删除新入口及使用说明。

默认 release 运行通过：600 个测量页面，每页 47 次布局，总计 28,200 次。
672/360/960 宽度的核心 median 分别为 2.546/2.612/2.488 ms，
p95 分别为 2.755/2.800/2.644 ms；首次 672 宽度为 40.497 ms。
这组结果作为新工具的初始记录，与 GUI demo 计时范围不同，不用于宣称额外加速。
自定义单宽度 480、缩放 1.25、零预热运行通过；帮助、零测量次数和 NaN 宽度
路径验证通过，非法参数按预期非零退出。新入口编辑器无错误，未重复运行核心测试。

## 第八轮：glyph run 分组借用

用户无采样基准的 672/360/960 核心 median 为 2.496/2.565/2.468 ms，
p95 为 2.605/2.680/2.593 ms。新图根样本 71,979，布局入口 67,273，
`build_glyph_runs` 为 4,350 个包含子调用样本，占布局入口约 6.5%。
分组函数仅供 glyph run 构造读取，却克隆每个可见 cluster。
将 `renderable_glyph_run_clusters` 返回值由 `Vec<Vec<Cluster>>` 改为
`Vec<Vec<&Cluster>>`，生命周期绑定输入 clusters；过滤条件、分组边界与输出顺序不变。
该辅助函数的返回类型改变，主布局 API 不变。验证 fixture golden 和固定工作量基准，
回滚恢复拥有值的分组与 clone。无新增布局规则，无需更新上游差异或 ADR。

fixture golden 通过，未更新预期，编辑器无错误。默认 release 基准的三个核心 median
为 2.490/2.564/2.470 ms，p95 为 2.652/2.753/2.636 ms，输出数量保持一致。
相对用户改前数据，median 基本持平，尾部耗时略高；单次对比尚无明确加速证据。
保留分组借用以消除不必要的复制，不将该结果描述为已获得端到端性能提升。

## 第九轮：debug 枚举名称输出

同一无界面火焰图中，debug assembly 为 8,066 个样本，占布局入口约 12%。
font 输出闭包为 3,032，metric 输出闭包为 3,065；后者直接 format 子调用为 2,985。
metric 每条记录格式化 role、raw source、baseline class、metric box、layout source。
在 debug assembly 内穷举这些无数据枚举的名称，直接构造同名 String，省去通用格式化。
font 输出与 role override 使用同一 role 名称函数；所有字段类型和文本保持一致。
不改变 debug 开关、布局规则或公开 API。以 fixture golden 和固定参数基准验证；
回滚恢复对应 `format!` 并删除名称函数即可。

fixture golden 通过，完整 dump 未变化，编辑器无错误。
默认 release 基准的三个核心 median 为 2.446/2.509/2.396 ms，
p95 为 2.644/2.692/2.555 ms，输出数量一致。
相对上一轮 median 下降约 1.8%/2.1%/3.0%，属于单轮改善迹象，尚未经交替 A/B 验证。
debug assembly 的约 12% 不代表全部 decision 成本：其他阶段仍生成和消费 decision，
不能据此估算关闭所有解释数据的收益，也不能把参与几何计算的 decision 当作可删除日志。