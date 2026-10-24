# 2026-10-24 字符串状态与 debug 分配的布局开销收敛

## 目标

减少布局管线中由 `String` 承载机器状态、以及按记录堆分配固定标签造成的开销。
输入为 `docs/tmp/string-cost-report.md` 的静态审计；本轮逐条核对源码后重新排序，并补充审计未覆盖的三处热路径。

验收指标按顺序为：

1. 布局时间（`paragraph-layout-bench` 的 `layout`）；
2. 每次布局的分配次数与累计字节；
3. 跨宽度重复布局时缓存的保留内存。

布局行为、断行选择、几何数值与 fixture golden 文本必须保持不变。
本迭代主分类为 `perf`：公开 debug 结构体的字段类型会调整，但那是消除字符串分配的实现手段，
不新增布局能力，也不改变任何 decision 的语义。

golden 零 diff 只说明 dump 文本未变，不足以保证行为未变：
`tests/fixture_layout/dump.rs` 不输出行 notes，且 `fmt()` 把浮点值截到一位小数。
因此每批次还要比对结构化差分：行与 cluster 范围、repair 选择、glyph 与 feature、
以及关键几何的原值；涉及缓存的改动另比对 backend 调用次数。

## 测量基线（Phase 0，必须先做）

现有 `examples/paragraph-layout-bench.rs` 只输出时间，没有分配数据，无法确定后续各项的收益。
该 benchmark 以 `mimalloc::MiMalloc` 作为全局分配器，而 `examples/white-space-bench.rs`
的计数分配器只包装 `System`，只能当作写法起点：需要改为包装 mimalloc，并补齐该实现缺少的部分。

### 测量范围一致

`measure_page` 中的 marker 页（`ListItem` 的 marker 测量）当前计入 `layout` 时间与 `calls`，
只是被排除于 replay 与 cluster query。新增的分配计数要么同样默认计入，
要么把时间、分配、`calls`、decision 记录数一起排除。两者择一，不能只排除其中一部分。

### 计数范围

计数只覆盖 `engine.layout()` 调用，不含输入准备、结果遍历、统计打印、replay 构造与结果析构。
每次分配都带原子操作，会放大「减少分配次数」的耗时收益，因此计时与计数分两次运行：
计时运行不启用计数，计数运行不采信时间。

### 新增输出

| 项 | 内容 |
| --- | --- |
| `alloc_calls` / `realloc_calls` | 分开记录，便于区分新建与扩容 |
| `bytes` | 累计请求字节；realloc 按请求的新尺寸计入，不代表同等大小的新增物理内存 |
| live requested bytes | 结果存活时记录一次，删除结果后再记录一次，用于观测引擎保留内存 |
| decision 记录数 | 不只六类：还含 metric、punctuation、嵌套 allocations 以及本轮涉及的其他集合 |

### 缓存路径的三个维度

重建引擎仍保留若干热状态：catalog、backend、系统字体缓存、allocator、CPU cache。
计划区分三个可分别开关的维度：

1. 默认：共享引擎、多宽度复用；
2. annotation-cold：只清空 annotation cache。该 trait 已有 `clear`，benchmark 可注入统计包装器
   记录命中与清空，不需要为此新增生产 API；
3. fresh-engine：重建引擎，字体装载不计入计时。

另需单独观测 `needs_dynamic`：annotation 命中与动态 shaping 是两个维度。
缓存命中后，窄宽度和 technical span 仍可能进入 `shape_paragraph()`。

### 分配器包装的注意事项

- 转发 `alloc`、`dealloc`、`realloc`，并显式转发 `alloc_zeroed`；不覆盖时 trait 默认实现仍可用，
  但会绕开底层的原生零填充分配路径。
- live bytes 只在 realloc 成功时用新尺寸替换旧尺寸；失败时旧分配仍有效。
- 分配器回调内不能格式化、打印或使用会分配的统计容器。
- Rust 全局分配器不覆盖字体库等原生代码的直接分配，读数不能当作进程总内存。
- 包装器按 `GlobalAlloc` 的要求实现，不依赖 mimalloc 特定版本的 realloc 内部行为。

### 基线表格

2026-10-24 实测。Windows x86_64 release，`profile=release arch=x86_64`，`strategy=greedy`，
宽度 672/360/960，缩放 1。计时运行 `--warmup 20 --iterations 200`；
计数运行 `--count-allocations --warmup 20 --iterations 100`；冷路径 `--widths 672 --warmup 5 --iterations 20`。
两轮运行分开，计数运行的时间不作比较。

每页 47 次布局调用、2057 个 cluster、82/119/69 个可见行，整页 7395/7345/7186 条 debug 记录。

计时运行（ms）：

| 宽度 | min | median | p95 | mean | max |
| --- | ---: | ---: | ---: | ---: | ---: |
| 672 | 2.683 | 2.770 | 3.023 | 2.829 | 4.305 |
| 360 | 2.579 | 2.656 | 3.288 | 2.741 | 4.244 |
| 960 | 2.465 | 2.550 | 2.873 | 2.606 | 3.898 |
| 全部 | 2.465 | 2.610 | 3.035 | 2.661 | 4.244 |

计数运行（每页，中位数）：

| 宽度 | layout median | alloc_calls | realloc_calls | bytes | window live | 记录数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 672 | 3.639 | 76,451 | 2,331 | 11,287,705 | 2,958,656 | 7,395 |
| 360 | 3.761 | 78,256 | 2,588 | 11,396,860 | 2,994,937 | 7,345 |
| 960 | 3.566 | 75,440 | 2,234 | 11,182,980 | 2,931,634 | 7,186 |
| 全部 | 3.658 | 76,451 | 2,331 | 11,287,705 | 2,958,656 | 7,345 |

冷路径（宽度 672）：

| 维度 | layout median | alloc_calls | realloc_calls | bytes | window live | 记录数 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 默认（共享引擎） | 3.639 | 76,451 | 2,331 | 11,287,705 | 2,958,656 | 7,395 |
| annotation-cold | 41.666 | 337,283 | 50,870 | 277,056,792 | 9,286,747 | 7,395 |
| fresh-engine（计时运行） | 39.431 | 未测 | 未测 | 未测 | 未测 | 未测 |

关键结论：

1. **annotation-cold 是默认路径的 11.4 倍耗时、4.4 倍分配次数、24.5 倍累计字节。**
   宽度无关缓存对重复布局的贡献远大于本迭代各项的预期收益。
2. **fresh-engine 与 annotation-cold 耗时接近（39.4 对 41.7 ms）**，说明引擎重建本身的成本
   相对于冷 shaping 很小。字体装载已在计时之外。
3. **每页 7,345 条 debug 记录、每次调用 155.5 条**，配合 76,451 次分配，
   平均每条记录对应约 10 次分配。这是第 5、6 项的目标区间，但其中也包含 glyph 与 cluster 相关分配。
4. **每页累计请求 11.3 MB、窗口存活约 2.96 MB**，说明大部分中间分配在页内即被释放。
   所有权收敛（第 10 项）应关注窗口存活字节，累计字节不能反映保留量。
5. **计数模式的 `layout` 比计时模式高约 1.05 ms（3.66 对 2.61）**，这是每次分配两个原子操作的成本。
   该差值本身可作为分配次数影响量级的下限参考，但不可直接换算收益。

`process_live` 为全进程存活字节绝对值（约 94 MB），由字体与 backend 占用主导，
只用于比较两个时点之间的差值，不作为本迭代指标。

三个维度各自对应一条路径：审计第 9 项关联 annotation 复用，第 10 项关联动态 shaping，
第 11 项只在开启 `--replay` 时才有意义。

## 范围与现状依据

### 审计已覆盖且核实成立的部分

| 编号 | 现状 | 位置 |
| --- | --- | --- |
| 1 | `RepairOption` 已是 enum，但每个变体仍带 `reason: String`；`line_repair` 用 `starts_with("LineAdjustmentPushIn:")` 区分推入来源，debug 汇编用 `split(':').next()` 反解析 reason code | `src/layout/line_optimization.rs:19`、`:194`；`src/layout/line_repair.rs:480`、`:620`；`src/layout/layout_debug_assembly.rs:366` |
| 2 | progressive technical 的 emergency 去重为比较 reason 文本，先收集 tier、排序、逐个 `format!("{:?}")`、`join("+")` 拼串；插入 decision 时重复同样的构造 | `src/layout/paragraph_shaping_stage.rs:379-426` |
| 3 | justifier 的 stage 用 `String` 作身份，且该字符串参与「容量设为无穷」的数值规划；allocation 逐条拥有 reason | `src/layout/justifier.rs:78`、`:232-251`、`:829`、`:861`、`:874` |
| 4 | `AutoSpaceDecisionInfo` 的 `side`/`boundary_role`/`mode` 为字符串，且被计宽、注释几何、shrink 资源按字符串分派 | `src/core/layout_model.rs:1476`；`src/layout/line_measurement.rs:70`；`src/core/layout_queries.rs:1699`；`src/layout/annotation_geometry_stage.rs:161`、`:167`；`src/layout/width_independent_annotation_cache.rs:1188` |
| 5 | debug model 把已有 enum 重新格式化成 `String` | `src/layout/layout_debug_assembly.rs:103`、`:124`、`:129-145`、`:156`、`:161`、`:274`；`src/layout/line_break_planning_stage.rs:322-323`；`src/layout/width_independent_annotation_cache.rs:362`、`:378-406`、`:627`；`src/layout/line_geometry_stage.rs:268`；`src/layout/punctuation_geometry_stage.rs:617` |
| 6 | 行级 notes 与结构化字段重复，且存在两份并行 notes（`LineBox.debug` 与 `LineDecisionInfo.notes`），格式已经漂移 | `src/layout/line_geometry_stage.rs:137-162`、`:183`；`src/layout/layout_debug_assembly.rs:205-232` |
| 7 | 标点几何来源把 `FixedHalfWidth` 拼进来源名 | `src/layout/punctuation_model.rs:453-456`；`src/layout/punctuation_geometry_ledger.rs:361` |
| 8 | 替换与度量在产出点立即格式化文本 | `src/clreq/clreq_profile.rs:878-889`、`:930-932`；`src/font/font_metrics.rs:71`、`:91`、`:105` |
| 9 | annotation 同时保存三份 shaping 缓存与包含它们的 `base_shaping_stage` | `src/layout/width_independent_annotation_cache.rs:113-118`、`:570-572` |
| 10 | 动态 shaping 先整份 clone 基础缓存，命中再 clone 结果 | `src/layout/paragraph_shaping_stage.rs:61-62`、`:158-160` |
| 11 | replay index 把 role 存成 `Option<String>`、feature 复制成独立 `Vec<String>` | `src/core/layout_result_replay_index.rs:23-24` |

### 审计未覆盖、本次补充的部分

**A. `substitute()` 在 shaping 热路径无条件构造 reason 后丢弃。**
`shape_paragraph` 只使用 `substitution.display_text`，从不读 `reason`；
非 CJK 标点角色还会先跑一遍完整 `substitute()` 造出用不上的串，再拼第二个
`CjkRoleGatedDisplaySubstitution:...`。这是纯浪费，且不涉及任何公开结构。

**B. 常量标签按记录 `to_owned()`。**
`ClusterGeometryDecisionInfo.source` 固定为 `"PunctuationGeometryLedger"`，
`FontDecisionInfo.reason` 固定为 `"FontBackendCompleteShapingSelection"`，
`LineEdgeTrimDecisionInfo` 的多条 reason 亦为常量。这些字段应为 `&'static str`（或枚举），
不应当每条记录各一次堆分配。

**C. emergency tracking 的 reason 按行内边界放大。**
`line_break_planning_stage` 把 eligibility decision 的 reason 逐左边界 clone 进
`HashMap<i32, String>`，justifier 随后又对每个机会 `format!("EmergencyGraphemeTracking:{reason}")`。

**D. `EmergencyTrackingEligibilityDecisionInfo.reason` 的值域比审计描述更宽。**
`strong_non_lexical_reason()` 还会产出 `LongRepeatedLetterRun`、`LongHexIdentityRun`、
`LongMixedAlphaNumericIdentifier`（`paragraph_shaping_stage.rs:995-1021`），
并在 `:512` 处写入同一个字段。这三个值来自代码内的有限集合，
但如果不纳入同一类型，结构化到一半就必须重新转回字符串。

### 审计中判断需要修正的部分

- 审计建议「把规范化推迟到输出边界」；但当前没有任何 debug 开关，
  `LayoutResult.debug` 在每次布局都完整构造（`paragraph-layout-bench` 的提示文本亦如此声明）。
  因此本轮只能通过**换类型**降低单条记录成本，不承诺「不构造」。
- 审计第 5 项的部分内容已在 2026-09-08 第九轮处理过：`font_role_name`、`font_metric_source_name`
  已改为穷举名称函数。剩余未处理的是 `punctuation_class`、`anchor`、`GlueKind`、role override、
  kinsoku、body alignment、ruby mode、outer spacing 等。

## 公开 API 与消费方核实

`huozi` 通过 `[patch.crates-io]` 使用本仓库路径，其代码只读取四个 debug 集合：
`decoration_decisions`、`ruby_decisions`、`bopomofo_decisions`、`inline_object_decisions`
（`huozi-rs/src/layout/tiqian_output.rs:429`、`:538`、`:575`、`:684`），
读取的字段均不在本轮变更范围内。

仓库内对以下结构的读取同样为零，只有写入：

- `LineBox.debug`（`LineDebugInfo`）：全仓库唯一写入点是 `line_geometry_stage.rs:183`，没有任何读取点；
- `LineDecisionInfo` 的消费者只有 fixture dump 与测试。

结论：本轮的字段类型变更不会要求 `huozi` 修改代码。
但 `huozi` 的测试会随本仓库一起编译，实施后需在 `huozi-rs` 中跑一次 `cargo check` 与 `cargo test` 确认。

## 设计与实施顺序

按批次推进，每批完成并验证后再评估下一批的投入。各项目标收益差异很大，
不应把下表当成必然全部完成的清单。

| 批次 | 内容 | 状态 |
| --- | --- | --- |
| P0 | 测量基建（见上） | 已完成 |
| P1 | A 项；第 6 项常量标签 | 已完成，每页 -8 次分配 |
| P2 | 第 5、4、2、3 项 | 已完成，每页 -16,332 次分配 |
| P3 | 第 7、8、9 项 | 已完成，每页再 -2,146 次 |
| P4 | 第 10.1、10.2、10.3；第 11 项 | 已完成，热路径累计 -26.2% |

排序依据是「记录条数 × 字段数」：每页 `font_decisions` 与 `metric_decisions` 各约 2,100 条，
`geometry_decisions` 168 条，而 P1 处理的 `InlineBoxDecisionInfo` 每页仅 3 条。
改动的难易程度与收益无关。

第 2 与第 3 项共用 `EmergencyTrackingReason`，先做第 2 项。
第 5 与第 11 项共用 `FontRole`，先做第 5 项。

### 类型选择原则

按职责选，不统一套一种容器：

| 场景 | 做法 |
| --- | --- |
| 影响算法分派的有限状态 | enum |
| 已有 enum 的诊断投影 | 直接复用原 enum |
| 仅用于显示、不参与算法的内置常量 | `&'static str` |
| 确有调用方或 backend 自定义文本的显示字段 | `Cow<'static, str>` |
| 原因码与额外说明各自独立且都有意义 | 原因 enum + 可选扩展文本 |

不默认使用「enum + `Option<Box<str>>`」。没有实际扩展需求时，它增加状态组合、记录尺寸与 API 负担。
若自定义原因只是内置原因的替代而非补充，`Custom(Box<str>)` 变体比「任意原因 + 可选文本」明确。
`Cow` 的 Owned 分支 clone 仍会复制文本，不自动解决共享问题。

### P1-A 项：shaping 热路径不再构造无用的替换 reason

范围严格限定为：

- 新增只返回显示文本的入口，`shape_paragraph` 改用它，不生成 reason；
- 既有带 reason 的接口行为保持不变；
- 两条入口共用同一套替换规则，不能各写一份。

不包含「debug 复用实际替换结果」（属第 9 项剩余部分），也不涉及缓存结构。
把它当作测量系统的验证：预期分配下降，布局与 dump 不变；耗时是否明显改善以实测为准。
如果分配计数下降而耗时不变，说明该路径不是瓶颈，后续优先度据此调整。

### 第 1 项：repair 机器状态结构化

1. `RepairOption` 的 `reason: String` 改为 `RepairReason` 枚举，参数只保留算法实际需要的量
   （offender 文本由 cluster index 查得，`shrink`/`capacity` 已是结构体字段，不重复进 reason）。
   需覆盖的现有分支：`LineAdjustmentPushIn`、`ProgressiveTechnicalTierPromotion`、
   `LineAdjustmentPushIn:DPFeasibility`、`ForbiddenAtLineStart`、`ForbiddenAtLineEnd`；
   其中后两者还带方向或 cause 参数。
2. 删除 `line_repair.rs:480`、`:620` 的前缀判断：`zero`/`continues` 直接匹配枚举变体。
3. `RepairCandidate.kind` → `RepairKind`，`reason_code` → `RepairReasonCode`，
   `rejection_reason` → `Option<RepairRejection>`（值域：`InsufficientCapacity`、`NoRoomToCarry`、
   `CarryWouldSplitMourningSpan`、`CarryOverflows`）。
4. `layout_debug_assembly` 的 `repair_to_decision_info` 不再 `split(':')`，
   改为枚举映射；`LineRepairDecisionInfo`/`LineRepairCandidateInfo` 的 `kind`/`reason_code`/
   `rejection_reason` 同步改为对应枚举。
5. fixture dump 在写出时生成原有文本，**golden 文本保持逐字节一致**。
   这是本项的核心验证：golden 零 diff 即证明文本等价。

### 第 2 项：progressive technical 与 emergency tracking 结构化

1. `BreakOpportunityDecisionInfo.tier` 由 `Option<String>` 改为 `Option<ProgressiveBreakTier>`。
2. `EmergencyTrackingEligibilityDecisionInfo.reason` 改为 `EmergencyTrackingReason`，
   需覆盖两个产生点（`paragraph_shaping_stage.rs:404` 与 `:512`）的全部值：
   - `ProgressiveTechnicalSpan`；
   - `CurrentLineTechnicalTierRejection(TierSet)`；
   - `StrongNonLexical(StrongNonLexicalReason)`，其中原因含
     `LongRepeatedLetterRun`、`LongHexIdentityRun`、`LongMixedAlphaNumericIdentifier`。
   `TierSet` 用 `u8` 位集合表示（5 个 tier，按 `priority()` 固定顺序输出名称）。
3. `shape_paragraph` 内重复的 reason 构造抽成一次，`emergency.iter().any(...)` 改为结构化比较；
   同时消除「为相等比较先生成展示字符串」的整条链。
4. `line_break_planning_stage` 的 `emergency_tracking_boundary_after_clusters: HashMap<i32, String>`
   改为 `HashMap<i32, EmergencyTrackingReason>`，`or_insert_with(clone)` 改为 `Copy` 插入。
   该类型也被 justifier 的 tracking reason 复用，避免中途转回字符串。

去重仍为 `emergency.iter().any(...)`。位集合只把原因比较变成 O(1) 整数比较，
不会让整个去重变成 O(1)。是否需要额外索引，由多 technical span 的压力测试决定。
在此之前不新增去重集合。

### 第 3 项：justifier 阶段枚举化

1. `JustificationStages.stages: Vec<(Vec<JustificationOpportunity>, String)>` 的第二项改为
   `JustificationStage` 枚举；`prepare_stages` 的
   `matches!(reason, "TerminalTechnicalEmergencyTracking" | "CjkInterChar" | "EmergencyGraphemeTracking")`
   改为枚举匹配。注意该判断决定哪些阶段容量设为无穷，属于**数值规划**，替换时必须逐项对齐。
2. `JustificationOpportunity.reason` 与 `JustificationAllocation.reason` 改为结构化原因：
   stage 名 + 可选修饰（attached inline virtual autospace、preferred tracking 类型）。
3. `allocate()` 不再为每条 allocation 生成字符串；`plan()` 不再 `clone()`/`format!`。
4. 保留 `fill_deltas()` 的「试算不生成机会或原因」特性，不得回退。

### 第 4 项：autospace 与 contextual kinsoku 的诊断字段枚举化

1. `AutoSpaceDecisionInfo.side` → `AutoSpaceSide { Leading, Trailing, Gap }`；
   `mode` 复用既有 `AutoSpaceMode`；`boundary_role` → 有限枚举
   （`EastAsianSpacingWide`、`InlineBoxNarrow`、`InlineAttachmentPrevious` 等现有取值）。
2. 消费者改为 match：`line_measurement.rs:70`、`layout_queries.rs:1699`、
   `annotation_geometry_stage.rs:161`/`:167`、`width_independent_annotation_cache.rs:1188`。
   `side` 是三态而非前后两态，不能机械套用 `GlueSide`。
3. `AutoSpaceDecisionInfo.reason` 的实际取值在 `punctuation_geometry_stage` 与
   `width_independent_annotation_cache` 内均为字面量常量，因此实现为 `&'static str`，
   不引入额外枚举。
4. **未做**：`line_adjustment_stage.rs` 的 `decision.reason == "AttachedAsciiPointMarkKinsoku"`
   仍按文本比较，`ContextualKinsokuDecisionInfo` 的 `forbidden_position` 与 `reason`
   也仍是 `String`。这两处属于 contextual kinsoku 的规则原因，需要先确认
   `unicode_punctuation_boundary_resolver` 是否允许扩展原因，再决定枚举还是保留文本。
   本轮不处理。

### 第 5 项：debug model 复用既有类型

按字段逐一替换，不新建并行的字符串分类：

| 结构 | 字段 | 目标类型 |
| --- | --- | --- |
| `FontDecisionInfo` | `role` | `FontRole` |
| `MetricDecisionInfo` | `role`、`raw_source`、`baseline_class`、`metric_box`、`layout_source` | 既有 `FontRole`/`FontMetricSource`/`BaselineClass`/`MetricBox` |
| `PunctuationDecisionInfo` | `punctuation_class`、`anchor` | `PunctuationClass`、`PunctuationAnchor` |
| `JustificationAllocationInfo` | `kind` | `GlueKind` |
| `RoleOverrideInfo` | `original_role`、`overridden_role` | `FontRole` |
| `KinsokuDecisionInfo` | `level`、`hanging` | `KinsokuLevel`、`HangingPunctuationStyle` |
| `LineLengthGridDecisionInfo` | `body_alignment` | `LastLineAlignment` |
| `InlineBoxDecisionInfo` | `outer_spacing` | `InlineBoxOuterSpacing` |
| `RubyLineHeightDecisionInfo` | `mode` | `RubyLineHeightMode` |
| `InlineObjectDecisionInfo` | preferred stretch kind | `InlineObjectPreferredStretchKind` |

这些类型均已实现 `Debug`，dump 写出时沿用 2026-09-08 第九轮的做法：穷举名称函数，
不依赖 `format!("{:?}")` 的隐式格式，以保证 golden 文本稳定。

### 第 6 项：常量标签去分配

- 内置原因确认为常量的字段改为 `&'static str`：
  `ClusterGeometryDecisionInfo.source`、`FontDecisionInfo.reason`、
  `InlineBoxDecisionInfo.reason`、`InlineObjectDecisionInfo.reason`、
  `ZeroWidthBreakDecisionInfo.reason`、`MaxLinesDecisionInfo.reason`、
  `InlineObjectPunctuationAttachmentDecisionInfo.reason`、`LineEdgeTrimDecisionInfo.reason`。
- 实施前逐个核对是否存在 `with_reason` 之类的扩展入口。若存在实际扩展需求，
  该字段保留为 `enum + Option<Box<str>>` 或 `Cow<'static, str>`；
  若扩展入口无调用方，直接删除，避免留下两套表达。
- `LineDecisionInfo.kind` 的来源是 `LineBreaker::strategy_name()`，已经是 `&'static str`，
  但被 `to_owned()` 一次；改为直接保存 `&'static str`。

### 第 7 项：行级 debug 去重

1. 删除 `LineBox.debug` 与 `LineDebugInfo`。依据：仓库内无读取点，`huozi` 不读，且内容与
   `LineDecisionInfo` 重复。可见行的行号、cluster 范围、end reason 与各项宽度可从 `LineBox` 读到。
2. `LineDecisionInfo.notes` 由 `Vec<String>` 改为两个独立可选字段：
   - `technical_break: Option<ProgressiveBreakTier>`；
   - `justify_fallback: Option<JustificationFallback>`。
   这两类信息每行至多一次、无先后语义，两个 `Option` 比 `Vec` 更直接，也省掉非空 Vec 的分配。
   修复原因从结构化 repair decision 读取，不再写进 notes。
3. `line_decisions` 继续对应 `laid_out_lines` 全部行。
4. **需要明确取舍**：`LayoutResult.lines` 只含可见行，而 `line_decisions` 覆盖全部 committed 行。
   因此被 `max_lines` 截断的隐藏行，其 end reason 与各项宽度**无法**从返回结果恢复。
   本项默认选择「只保留可见行的完整解释，隐藏行保留决策记录与结构化 repair」，
   即：隐藏行的这些宽度不再可从 debug 还原。这属于解释模型的变化，应视为有意的信息取舍。
   若需要隐藏行的完整解释，应在 `LineDecisionInfo` 内保存类型化的行摘要。实施前二选一。
5. 受影响的测试断言改为结构化匹配（`emergency_grapheme_tracking_test`、
   `line_break_repair_engine_test`、`phase_three_white_space` 等对
   `"technical-break:..."` 前缀的断言）。
6. 该项改变了与 Kotlin 上游的结构（上游保留 `LineDebugInfo`），需在
   `docs/key-differences.md` 记录。

### 第 8 项：标点几何来源结构化

1. `CompressionGeometry.source` 改为枚举；`FixedHalfWidth` 从来源名中的后缀变为独立的布尔修饰，
   由 dump 在输出时组合原文本。
2. `PunctuationAtom.geometry_source`、`PunctuationClusterGeometry.reason` 复用同一枚举。
   `ClusterGeometryDecisionInfo.source` 不参与这项：它的固定值 `"PunctuationGeometryLedger"`
   表示「哪一组件产生了记录」，与「几何证据来自哪里」不是同一件事，按第 6 项保留为静态标签。
3. `halt_validation`、`ink_bounds_fallback`、`glyph_placement_reason` 改为各自的有限原因类型。
   `ink_bounds_fallback` 的原因是 shaping 侧产生的，需确认是否允许 backend 扩展。
   本轮**未做**这三项，原因同上。

### 第 9 项：替换与度量原因结构化

1. `CjkPunctuationGlyphSubstitution.reason` 改为结构化原因，码点标签与 role 文本在 dump 时生成。
2. 「debug 汇编重跑替换器」：让 shaping stage 记录实际采用的替换结果与 rollback 原因，
   debug 汇编直接复用，不再第二次调用替换器。
   当前 `layout_debug_assembly` 会对每个 font resolution 重跑 `substitute_for_role()`，
   其 `display_text` 必须与 shaping 实际使用的那份一致。
3. 本项与第 5 项的 `FontDecisionInfo` 类型变更同步进行。
4. `font_metrics` 的 `reason` **未改**。它来自可替换的 `FontMetricsNormalizer`，
   封闭成枚举会约束 backend 扩展；只把它从 `MetricDecisionInfo` 的构造链中保留为 `String`。

### 第 10 项：缓存所有权收敛

该项拆成三个独立验收点，前一点完成且验证通过后才进入下一点。

**10.1 删除 annotation 的重复字段**

`WidthIndependentParagraphAnnotation` 删除 `segment_shaping_cache`、
`segment_font_resolutions`、`substitution_rollbacks` 三份字段，改用 `base_shaping_stage` 中的同名集合。
这是确定的重复存储，不涉及借出与合入。主要收益可能在冷路径分配与缓存保留内存，
不一定体现在热路径耗时，验收按 live bytes 衡量。

**10.2 动态 shaping 的探索不重建整份基础 map**

`shape_paragraph` 不再在开头 clone 整份基础缓存。改为「基础缓存只读借用 + 本次新增 overlay」，
消费方通过联合查找读取。必须明确：

- overlay 的覆盖优先级；
- 重复 range 的处理；
- 返回结构在结束时**不**物化整份基础 map。

如果最终返回类型仍要求完整拥有 `HashMap<TextRange, ShapingResult>`，
在结尾重建基础 map 会把 clone 从入口移到出口，无法得到收益。实施前先把返回结构的
所有权形状确定下来；不能确定就不做这一条。

实施结论：`segment_shaping_cache` 在动态路径上没有下游消费者，只有命中复用的用途，
因此返回结构不需要完整拥有它。改为只返回本次新增项后，不存在「出口重建」的问题。

**10.3 命中路径不 clone `ShapingResult`**

当前 `shape_segment` 的返回类型是拥有的 `ShapingResult`，改掉它涉及闭包签名与结果所有权的整体调整，
超出局部替换的范围。实施前确认能否用借用、共享句柄或索引表达：

- 不用 `Arc` 套所有对象，只包裹确定为不可变且确实被共享的容器；
- 不为免除 clone 引入自引用结构或 `unsafe`。

`build_paragraph_layout_prep` 已把 cluster 复制进 `raw_natural`，这支持「基础 shaped 数据可只读共享」。
但基础与 overlay 的生命周期、以及旧结果在后续布局后是否仍然有效，需单独验证。

### 第 11 项：replay 索引

1. `font_role_by_cluster_range` 的 value 由 `Option<String>` 改为 `Option<FontRole>`
   （依赖第 5 项）。
2. `open_type_features_by_cluster_range` 的 `Vec<String>` 改为共享不可变列表，
   或按 glyph run 索引引用，避免逐 cluster 复制。
3. feature 是测量与绘制同源的必需数据，只优化表示与索引，不改语义。

## 不在本轮范围

- 增加 debug 采集开关、把 `LayoutDebugInfo` 拆成多个 API。这属于 `docs/api-design-report.md`
  记录的更大重构，需要单独设计。
- 把 `Text` 改造成其它表示。`Text::clone()` 已是共享视图复制，不复制底层 UTF-8。
- 字体 family、`candidate_key`、locale、链接目标、profile ID 等由调用方或 backend 提供的动态字符串。
  它们不适合封闭枚举，如需优化应在第 10 项之后单独评估借用或共享。
- 竖排与 JLREQ 相关改动。

## 验证

每项完成后：

1. `cargo check`，检查编辑器诊断；
2. `cargo test`，全量；
3. `cargo test --test tiqian layout_fixture_golden_test`，不得更新 golden；
4. 结构化差分（golden 不输出行 notes，浮点截到一位小数，因此不能单独作为行为不变的证据）：
   行与 cluster 范围、repair 选择与罚分、候选顺序、glyph 与 feature、关键几何原值；
   涉及缓存的改动另比对 backend 调用次数；
5. 计时运行与计数运行分开：计时不启计数，计数不采信时间；与上一轮同参数对比；
6. 三个缓存维度分别跑（默认、annotation-cold、fresh-engine），第 10 项按 10.1、10.2、10.3 分次比较；
7. 开启 `--replay` 单独测第 11 项，不把它的收益算进 layout 时间。

结构变化较大时额外检查：

- 依赖 reason 文本前缀的测试已全部改为结构化断言；
- `huozi-rs` 中 `cargo check` 与 `cargo test` 通过；
- `docs/key-differences.md` 已记录删除 `LineDebugInfo` 与 Kotlin 上游的差异。

## 回滚

提交与回滚单位按依赖拆分，不按文件拆分：

- P1 的三部分各自独立提交；
- 第 2 项与第 3 项共享 `EmergencyTrackingReason`，应作为一组依赖提交；
- 第 5 项与第 11 项共享 `FontRole`，先提交第 5 项，第 11 项可单独回滚；
- 第 10 项的 10.1、10.2、10.3 分三次提交，便于单独回滚；
- 第 1、第 4 项只改内部类型与消费点，可独立回滚；
- 第 7 项改变公开结构，回滚需同时恢复 dump 与测试断言。

任一项出现无法解释的行为差异时，回滚该项，不调整 golden。

## 实施状态

**P0 测量基建已完成**（2026-10-24）。`examples/paragraph-layout-bench.rs` 现有能力：

- 包装 mimalloc 的计数分配器，转发 `alloc`/`alloc_zeroed`/`dealloc`/`realloc`；
  计数只覆盖 `engine.layout()`，`--count-allocations` 开启，默认关闭时不产生原子操作；
- 分离 `alloc_calls`/`realloc_calls`/`dealloc_calls`/`allocated_bytes`/`freed_bytes`；
- `window_live_bytes` 为窗口内未释放的请求字节，`process_live_bytes` 为全进程绝对值；
- 25 类 decision 记录数与记录总数，并提供每次调用的平均值；
- `--annotation-cold`（清空 annotation cache，保留 backend）与 `--fresh-engine`
  （每样本换全新的 cache 状态并重建引擎）；两者互斥；
- annotation cache 的 hits/misses/puts 统计，通过 benchmark 内部的包装层实现，未新增生产 API。

验证：`cargo check`、三种模式（默认、annotation-cold、fresh-engine）实跑正常，
工作量为 47 calls/page、2057 clusters、28200 calls。

尚未采集：`--fresh-engine` 没有计数模式基线，因为重建引擎本身的分配会超出冷 shaping 的量级。

### P1 已完成（2026-10-24）

**A 项：shaping 热路径不再构造替换 reason**

新增 `ClreqPunctuationGlyphSubstitutor::display_text_for_role`，`shape_paragraph` 改用它。
非 `CjkPunctuation` 角色在 role 筛选后显示文本总是等于 source，因此直接返回 source，
不构造候选也不拼 reason。`substitute()` 与 `substitute_for_role()` 行为与文本完全保留。

**第 6 项：常量标签去分配**

以下字段改为 `&'static str`：`InlineBoxDecisionInfo.reason`、`InlineObjectDecisionInfo.reason`、
`ZeroWidthBreakDecisionInfo.reason`、`MandatoryBreakDecisionInfo.reason`、
`MaxLinesDecisionInfo.reason`、`InlineObjectPunctuationAttachmentDecisionInfo.reason`。

删除四处 `with_reason` 扩展入口（`InlineBoxDecisionInfo`、`ZeroWidthBreakDecisionInfo`、
`MaxLinesDecisionInfo`、`InlineObjectPunctuationAttachmentDecisionInfo`），
它们没有生产调用方，唯一一处测试调用改为 `new()`。
`InlineObjectDecisionInfoBuilder::reason` 保留但参数改为 `&'static str`。

**P1 实测**

计数结果是确定性的，可直接比较：

| 指标（宽度 672，每页 47 次布局） | 基线 | P1 | 变化 |
| --- | ---: | ---: | ---: |
| alloc_calls | 76,451 | 76,443 | -8 |
| allocated_bytes | 11,287,705 | 11,287,015 | -690 |
| window_live_bytes | 2,958,656 | 2,958,531 | -125 |
| realloc_calls | 2,331 | 2,331 | 0 |
| debug 记录数 | 7,395 | 7,395 | 0 |

结论：**A 项与第 6 项的收益远低于预期，每页仅减少 8 次分配。**

原因是两条路径的触发频率都很低：

- `display_text_for_role` 只在 CJK 标点 cluster 上才需要解析 policy；
  其余角色原本也只是走一次 `format!`，而该 `format!` 的结果本来就会被丢弃。
  每页 CJK 标点 segment 数量有限，因此只省下 8 次。
- 常量 reason 字段的记录数并不等于分配次数：`InlineBoxDecisionInfo` 每页 3 条、
  `MaxLinesDecisionInfo` 至多 1 条、`ZeroWidthBreakDecisionInfo` 与
  `MandatoryBreakDecisionInfo` 合计 1 条。记录密度高的 `font_decisions`（2,104 条）
  与 `metric_decisions`（2,104 条）尚未处理。

因此 P2 及之后的排序依据改为「记录条数 × 字段数」，改动难度不参与排序。
`font_decisions` 与 `metric_decisions` 各约 2,100 条/页，`geometry_decisions` 168 条，
是接下来更有价值的对象。

**时间测量**

第一次测量时机器上有其他高负载进程，连续运行的 median 跳到 2.55~4.34 ms，
A/B 交替还出现顺序偏差：同一构建放在第二个位置总是更快，正序三轮 P1 全更低、
反序三轮 P1 全更高。该批数据作废。

在 CPU 占用率降到 1.6%~8.5%（24 核）后重测，四轮 A/B 交替（宽度 672，预热 30、150 样本）：

| 轮次 | A 基线 median | B P1 median | 差值 |
| --- | ---: | ---: | ---: |
| r0 | 2.625 | 2.659 | +1.3% |
| r1 | 2.632 | 2.571 | -2.3% |
| r2 | 2.588 | 2.566 | -0.9% |
| r3 | 2.543 | 2.570 | +1.1% |

`min` 同样在两方向摆动（2.485~2.572 与 2.486~2.601）。
两胜两负、差值都在 ±2% 内，与「每页仅少 8 次分配」的预期一致：
**P1 的耗时变化落在噪声范围内，没有可分辨的收益或退化。**

结论：分配计数是本轮唯一有分辨率的指标；耗时变化需按分配次数乘以单次分配成本估算，
不能从端到端时间反推。

### P2 已完成（2026-10-24）

P2 计划覆盖第 5、4、2、3 项，实际完成的是第 5、4、2、3 项（**第 1 项当时被跳过，
见 P5**）。这四项合起来使每页分配次数下降 **21.4%**。

第 5 项中 `InlineObjectDecisionInfo` 的两个 preferred stretch kind 字段实际未改，
同样见 P5。

**第 5 项：debug model 复用既有类型**

按记录密度优先处理 `font_decisions` 与 `metric_decisions`（各约 2,104 条/页）：

| 结构 | 字段 | 改为 |
| --- | --- | --- |
| `FontDecisionInfo` | `role` | `FontRole` |
| `FontDecisionInfo` | `reason` | `&'static str` |
| `MetricDecisionInfo` | `role` | `FontRole` |
| `MetricDecisionInfo` | `raw_source`、`layout_source` | `FontMetricSource` |
| `MetricDecisionInfo` | `baseline_class` | `BaselineClass` |
| `MetricDecisionInfo` | `metric_box` | `MetricBox` |
| `PunctuationDecisionInfo` | `punctuation_class` | `PunctuationClass` |
| `PunctuationDecisionInfo` | `anchor` | `PunctuationAnchor` |
| `JustificationAllocationInfo` | `kind` | `GlueKind` |
| `RoleOverrideInfo` | `original_role`、`overridden_role` | `FontRole` |
| `KinsokuDecisionInfo` | `level`、`hanging` | `KinsokuLevel`、`HangingPunctuationStyle` |
| `LineLengthGridDecisionInfo` | `body_alignment` | `LastLineAlignment` |
| `RubyLineHeightDecisionInfo` | `mode` | `RubyLineHeightMode` |
| `InlineBoxDecisionInfo` | `outer_spacing` | `InlineBoxOuterSpacing` |
| `BreakOpportunityDecisionInfo` | `reason` | `&'static str` |
| `BreakOpportunityDecisionInfo` | `tier` | `Option<ProgressiveBreakTier>` |

`MetricDecisionInfo.reason` 保留 `String`：它来自可替换的 `FontMetricsNormalizer`，
封闭成枚举会约束 backend 扩展。`FontDecisionInfo.substitution_reason` 同理保留。

删除 `src/` 中已无生产调用方的 `font_role_name` 与 `font_metric_source_name`；
名称映射改由 dump 层自行实现。

**第 4 项：autospace 诊断字段枚举化**

- `AutoSpaceDecisionInfo.side` → 新增 `AutoSpaceSide { Leading, Trailing, Gap }`。
  它是三态，不能套用只有前后的 `GlueSide`。
- `boundary_role` → 新增 `AutoSpaceBoundaryRole { EastAsianSpacingWide, InlineBoxNarrow, InlineAttachmentPrevious }`。
- `mode` 复用既有 `AutoSpaceMode`；`reason` → `&'static str`。
- 消费点改为枚举匹配：`line_measurement`、`layout_queries`、`annotation_geometry_stage`、
  `width_independent_annotation_cache`。原先 `match decision.side.as_str()` 的 `_ => {}` 通配分支
  在枚举化后消失，未覆盖的取值由编译器检查。

**第 2 项：progressive technical 与 emergency tracking 结构化**

- 新增 `RejectedTierSet`：5 个 tier 用 `u8` 位集合表示，按 `priority()` 升序迭代，
  使原因相等比较退化为整数比较。
- 新增 `StrongNonLexicalReason`，覆盖审计漏掉的三个分支：
  `LongRepeatedLetterRun`、`LongHexIdentityRun`、`LongMixedAlphaNumericIdentifier`
  （`paragraph_shaping_stage.rs:967` 起）。
- 新增 `EmergencyTrackingReason { ProgressiveTechnicalSpan, CurrentLineTechnicalTierRejection(RejectedTierSet), StrongNonLexical(StrongNonLexicalReason) }`，
  覆盖两个产生点。
- 原「收集 tier → 排序 → 逐个 `format!("{:?}")` → `join("+")`」的整条链被删除；
  同一 reason 只构造一次，同时用于去重比较与 decision 记录。
- `emergency_tracking_boundary_after_clusters` 与 justifier 的两个 tracking map
  从 `HashMap<i32, String>` 改为 `HashMap<i32, EmergencyTrackingReason>`。
- 位集合只把原因比较变成整数比较，`emergency.iter().any(...)` 仍是线性扫描；
  未新增去重索引。

**第 3 项：justifier 阶段枚举化**

- 新增 `JustificationStage` 枚举，`has_unbounded_capacity()` 精确对应原先的三个阶段名字符串
  （`TerminalTechnicalEmergencyTracking`、`CjkInterChar`、`EmergencyGraphemeTracking`）。
  该判断决定哪些阶段容量视为无穷，属于数值规划，替换后逐个对齐。
- 新增 `JustificationReason`（含 `AttachedInlineVirtualAutoSpace`、
  `WesternBracketCjkInterChar`、`AttachedInlineVirtualInterChar`、
  `InlineObjectPreferredStretch(kind)`、`TerminalTechnicalEmergencyTracking(reason)`、
  `EmergencyGraphemeTracking(reason)`）。
- 新增 `JustificationAllocationReason { Stage(JustificationStage), Opportunity(JustificationReason) }`。
- `allocate()` 与 `plan()` 不再逐条生成字符串；`boundary_opportunities` 的 `reason`
  参数由 `Option<&str>` 改为 `Option<JustificationReason>`。
- `fill_deltas()` 的「试算不生成机会或原因」特性保留。

**P2 实测**

| 指标（宽度 672，每页 47 次布局） | 基线 | P1 后 | P2 后 | P2 相对基线 |
| --- | ---: | ---: | ---: | ---: |
| alloc_calls | 76,451 | 76,443 | 60,111 | **-21.4%** |
| allocated_bytes | 11,287,705 | 11,287,015 | 10,632,842 | -5.8% |
| window_live_bytes | 2,958,656 | 2,958,531 | 2,435,636 | -17.7% |
| realloc_calls | 2,331 | 2,331 | 2,331 | 0 |
| debug 记录数 | 7,395 | 7,395 | 7,395 | 0 |

记录数与 `realloc_calls` 不变，符合预期：本轮只改变字段表示，不增删记录。

分配次数下降 16,332 次，其中大部分来自第 5 项：
`font_decisions` 与 `metric_decisions` 合计约 4,208 条/页，每条省去 5 至 6 次
角色与来源名称的 `to_owned()`。这验证了「按记录条数乘字段数排序」的判断：
P1 处理的字段每页合计不到 10 条，收益不足 10 次分配。

**当前验证边界**

`cargo check --lib` 零错误零警告。测试与 dump 尚未适配：
测试中的 `"CjkText"`、`"CjkInterChar"` 等字符串字面量不再能与新类型比较，
`tests/fixture_layout/dump.rs` 也需要用自己的名称函数生成原有输出文本。
这些属于接口变更的连带修复，安排在迭代最后统一处理。
届时以「golden 文本逐字节一致」作为类型替换未改变可解释输出的证据。

### P3 已完成（2026-10-24）

P3 覆盖第 7、8、9 项。三项合起来使每页分配次数再降 2,146 次。

**第 7 项：行级 debug 去重**

- 删除 `LineBox.debug` 字段与 `LineDebugInfo` 结构体，以及 `LineBoxBuilder::debug`。
  依据：仓库内无读取点，`huozi` 不读，内容与 `LineDecisionInfo` 重复。
- `LineDecisionInfo.notes: Vec<String>` 改为两个字段：
  - `technical_break: Option<ProgressiveBreakTier>`；
  - `justify_fallback: Option<String>`。
  这两类信息每行至多一次、无先后语义，两个 `Option` 省掉非空 `Vec` 的分配。
  修复原因从结构化 repair decision 读取，不再写进 notes。
- 删除 notes 中与 `LineBox` 完全重复的六条记录：行号、end reason、
  natural/adjusted/alignment/visual 宽度。
- `LineDecisionInfo.kind` 改为 `&'static str`；`line_breaker_strategy_name` 沿
  `LineAdjustmentRequest`、`LayoutDebugStageInput` 一并改为 `&'static str`。
- 删除 `build_line_boxes` 已无消费者的 `justification_plans` 参数。

信息取舍已明确：`LayoutResult.lines` 只含可见行，`line_decisions` 覆盖全部 committed 行，
因此被 `max_lines` 截断的隐藏行的宽度不再可从 debug 还原。本项接受该取舍。

**第 8 项：标点几何来源结构化**

- 新增 `PunctuationGeometrySource` 枚举与 `name()`，覆盖六个现有取值：
  `PolicyDerived`、`FontHaltFittedBodyCompression`、
  `FontHaltAdvanceWithInkBoundsFittedPlacement`、`InkBoundsFittedBodyCompression`、
  `FontHaltAdvanceWithProfileFallback`、`ProfileGlueFallbackWithoutFontGeometry`。
- `PunctuationAtom.geometry_source` 改为该枚举；`FixedHalfWidth` 从来源名后缀
  拆为独立的 `geometry_forced_half_width: bool`，由输出层重新组合原文本。
- `CompressionGeometry.source`、`PunctuationClusterGeometry.reason`、
  `ClusterGeometryDecisionInfo.reason`、`PunctuationDecisionInfo.geometry_source`
  复用同一枚举。
- `ClusterGeometryDecisionInfo.source` 改为 `&'static str`。它与几何来源语义不同：
  前者记录「哪一组件产生记录」（固定为 `PunctuationGeometryLedger`），
  后者记录「几何证据来自哪里」，因此不共用枚举。
- `halt_validation`、`ink_bounds_fallback`、`glyph_placement_reason` 本轮未改。
  `ink_bounds_fallback` 的来源在 shaping 侧，需先确认 backend 是否允许扩展。

**第 9 项：debug 复用 shaping 实际采用的替换结果**

- 新增 `CjkPunctuationGlyphSubstitutionReason` 枚举：
  `PolicyPreserved(policy)`、`PolicyReplaced(policy)`、`RolePreserved(role)`。
  `render()` 接受两个按需调用的标签构造器，码点标签只在输出时生成。
- `CjkPunctuationGlyphSubstitution.reason` 改为该枚举；
  新增 `reason_for_role(source, role, display_text)`，按 role 筛选结果推导原因，
  不重算显示文本。
- `ParagraphShapingStageResult` 新增 `substitutions: HashMap<TextRange, CjkPunctuationGlyphSubstitution>`，
  记录 shaping 实际采用的替换；沿 `ParagraphLayoutPrep` 透传到 debug 汇编。
- `layout_debug_assembly` 不再调用 `substitute_for_role()`，
  改用记录的结果；`LayoutDebugStageInput` 因此删除 `punctuation_glyph_substitutor` 字段。
  debug 的 `display_text` 与测量、绘制因此同源。
- `to_code_point_labels` 改为 `pub`，供输出层复用同一格式。

**已知行为差异（待测试阶段确认）**

第 9 项中，rollback 发生且 shaping 未记录替换时，reason 文本为单独的 cause；
原实现为「替换原因:cause」。若某 fixture 中存在「原始文本可被替换但发生 rollback」
的情形，dump 输出会缺少前半段。测试阶段以 golden 对比确认是否触发；
若确实触发，需要让 rollback 分支也记录替换，代价是多一次 map 写入。

**P3 实测**

| 指标（宽度 672，每页 47 次布局） | 基线 | P2 后 | P3 后 | 累计相对基线 |
| --- | ---: | ---: | ---: | ---: |
| alloc_calls | 76,451 | 60,111 | 57,965 | **-24.2%** |
| realloc_calls | 2,331 | 2,331 | 2,102 | -9.8% |
| allocated_bytes | 11,287,705 | 10,632,842 | 10,841,537 | -4.0% |
| window_live_bytes | 2,958,656 | 2,435,636 | 2,414,801 | -18.4% |
| debug 记录数 | 7,395 | 7,395 | 7,395 | 0 |

`realloc_calls` 开始下降，来自 `notes: Vec<String>` 改为两个 `Option` 后不再扩容。
`allocated_bytes` 略高于 P2，因为新增 `substitutions` map 记录。

### P4 已完成（2026-10-24）

P4 覆盖第 10 项的三步与第 11 项。

**10.1 删除 annotation 的重复字段**

`WidthIndependentParagraphAnnotation` 删除 `segment_shaping_cache`、
`segment_font_resolutions`、`substitution_rollbacks` 三份字段，统一从 `base_shaping_stage` 读取。
这三份此前由 `base` 显式 clone 一份，与 `base_shaping_stage` 内的同名集合重复存储。

验收按常驻内存衡量：分配次数不变（同一份数据只是不再存两次），
`process_live_bytes` 从 93.77 MB 降到 92.01 MB，减少 1.76 MB。

**10.2 动态 shaping 不再整份 clone 基础缓存**

确认 `segment_shaping_cache` 没有下游消费者，只有 segment 命中复用的用途。
因此改为「本次新增项局部表 + 基础缓存只读借用」：`shape_segment` 先查新增项，
再查传入的基础缓存，不再在入口整份复制 `HashMap<TextRange, ShapingResult>`。

`segment_font_resolutions` 与 `substitution_rollbacks` 仍保留完整集合，
因为 prep 与 debug 会读取它们，语义未变。

冷路径（annotation-cold）分配从 337,283 降到 301,552（-10.6%），
窗口存活从 9.29 MB 降到 7.19 MB（-22.6%）。

**10.3 命中路径不 clone `ShapingResult`**

`shape_segment` 返回类型改为 `Arc<ShapingResult>`；`shaping_results` 与
`segment_shaping_cache` 同步改为 `Arc`。命中时只增加引用计数，
不再复制 cluster、glyph run 与 decision 三个向量。

`ShapingResult` 在下游只读：`build_paragraph_layout_prep` 已先把 cluster
复制进 `raw_natural`，后续阶段不再修改 shaped cluster。因此共享是安全的。

未引入 `unsafe` 或自引用结构。`latin_word_cuts` 与 `opaque_hard_cuts`
的闭包参数类型同步改为 `Arc<ShapingResult>`。

冷路径分配再降到 274,604，窗口存活降到 6.13 MB。

**第 11 项：replay 索引共享 feature 列表**

`open_type_features_by_cluster_range` 的 value 由 `Vec<String>` 改为 `Arc<[String]>`，
按 glyph run 建一份共享列表，同一 run 内的 cluster 共用句柄。
role 类型已在 P2 的第 5 项改为 `Option<FontRole>`。

该改动只影响 `--replay` 路径，不改变 layout 时间；开启 replay 时
`replay` 阶段 median 为 0.319 ms。

**P4 实测**

| 指标（宽度 672，每页 47 次布局） | P3 后 | 10.1 | 10.2 | 10.3（P4 终值） |
| --- | ---: | ---: | ---: | ---: |
| alloc_calls（热路径） | 57,965 | 57,965 | 57,432 | 56,403 |
| alloc_calls（annotation-cold） | 337,283 | 未测 | 301,552 | 274,604 |
| window_live_bytes（冷） | 9.29 MB | 未测 | 7.19 MB | 6.13 MB |
| process_live_bytes | 93.77 MB | 92.01 MB | 92.01 MB | 90.95 MB |
| debug 记录数 | 7,395 | 7,395 | 7,395 | 7,395 |

10.1 只降常驻内存，不改分配次数；10.2 与 10.3 同时降低热路径与冷路径分配，
冷路径降幅更大（-18.6%），说明缓存复用路径的复制成本确实集中在
`ShapingResult` 与整份 map 上。

**累计（P0 基线 → P4 终值）**

| 指标 | 基线 | P4 终值 | 变化 |
| --- | ---: | ---: | ---: |
| alloc_calls（热路径） | 76,451 | 56,403 | **-26.2%** |
| alloc_calls（annotation-cold） | 337,283 | 274,604 | -18.6% |
| realloc_calls | 2,331 | 2,102 | -9.8% |
| allocated_bytes | 11,287,705 | 10,678,945 | -5.4% |
| window_live_bytes | 2,958,656 | 2,414,801 | -18.4% |
| debug 记录数 | 7,395 | 7,395 | 0 |

### 待办：测试与 dump 适配

`src/` 主线已全部完成，`cargo check --lib` 零错误零警告。剩余工作是让测试与
fixture dump 适配新的公开类型，属于接口变更的连带修复。

**状态：已完成。** 见下方「测试与 dump 适配」小节。

- 测试中的 `"CjkText"`、`"CjkInterChar"`、`"CjkPunctuation"` 等字符串字面量
  不再能与 enum 比较，需改为枚举值；
- `tests/fixture_layout/dump.rs` 需自行实现名称映射
  （`src/` 中的 `font_role_name`、`font_metric_source_name` 已随类型化删除），
  以逐字节复现原有输出文本；
- 依赖 `notes` 前缀的断言（`technical-break:...`、`justify-fallback:...`）
  改为读取新字段；
- `LineDecisionInfo.kind` 变为 `&'static str`，`MaxLinesDecisionInfo::with_reason`
  等已删除的构造函数调用点需要改用 `new()`。

验收以 `cargo test` 全通过且 golden 文本逐字节一致为准。
这是类型替换未改变可解释输出的唯一直接证据。

- 测试中的 `"CjkText"`、`"CjkInterChar"`、`"CjkPunctuation"` 等字符串字面量
  不再能与 enum 比较，需改为枚举值；
- `tests/fixture_layout/dump.rs` 需自行实现名称映射
  （`src/` 中的 `font_role_name`、`font_metric_source_name` 已随类型化删除），
  以逐字节复现原有输出文本；
- 依赖 `notes` 前缀的断言（`technical-break:...`、`justify-fallback:...`）
  改为读取新字段；
- `LineDecisionInfo.kind` 变为 `&'static str`，`MaxLinesDecisionInfo::with_reason`
  等已删除的构造函数调用点需要改用 `new()`。

验收以 `cargo test` 全通过且 golden 文本逐字节一致为准。
这是类型替换未改变可解释输出的唯一直接证据。

## P5：遗漏项补做

P0–P4 完成后，用「所有 `String` 形态的 reason 类字段」做了一次全量复查，
确认仍有未处理项。下列每一项都在 2026-10-24 逐条核实了值域与产生点。

复查方法：在 `src/` 中搜 `pub (kind|reason|reason_code|rejection_reason|side|source|mode|level|hanging|outer_spacing|body_alignment|forbidden_position|substitution_reason): String` 及对应 `Option<String>` 形式，再逐个回溯产生点确认值域。

### 已确认的遗漏（文档记录与实际不一致）

| 编号 | 项目 | 字段位置 | 值域（已核实） | 未做的原因 |
| --- | --- | --- | --- | --- |
| 1 | **repair 机器状态结构化** | `line_optimization.rs:195` `RepairCandidate.kind`<br>`:196` `reason_code`<br>`:200` `rejection_reason` | `kind`：PushIn / Hang / CarryPrevious / CarryNext / LeaveRagged<br>`reason_code`：LineAdjustmentPushIn / ProgressiveTechnicalTierPromotion / ForbiddenAtLineStart / ForbiddenAtLineEnd<br>`rejection_reason`：insufficient-capacity / no-room-to-carry / carry-would-split-mourning-span / carry-overflows | P2 只做了 A 项与第 6 项，**第 1 项从未实施**，但 P2 小节把它列为已完成 |
| 2 | **`RepairOption` 各变体 reason** | `line_optimization.rs:23` 起五个变体 | 与第 1 项同源，另带 offender 文本、shrink/capacity 数值 | 同上；`line_repair.rs:480`、`:620` 的 `starts_with("LineAdjustmentPushIn:")` 仍在，它是实际参与算法分派的文本判断 |
| 3 | **repair 的 debug 投影** | `layout_model.rs:2011` `LineRepairDecisionInfo.kind`<br>`:2012` `reason_code`<br>`:2088` `LineRepairCandidateInfo.kind`<br>`:2089` `reason_code`<br>`:2093` `rejection_reason` | 同第 1 项 | 承接未完成的第 1 项；`layout_debug_assembly.rs:366` 的 `split(':').next()` 仍在 |
| 4 | **`InlineObjectDecisionInfo` preferred stretch kind** | `layout_model.rs:801` `leading_preferred_stretch_kind`<br>`:809` `trailing_preferred_stretch_kind` | `InlineObjectPreferredStretchKind` 三值：PunctuationTrailing / Relation / BinaryOperator | P2 第 5 项表格**曾写为已完成**，实际当时仍是 `Option<String>`；已在 P5 补做 |

### 本轮明确不做（值域有限，但保留文本有理由或收益过低）

| 编号 | 项目 | 字段位置 | 值域（已核实） | 保留理由 |
| --- | --- | --- | --- | --- |
| 5 | `MetricDecisionInfo.reason` | `layout_model.rs:1681` | 由 `ScriptAwareFontMetricsNormalizer` 产出三段固定文本 | 来源是可替换的 `FontMetricsNormalizer`，封闭成枚举会约束 backend 扩展 |
| 6 | `RoleOverrideInfo.source` / `reason` | `layout_model.rs:1939` / `:1940` | 由 quote / dash-ellipsis resolver 产出 | 与第 5 项同理：resolver 可通过 trait 替换 |
| 7 | `ContextualDashEllipsisRoleResolver` / `ContextualQuoteRoleResolver` 的 `source`、`reason` | `contextual_dash_ellipsis_role_resolver.rs:19`、`:20`；`quote_pair_analyzer.rs:38`、`:39` | resolver 内部产出 | 同上，属可扩展解析器 |
| 8 | `font_policy.rs:74` / `:427` 的 reason | 字体选择与度量规范化决策 | resolver 名 + role | 同上 |
| 9 | `BreakOpportunity.reason` / `ForbiddenBreak.reason` | `linebreak/line_break.rs:11`、`:117` | 内置 analyzer 产出固定文本，但 `LineBreakAnalyzer` 是公开 trait | 允许外部 analyzer 提供自定义原因，封闭枚举会破坏扩展能力 |

### 本轮可以补做，但当时未覆盖（值域有限、无扩展入口）

| 编号 | 项目 | 字段位置 | 值域（已核实） | 密度 |
| --- | --- | --- | --- | --- |
| 10 | `ContextualKinsokuDecisionInfo.reason` | `layout_model.rs:1029` | 三个产生点：<br>· `AttachedAsciiPointMarkKinsoku`（`punctuation_geometry_stage.rs:231`）<br>· `InlineObjectAttachedMarkKinsoku`<br>· `Uax14WesternPunctuationBoundary:{PairedClosingQuote \| LB19 \| rule_for_start}`，其中 `rule_for_start` 只返回 `LB13` / `LB15d` | 每页数十条量级 |
| 11 | `ContextualKinsokuDecisionInfo.forbidden_position` | `layout_model.rs:1028` | 仅 `LineStart` / `LineEnd` 两值 | 同上 |
| 12 | `LineEdgeTrimDecisionInfo.side` | `layout_model.rs:1496` | 仅 `leading` / `trailing`（`line_adjustment_stage.rs:694`、`:718`、`:744`） | 每页数十条 |
| 13 | `SpacingDecisionInfo.reason` | `layout_model.rs:1929` | 三类固定前缀 + 四个后缀：<br>`AttachedInlineVirtualPunctuationBoundary:{line-end \| adjacent-punctuation \| ascii-point-mark \| natural}`（`punctuation_geometry_ledger.rs:211`）<br>`Line{Start\|End}CenteredPunctuationPairedCompression`<br>`Line{Start\|End}HalfWidthPunctuation`（`:342`、`:347`） | 每页数条 |
| 14 | `PunctuationSpacingAdjustment.reason` | `punctuation_model.rs:223` | 与第 13 项同一组取值 | 同上 |
| 15 | `DecorationDecisionInfo.reason` | `layout_model.rs:1430` | 三值：`EmphasisDotOnHanText` / `clreq-no-dot-on-punctuation` / `no-dot-on-non-han`（`annotation_geometry_stage.rs:283-291`） | 逐着重号 cluster，密度较高 |
| 16 | `DecorationSegmentInfo.reason` | `layout_model.rs:1416` | 三值 + 一个后缀：`MourningSpanKeptUnbroken` / `mourning-span-split-across-lines` / `InterlinearLinePerAnnotatedItem`，外加 `;AdjacentInterlinearLineShortening` 修饰（`annotation_geometry_stage.rs:410-418`、`:452`、`:457`） | 逐行段，密度中等 |
| 17 | `FirstLineIndentDecisionInfo.source` | `layout_model.rs:985` | 二值：`Explicit` / `MeasureAdaptiveFirstLineIndent`（`line_break_planning_stage.rs:313-318`） | 每段 1 条 |
| 18 | `KinsokuDecisionInfo.reason` / `ResolvedKinsoku.reason` | `layout_model.rs:1016`；`clreq_profile.rs:470` | `Fixed:{level}{+Hang}` 或 `MeasureAdaptiveKinsoku:{n}字→{level}{+Hang}`（`clreq_profile.rs:537`、`:566`） | 每段 1 条 |
| 19 | `LineSpacingDecisionInfo.reason` | `layout_model.rs:1116` | 三值：`ExplicitLineHeight` / `InterlinearMarkLineSpacingFloor` / `CjkBodyLineHeightDefault`（`line_break_planning_stage.rs:285-291`） | 每段 1 条 |
| 20 | `RubyLineHeightDecisionInfo.reason` | `layout_model.rs:1131` | 二值：`ConditionalRubyLineHeight` / `ExistingInterlineSpaceFitsRuby`（`line_geometry_stage.rs:242-246`） | 每段至多 1 条 |
| 21 | `InlineObjectLineHeightDecisionInfo.reason` | `layout_model.rs:1151` | 二值：`ExistingInterlineSpaceFitsInlineObjects` / `InlineObjectInterlineCollision`（`line_geometry_stage.rs:366-370`） | 每段至多 1 条 |
| 22 | `JustificationAllocationInfo.reason` | `layout_model.rs:2181` | 内部已是 `JustificationAllocationReason` 枚举，**在 dump 边界又转回 String** | 每页数百条，密度最高 |

### 优先级建议

按「记录条数 × 字段数」排序，第 15、16、22 项密度最高；第 10–12 项密度中等，且带参与分派的文本比较（`line_adjustment_stage.rs:85` 的 `== "AttachedAsciiPointMarkKinsoku"`），
改动价值高于纯 debug 字段；第 17–21 项每段至多一条，收益以类型安全为主。

第 1–3 项是唯一仍有「算法按文本分派」的部分，架构价值最高，但涉及约 25 处测试断言，
且需要保证 golden 文本不变。

### P5 验收

- `cargo check --lib` 零错误零警告；
- 每完成一项记录分配次数变化，保持「记录数不变」；
- 第 1–3、22 项完成后，`starts_with`、`split(':')`、dump 边界的字符串往返应全部消除。

### P5 状态

按编号顺序实施。已完成第 1–4 项。

**第 1–3 项：repair 机器状态结构化（提交 `54e8b5e`）**

新增四个类型：

| 类型 | 变体 |
| --- | --- |
| `RepairKind` | PushIn / Hang / CarryPrevious / CarryNext / LeaveRagged |
| `RepairReasonCode` | LineAdjustmentPushIn / LineAdjustmentPushInDpFeasibility / ProgressiveTechnicalTierPromotion / ForbiddenAtLineStart / ForbiddenAtLineEnd |
| `RepairRejection` | InsufficientCapacity / NoRoomToCarry / CarryWouldSplitMourningSpan / CarryOverflows |
| `RepairDetail` | Hang / Carried{index} / MovedToNextLine{index} / FitsNoShrink / PushedIn / Rejected(rejection) |

`RepairOption` 五个变体的 `reason: String` 改为 `reason_code: RepairReasonCode`
加 `detail: RepairDetail`；`RepairCandidate` 的 `kind`/`reason_code`/`rejection_reason`
同步类型化；`LineRepairDecisionInfo` 与 `LineRepairCandidateInfo` 一并改为枚举。

消除的文本协议：

- `line_repair.rs` 两处 `reason.starts_with("LineAdjustmentPushIn:")` 改为
  `*reason_code == RepairReasonCode::LineAdjustmentPushIn`；
- `layout_debug_assembly` 的 `reason.split(':').next()` 反解析已删除；
- 顺带删除已无消费者的 `portable()` 与 `repair_name()`，
  以及 `leave_ragged` 的 `cluster` 参数。

**第 4 项：preferred stretch kind 类型化**

`InlineObjectDecisionInfo` 的 `leading_preferred_stretch_kind` 与
`trailing_preferred_stretch_kind` 由 `Option<String>` 改为
`Option<InlineObjectPreferredStretchKind>`；builder 签名同步改为该类型；
`annotation_geometry_stage.rs` 的 `format!("{:?}", kind)` 两处删除。

**P5 前四项实测**

| 指标（宽度 672，每页 47 次布局） | P4 终值 | 第 1–3 项 | 第 4 项 |
| --- | ---: | ---: | ---: |
| alloc_calls | 56,403 | 56,093 | 56,093 |
| realloc_calls | 2,102 | 2,065 | 2,065 |
| allocated_bytes | 10,678,945 | 10,659,487 | 10,659,487 |
| debug 记录数 | 7,395 | 7,395 | 7,395 |

第 1–3 项减少 310 次分配、37 次 realloc。第 4 项在普通中文 demo 上没有可测变化：
`InlineObjectDecisionInfo` 每页只有少数几条，属于低频路径，收益以类型安全为主。

**过程记录**

实施第 4 项前误跳到第 10 项（contextual kinsoku），并留下编译不通过的半成品。
已回滚该部分，按顺序重做第 4 项。第 10 项的四个新类型定义
（`KinsokuPosition`、`ContextualKinsokuRule`、`Uax14Rule`、`ImpossibleMeasureFallback`）
随回滚一并撤销，待轮到该项时重新实现。

**下一步**：第 5–9 项标注为本轮不做（理由见上表），因此剩余项为 10–22。

### P5 剩余项执行顺序

按优先级建议重排为：22 → 15 → 16（密度最高）→ 10 → 11 → 12（中等密度且带算法分派）
→ 13 → 14（同一组取值）→ 17 → 18 → 19 → 20 → 21（每段至多一条）。
编号顺序不等于价值顺序，此处按价值排序，逐项记录，不跳项。

**第 22 项：`JustificationAllocationInfo.reason`**

`LayoutDebugAssembly` 此前把 `allocation.reason`（已是 `JustificationAllocationReason`）
调 `name()` 转成字符串。改为直接保留枚举，`layout_debug_assembly.rs` 的
`.map(|allocation| ... reason: allocation.reason.name())` 改为 `reason: allocation.reason`。

这是全部项目里唯一的「内部已是枚举、在 debug 边界又转回字符串」情形，
去掉后不再有往返。分配次数 56,093 → 55,694（-399）。

**第 15 项：`DecorationDecisionInfo.reason`**

新增 `EmphasisDotReason` 三值：`EmphasisDotOnHanText` / `ClreqNoDotOnPunctuation` /
`NoDotOnNonHan`；字段与构造函数签名同步改为该类型，`annotation_geometry_stage.rs`
的三处 `.to_owned()` 删除。

分配次数 55,694 → 55,675（-19）。该字段逐着重号 cluster 记录，但默认 demo 的着重号
覆盖不高，所以降幅小。

**第 16 项：`DecorationSegmentInfo.reason`**

新增 `DecorationSegmentReason` 三值：`MourningSpanKeptUnbroken` /
`MourningSpanSplitAcrossLines` / `InterlinearLinePerAnnotatedItem`。原先
「三值 + `;AdjacentInterlinearLineShortening` 后缀」的组合文本，改为
`reason` 枚举加独立的 `adjacent_shortened: bool`。

构造时原先先写 `String::new()`、再在 span 层回填 reason，现在直接写枚举值；
`shorten_adjacent_interlinear_lines` 里两处 `format!` 拼接改为置位布尔。

分配次数 55,675 → 55,672（-3）。`DecorationSegmentInfo` 每个 span 每行一条，
demo 中示亡号与专名号覆盖很低。

**第 10、11 项：`ContextualKinsokuDecisionInfo`**

这三个字段一起改，因为它们在同一条记录上，逐个改只会多做两遍构造点遍历。

| 字段 | 新类型 | 变体 |
| --- | --- | --- |
| `forbidden_position` | `KinsokuForbiddenPosition` | LineStart / LineEnd |
| `reason` | `ContextualKinsokuRule` | AttachedAsciiPointMarkKinsoku / InlineObjectAttachedKinsoku / InlineObjectAttachedKinsokuAcrossCollapsedSeparatorSpace / Uax14WesternPunctuationBoundary(`Uax14BoundaryRule`) |
| `impossible_measure_fallback` | `Option<ImpossibleMeasureFallback>` | AttachedAsciiPointMarkImpossibleMeasureHang / InlineObjectAttachedMarkImpossibleMeasureHang |

`Uax14BoundaryRule` 六值（PairedClosingQuote / PairedOpeningQuote / LB19 / LB14 / LB13 / LB15d）
吸收原先 `format!("Uax14WesternPunctuationBoundary:{}", rule_for_start(...))` 的拼装；
`rule_for_start` 返回类型改为该枚举。

两处算法分派同步改为匹配：`line_adjustment_stage.rs` 的
`decision.reason == "AttachedAsciiPointMarkKinsoku"` 改为
`decision.reason.is_attached_ascii_point_mark()`；去重条件里的
`existing.forbidden_position == decision.forbidden_position` 现在是枚举比较。

分配次数 55,672 → 55,634（-38）。

**第 12 项：`LineEdgeTrimDecisionInfo`**

`side` 改 `LineEdgeTrimSide`（Leading / Trailing / Both），`reason` 改 `LineEdgeTrimReason`。
核实取值时发现文档漏记 `both`：`punctuation_geometry_ledger.rs` 的居中锚定标点分支
写 `"both"`，原本靠 `side` 字符串承载。改为枚举时一并纳入。

原 `format!("Line{Start|End}{CenteredPunctuationPairedCompression|HalfWidthPunctuation}")`
四个组合改为四个变体；`PunctuationLineEdge::side()`/`reason_part()` 换成
`trim_side()`，不再需要字符串拼接。

顺带发现 `LineEdgeWordSpaceCollapse` 已无产生点：提交 `5e90140` 的空白处理重构
删除了 `line_adjustment_stage.rs` 里 `add_amount(&mut trims, cluster_index, cluster.advance)`
那一整段分支，但 `origin/` 与旧文档仍保留该名称。Rust 侧只有测试在用
`decision.reason != "LineEdgeWordSpaceCollapse"` 做反向断言。该变体在类型化时直接不建立，
不保留死变体。

分配次数 55,634 → 55,550（-84）。

**第 13、14 项：`SpacingDecisionInfo.reason` 与 `PunctuationSpacingAdjustment.reason`**

两者共用同一组取值，一起处理。新增 `SpacingReason`（定义在 `punctuation_model.rs`，
`layout_model.rs` 从那里导入）：

| 变体 | dump 文本 |
| --- | --- |
| `CollapseAdjacentPunctuationInnerGlue` | `collapse-adjacent-punctuation-inner-glue` |
| `CollapseCjkClosingBeforeAsciiPointMark` | `collapse-cjk-closing-before-ascii-point-mark` |
| `AttachedInlineVirtualPunctuationBoundary(case)` | `AttachedInlineVirtualPunctuationBoundary:{case}` |

`case` 是 `AttachedInlineBoundaryCase` 四值：LineEnd / AdjacentPunctuation / AsciiPointMark / Natural，
吸收原先 `punctuation_geometry_ledger.rs:212` 那行单行紧凑写法的四段三元表达。
该行同时被拆成正常缩进的多行结构，不再是一整行。

分配次数 55,550 → 55,536（-14）。

**第 17–21 项：每段至多一条的决策字段**

五项都是每段最多一条记录，收益以类型安全为主，一起处理。每项都新增一个二值或三值枚举：

| 项目 | 字段 | 新类型 |
| --- | --- | --- |
| 17 | `FirstLineIndentDecisionInfo.source` | `FirstLineIndentSource`：Explicit / MeasureAdaptiveFirstLineIndent |
| 18 | `KinsokuDecisionInfo.reason` | `KinsokuPolicySource`：Fixed / MeasureAdaptiveKinsoku |
| 19 | `LineSpacingDecisionInfo.reason` | `LineSpacingReason`：ExplicitLineHeight / InterlinearMarkLineSpacingFloor / CjkBodyLineHeightDefault |
| 20 | `RubyLineHeightDecisionInfo.reason` | `RubyLineHeightReason`：ConditionalRubyLineHeight / ExistingInterlineSpaceFitsRuby |
| 21 | `InlineObjectLineHeightDecisionInfo.reason` | `InlineObjectLineHeightReason`：ExistingInterlineSpaceFitsInlineObjects / InlineObjectInterlineCollision |

第 18 项的 `ResolvedKinsoku.reason` 原先编码了两项信息（谁选定 + 自适应时的 measure 字数），
改为 `source: KinsokuPolicySource` 加 `adaptive_measure_cells: Option<i32>`；
`Fixed` 与 `MeasureAdaptiveKinsoku` 两处 `format!` 删除。
`KinsokuDecisionInfo` 同步携带这两个字段，dump 侧的
`Fixed:{level}{+Hang}` / `MeasureAdaptiveKinsoku:{n}字→{level}{+Hang}` 文本需自行拼装。

分配次数 55,536 → 55,205（-331）。这一批降幅明显大于前面几项，因为
`ResolvedKinsoku` 的 `format!` 原先每次布局都无条件执行，包括 `Fixed` 分支里的
`{level:?}` 格式化。

### P5 完成汇总

第 1–4 项与第 10–22 项全部完成；第 5–9 项按上表保留文本。逐项实测：

| 阶段 | alloc_calls | 相对上一阶段 |
| --- | ---: | ---: |
| P4 终值 | 56,403 | — |
| 第 1–3 项 | 56,093 | -310 |
| 第 4 项 | 56,093 | 0 |
| 第 22 项 | 55,694 | -399 |
| 第 15 项 | 55,675 | -19 |
| 第 16 项 | 55,672 | -3 |
| 第 10、11 项 | 55,634 | -38 |
| 第 12 项 | 55,550 | -84 |
| 第 13、14 项 | 55,536 | -14 |
| 第 17–21 项 | 55,205 | -331 |
| **P5 合计** | **55,205** | **-1,198** |

realloc_calls 全程保持 2,065；debug 记录数全程保持 7,395，未因类型替换增减。

**P0 基线 → 最终**

| 指标 | 基线 | 最终 | 变化 |
| --- | ---: | ---: | ---: |
| alloc_calls（热路径） | 76,451 | 55,205 | **-27.8%** |
| realloc_calls | 2,331 | 2,065 | -11.4% |
| allocated_bytes | 11,287,705 | 10,613,767 | -6.0% |
| debug 记录数 | 7,395 | 7,395 | 0 |

**被撤销的旧类型名**

第 10 项在回滚时的四个临时类型名（`KinsokuPosition`、`Uax14Rule`、
`ImpossibleMeasureFallback`、`ContextualKinsokuRule`）中，三个重做时保留，
`KinsokuPosition` 改为 `KinsokuForbiddenPosition`，与其记录的语义（禁则位置）更贴合；
`ImpossibleMeasureFallback` 与 `ContextualKinsokuRule` 沿用原名。

**剩余 `String` 形态的字段**

`src/` 中仍有 `String` 类型的 reason 类字段，全部属于第 5–9 项
（可替换的 backend / resolver / analyzer trait），以及以下不属于本轮范围的部分：

- `FontDecisionInfo.substitution_reason`：由 `CjkPunctuationGlyphSubstitutionReason`
  在 dump 边界拼装码点标签，属于展示层文本；
- `ShapingDecisionInfo.source` / `reason`：含 `format!("WhiteSpace:{:?}:{:?}")` 与
  `format!("WhiteSpaceToSpace:{}")` 的前缀拼接，来源是 backend 返回的决策；
- `RoleOverrideInfo.source` / `reason`：与第 6 项同理，来自可替换 resolver。

## 测试与 dump 适配

适配范围：`tests/` 全部测试加 `src/` 内单元测试，共 60 个文件、约 470 处编译错误。

### dump 侧的映射策略

`tests/fixture_layout/dump.rs` 需要把枚举还原成类型化之前的文本。
判定依据是「该枚举的 `Debug` 输出是否等于原文本」：

- **变体名与原文本一致的**（`FontRole`、`PunctuationClass`、`PunctuationAnchor`、
  `BaselineClass`、`MetricBox`、`LastLineAlignment`、`DecorationKind`、
  `RepairKind`、`RepairReasonCode`、`ProgressiveBreakTier` 等）直接用 `{:?}`。
  这类占绝大多数。
- **原文本是组合形式的**（`KinsokuDecisionInfo` 的
  `Fixed:{level}{+Hang}` / `MeasureAdaptiveKinsoku:{n}字→{level}{+Hang}`、
  `ContextualKinsokuRule` 的 `Uax14WesternPunctuationBoundary:{rule}`、
  `SpacingReason` 的 `AttachedInlineVirtualPunctuationBoundary:{case}`、
  `DecorationSegmentReason` 的 `;AdjacentInterlinearLineShortening` 后缀）
  需要 dump 自行拼装。
- **原文本是小写的**（`AutoSpaceSide` 的 `leading`/`trailing`/`gap`）需要显式映射。
- **字段被拆成独立成员的**（`DecorationSegmentInfo.adjacent_shortened`、
  `PunctuationDecisionInfo.geometry_forced_half_width`、
  `KinsokuDecisionInfo.source` + `adaptive_measure_cells`）
  需要 dump 重新组合。

映射函数全部放在 `dump.rs` 内，`src/` 不提供同义函数：
`kinsoku_reason_text`、`preferred_stretch_kind_name`、`segment_reason_text`、
`glue_kind_name`、`auto_space_side_name`。

### 适配中发现并修复的两处行为回归

适配过程中 golden 出现差异，定位到 P3 引入的两处行为变化，均已修复：

**1. role 限制下的替换原因判定错误**
提交 `adebc9f` 把 `substitute_for_role` 拆成 `display_text_for_role` 与
`reason_for_role` 时，新的 `reason_for_role` 把两种情况混在一起：
非 `CjkPunctuation` 角色一律返回 `RolePreserved`。

原语义是两步判定：先算 policy 层结果，只有「policy 确实要替换码点、而角色又不是
`CjkPunctuation`」时才回退成 `RolePreserved`。因此：

- `中文`（policy 不替换任何码点）+ `CjkText` 角色 → `PolicyPreserved`
- `…`（policy 会替换成 `⋯`）+ `LatinText` 角色 → `RolePreserved`

新实现把第一种也判成 `RolePreserved`，导致 golden 里大量 `sub=` 文本从
`CjkPunctuationGlyphPolicy:...:preserve` 变成
`CjkRoleGatedDisplaySubstitution:preserve-role-...`。

修复方式：`reason_for_role` 增加 `role` 参数并恢复两步判定；
新增 `display_text_for_role_with_candidate` 表达「候选显示文本 + role 限制」，
`policy_display_text` 提升为 `pub`。

**2. dump 的 `display_text` 粒度错误**

同一提交让 dump 从 shaping 记录的 `substitutions` map 取 `display_text`，
但该 map 以 shaping segment 为 key，而 `FontDecisionInfo` 以 font resolution
的整段范围为 key。跨字体的段落因此被截断：`44(10):21-38.` 被取成 `(10):`。

修复方式：dump 侧按整段范围重新走一次 role 限制的显示文本解析，
并为此在 `LayoutDebugStageInput` 加回 `punctuation_glyph_substitutor` 字段
（该字段在 `origin/` 中存在，P3 移植时丢失）。

**这两处说明 golden 零 diff 不足以证明类型替换无行为变化；它同时也是发现行为回归的手段。**

### 断言删除与语义调整

| 文件 | 调整 | 原因 |
| --- | --- | --- |
| `line_adjustment_stage_coverage_test.rs` | `reason != "LineEdgeWordSpaceCollapse"` 改为 `reason != TextAutoSpaceLineEdgeTrim` | 前者的产生点已在提交 `5e90140` 删除，变体不再建立 |
| `justifier_coverage_test.rs` | 原用任意字符串 `"token"` 作 emergency reason，改为具体变体 `LongMixedAlphaNumericIdentifier` | 类型化后不再能传任意值；断言同步改为完整枚举匹配 |
| `line_break_repair_engine_test.rs` | `reason.starts_with("TerminalTechnicalEmergencyTracking")` 保留 `starts_with` 形式（走 `name()`） | 该值可能是 `Stage(...)` 或 `Opportunity(...)` 两种，枚举匹配无法覆盖 |

### 批量处理与人工核对的边界

约 470 处错误中，约 380 处是「字符串字面量与枚举比较」的机械模式，
用脚本批量替换；剩余约 90 处需要判断语义。

批量阶段产生过一次错误：按字段名（`reason`、`kind`、`side`）加 `.name()`
误伤了 55 处仍是 `String` 的字段（`RoleOverrideInfo.reason`、
`ShapingDecisionInfo.reason` 等），已全部回退。
字段名不能作为类型判定依据：`reason` 在不同结构体上分别是
`String`、`SpacingReason`、`JustificationAllocationReason`、`ContextualKinsokuRule`。

人工核对阶段发现 5 处子代理误判（均为 `.name()` 之外的自创断言）：

- `punctuation_model_coverage_test.rs`：把原 `geometry_source.ends_with("FixedHalfWidth")`
  改成断言 `PolicyDerived`，并自行加了该断言。原测试只断言「强制半宽」，
  正确改法是断言 `geometry_forced_half_width: bool`。
- `line_repair_coverage_test.rs`：额外加了原测试没有的 `RepairDetail::PushedIn` 断言。
- `line_adjustment_stage_coverage_test.rs`：把排除一个原因改成了排除两个。
- `justifier_coverage_test.rs`：把任意字符串改成具体变体但未同步断言文本。
- `line_break_repair_engine_test.rs`：把 `starts_with` 改为精确枚举匹配，
  而实际值有两种可能。

这 5 处已全部按原断言语义修正。

### 适配后的验证结果

| 项目 | 结果 |
| --- | --- |
| `cargo check --all-targets` | 0 错误 |
| `cargo test` | 1,435 个测试全部通过 |
| `layout_fixture_golden_test` | 通过，golden 逐字节一致 |
| alloc_calls（宽度 672） | 55,233 |
| debug 记录数 | 7,395（全程未变） |

分配次数比 P5 的 55,205 多 28 次，来自 `reason_for_role` 的两步判定与
`policy_display_text` 在 dump 侧的调用。这 28 次换来的是行为正确性，
且只发生在 debug 汇编路径。

剩余警告均为适配前既有的测试辅助代码问题
（`deterministic_stub_font_backend.rs` 中未被使用的构造函数与辅助函数），
与本次改动无关。

## 后续：运行时事实与 debug 结构的分离

本轮收尾时发现，`LayoutResult.debug` 同时承担诊断记录与运行时数据读取两件事。
逐字段核查后确认，`core/layout_queries.rs`、`core/layout_paint_bounds.rs` 与
`core/layout_result_replay_index.rs` 共读取 10 个 debug 集合里的几何数值。

这件事已拆为独立迭代：
[`2026-10-24-refactor-runtime-facts-out-of-debug.md`](2026-10-24-refactor-runtime-facts-out-of-debug.md)，
内容包括消费点清单、上游决策记录的计算依赖、新字段设计与分阶段验证。
其中也记录了 `MetricDecisionInfo.reason` 的单项测量数据。

本轮的第 5–9 项（来自可替换 trait 的 reason 文本）在那份文档里标为待重审：
若默认构建不再构造 `debug`，这些字段的成本只发生在诊断构建中。
