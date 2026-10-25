# 2026-10-24 把运行时数据从 debug 结构中分离

## 目标

当前 `LayoutResult.debug` 同时承担两件事：

1. 解释布局决策的诊断记录；
2. 绘制与查询需要读取的数值。

第二类用途是从第一类里「顺手」读出来的：`core/layout_queries.rs` 与
`core/layout_paint_bounds.rs` 的若干查询直接读 `debug` 集合里的几何字段，
因为这些字段就在同一条记录里。随着诊断记录不断增加，运行时路径被动承担了
debug 的构造与内存成本。

本轮把这些运行时数据移到与 `debug` 平级的新字段，使运行时路径不再依赖
`debug`；然后把 `debug` 的构造变成默认关闭的诊断能力。

验收指标按顺序为：

1. 行为一致（查询、绘制与溢出输入逐值相同）；
2. 运行时间（`paragraph-layout-bench` 的 `layout`）；
3. 每次布局的分配次数与累计字节；
4. 结果存活时的窗口保留内存。

分成两个阶段，因为两个阶段的目标与验收不同：

- **阶段一（迁移）**：按 `refactor` 验收，行为一致是主要指标，性能只作为
  回归约束。这一步本身会增加一份运行时副本，不追求性能。
- **阶段二（开关）**：按 `perf` 验收，默认构建不再构造 `debug`，
  以实测的成本下降为主要指标。

两个阶段分开提交、分开验收。阶段二依赖阶段一，但阶段一不依赖阶段二。
本迭代同时包含两个阶段，最终交付的是两项能力：运行时数据脱离 debug，
以及诊断默认关闭。

文档主分类取 `refactor`，按迭代文档规则的判断顺序：最先满足的是
「只调整内部结构、模块边界、所有权或调用路径」。阶段二确实带来性能收益，
但它的目标是让诊断不再成为默认成本，与「保持行为不变、压缩既有计算」的
`perf` 目标不同。两阶段的序号也说明阶段一是前提。

## 现状依据

### 实际读取 `debug` 的功能代码

读取 `LayoutResult.debug` 的位置分两类。两类都是仓库内的运行路径，
不只是测试或外部兼容需求。

**`src/` 内的查询 API**

| 模块 | 读取的集合 |
| --- | --- |
| `core/layout_queries.rs` | `ruby_decisions`、`bopomofo_decisions`、`metric_decisions`、`decoration_segments`、`geometry_decisions`、`auto_space_decisions`、`line_white_space_adjustments` |
| `core/layout_paint_bounds.rs` | `decoration_decisions`、`ruby_decisions` |
| `core/layout_result_replay_index.rs` | `font_decisions` |

**`examples/paragraph_demo` 内的绘制与溢出计算**

| 读取点 | 读取的字段 |
| --- | --- |
| `renderer.rs:121` | `ruby_decisions` 的 `center_x`、`width`、`font_size`、`baseline_y`、`glyphs` |
| `renderer.rs:145` | `bopomofo_decisions` 的 `placements`（含 `glyphs`、`font_size`、`draw_x`、`baseline_y`） |
| `renderer.rs:201` | `decoration_decisions` 的 `applied`、`kind`、`cluster_range`、`anchor_x`、`anchor_y`、`dot_diameter` |
| `renderer.rs:220` | `decoration_segments` 全字段 |
| `app.rs:113` | `ruby_decisions` 的 `glyphs`（含 `bounds`）、`ascent`、`descent` |
| `app.rs:137` | `bopomofo_decisions.placements` 的 `glyphs`、`left`、`top`、`width`、`height` |
| `app.rs:155` | `decoration_decisions` 的同一组字段 |
| `app.rs:164` | `decoration_segments` 的 `left`、`top`、`right`、`bottom` |

这张表说明一件事：绘制需要的不止查询读的那几列。ruby 需要 `glyphs` 与
`font_size`；bopomofo 的几何全部由 `placements` 表达，没有 `center_x`/`width`
这类统一字段；着重号需要 `kind` 才能关联富文本颜色。把这些集合的来源
当作「只有查询需要的几列」会漏掉绘制。

因此，把消费者按三个维度分开记录：查询需要的列、绘制需要的列、溢出计算需要的列。

### 上游决策记录的计算依赖

「最终 `LayoutDebugInfo` 不被回读」不能推出「同类型的上游记录只写不读」。
已核实的反例：

| 上游记录 | 读取点 | 影响 |
| --- | --- | --- |
| `ShapingResult.decisions` 的 `capability_issue` / `missing_glyphs` | `paragraph_shaping_stage.rs:141`、`:149` | 决定是否放弃显示替换并重新 shaping |
| `ShapingResult.decisions` 的 `reason` | `paragraph_shaping_stage.rs:116` | `WhiteSpaceToSpace` 分支改写 reason 文本 |
| `emergency` 局部集合 | `paragraph_shaping_stage.rs:412`、`:506` | 按 `range` + `reason` 去重 |
| `emergency_tracking_eligibility_decisions` | `line_break_planning_stage.rs:558` 起 | range 与 reason 构建 emergency tracking 边界，进入 justification |
| `contextual_kinsoku_decisions` | `line_adjustment_stage.rs:67-105` | 按 range 与 forbidden position 去重，并按 reason 选 fallback |

这些上游记录不属于本轮改动范围：它们参与布局计算，本轮只处理
`LayoutResult` 上作为最终产物的 debug 集合。

### 各集合的规模

宽度 672 的 demo 页，每页 47 次布局。数据来自
`paragraph-layout-bench` 的 `DebugRecordCounts`，该统计包含
`justification_decisions` 内嵌套的 allocations，不统计各 `Option` 字段：

| 集合 | 每页记录数 |
| --- | ---: |
| `font_decisions` | 2,104 |
| `metric_decisions` | 2,104 |
| `shaping_decisions` | 2,056 |
| `white_space_units` | 181 |
| `geometry_decisions` | 168 |
| `punctuation_decisions` | 168 |
| `line_decisions` | 82 |
| `line_edge_trim_decisions` | 42 |
| `justification_decisions` 及其 allocations | 23 + 399 |
| `decoration_decisions` | 19 |
| `role_overrides` | 11 |
| `spacing_decisions`、`line_white_space_adjustments` | 各 7 |
| `auto_space_decisions`、`inline_box_decisions`、`break_opportunity_decisions` | 各 3 |
| `decoration_segments` | 3 |
| `ruby_decisions`、`bopomofo_decisions` | 各 2 |
| `mandatory_break_decisions` | 1 |
| `inline_object_decisions` | 0 |
| 合计 | 7,395 |

`font_decisions` 与 `metric_decisions` 来自同一份 `font_resolutions`
（`line_break_planning_stage.rs:186-206`），两者相加 4,208 条。
`geometry_decisions` 按原 cluster 顺序，为存在 punctuation geometry 的
cluster 各产出一条（`to_decision_info()` 用 `filter_map`，没有 atom 的 cluster 跳过）。

## 收益量级

上界估算方式：在 `build_layout_debug_info()` 入口临时加环境变量开关，令其直接返回
`LayoutDebugInfo::default()`。该探针已删除。基线为恢复全部字段后的源码。

这是一次跳过最终装配的破坏性探针，有两个方向的偏差：它同时删掉了绘制仍需要的
注音与装饰集合（高估），又没关闭上游构造与两处直接写入（低估）。
下表只说明「最终 debug 装配」这一段的量级。

| 指标（宽 672） | 当前 | 探针 | 变化 |
| --- | ---: | ---: | ---: |
| `layout` median | 2.526 ms | 2.208 ms | -12.6% |
| `alloc_calls` | 55,205 | 48,617 | -6,588（-11.9%） |
| `realloc_calls` | 2,065 | 1,916 | -7.2% |
| `allocated_bytes` | 10,613,767 | 9,201,154 | -13.3% |
| `window_live` | 2,378,110 | 493,688 | -79.2% |
| debug 记录数 | 7,395 | 188 | — |

`window_live` 是布局窗口内的净分配量（窗口内累计分配减释放），采样时整页
`LayoutResult` 尚未释放，因此混有结果自身与缓存的成本。

计时为 A/B/A（有 2.526 → 无 2.208 → 有 2.542），包含一次「无」的测量。
两次「有」相差 0.6%，观测到约 12% 的差值。

## 实施实测

`debug-info` 关闭的生产构建与开启的诊断构建，各自单独测量。
基准为宽 672、`--warmup 20 --iterations 100`（计数）与 `--iterations 200`（计时）。

| 指标 | Phase 0 基线（改造前） | Phase 5 迁移完成 | Phase 6 生产构建（debug 关闭） |
| --- | ---: | ---: | ---: |
| `layout` median | 2.348 ms | 2.480 ms | **2.184 ms** |
| `alloc_calls` | 55,233 | 55,599 | **48,805** |
| `realloc_calls` | 2,065 | 2,232 | **2,064** |
| `allocated_bytes` | 10,572,103 | 10,869,939 | **9,441,075** |
| `window_live` | 2,335,927 | 2,521,475 | **683,755** |
| debug 记录数 | 7,395 | 7,395 | 0 |

Phase 5 比基线多出 +366 次分配，这是迁移阶段必然的副本成本：运行时容器与诊断容器
各持一份注音、装饰与空白数据。

Phase 6 关闭诊断后，相对基线：

| 指标 | 变化 |
| --- | ---: |
| `layout` | 2.348 → 2.184 ms（-7.0%） |
| `alloc_calls` | 55,233 → 48,805（-11.6%） |
| `allocated_bytes` | 10,572,103 → 9,441,075（-10.7%） |
| `window_live` | 2,335,927 → 683,755（-70.7%） |

`window_live` 降幅最大：诊断记录挂在 `LayoutResult` 上整个窗口都存活，
关闭后每页释放约 1.65 MB。

计时 -3.7% 是单次 A/B 差值，分配次数与字节为确定性指标。

### 测量注意事项

`Cargo.toml` 的 dev-dependency 自引用 `tiqian = { path = ".", features = ["debug-info"] }`
使所有 dev target（含 `examples/`）都启用该 feature，因此用 benchmark 测生产路径时
需要临时注释掉这一行。测试与 golden 需要诊断记录，因此该行在正常工作区状态下保留。

## 设计

### 设计原则

**不做逐字段裁剪。** 排版产出的几何与文本数据多数有潜在用途，demo 没有用到
不等于消费方不需要。若按现有读取点逐列裁剪，会得到一个只服务于本仓库现有
实现的窄接口，后续每个新需求都要回头扩字段。

本轮只解决一件事：**这些数据不该放在 `debug` 里**。它们的形状由「排版库应该
对外暴露什么」决定，不由「现有消费方读哪一列」决定。

具体做法：`LayoutResult` 增加一个与 `debug` 平级的字段，装运行时产物。
容器内按语义分组，每组沿用或提炼现有决策记录的类型，不新建窄副本。

### 容器

```rust
pub struct LayoutResult {
    pub input: LayoutInput,
    pub size: Size,
    pub clusters: Vec<Cluster>,
    pub glyph_runs: Vec<GlyphRun>,
    pub lines: Vec<LineBox>,
    /// 排版产出的运行时数据：查询、绘制与几何计算消费。
    pub layout: LayoutOutput,
    pub debug: LayoutDebugInfo,
}
```

命名：`layout` 与 `debug` 平级，都是名词。容器类型叫 `LayoutOutput`，
说明它是「排版的产出」，不暗示只服务于某类消费者。不用 `Facts`
（不说明内容）、不用 `Info`（与 `*DecisionInfo` 混淆）、
不用 `Metrics`（注音与装饰不是度量）。

### 分组

| 字段 | 类型 | 来源 | 每页条数 |
| --- | --- | --- | ---: |
| `font_metrics` | `Vec<ResolvedFontMetrics>` | `metric_decisions` | 2,104 |
| `font_roles` | `Vec<ResolvedFontRole>` | `font_decisions` | 2,104 |
| `clusters` | `Vec<ResolvedClusterGeometry>` | `geometry_decisions` | 168 |
| `punctuation` | `Vec<PunctuationAtom>` | `punctuation_atoms` | 168 |
| `spacing` | `Vec<PunctuationSpacingAdjustment>` | `spacing_plan` | 7 |
| `line_edge_trims` | `Vec<LineEdgeTrimDecisionInfo>` | 同名 | 42 |
| `annotations` | `AnnotationGeometry` | 见下 | — |
| `decorations` | `DecorationGeometry` | 见下 | — |
| `inline_objects` | `Vec<InlineObjectGeometry>` | `inline_object_decisions` | 按内容 |
| `white_space` | `WhiteSpaceGeometry` | 见下 | 188 |

### 各组类型

**字体与度量**

```rust
/// 一个 font resolution 范围解析出的字体角色。
pub struct ResolvedFontRole {
    pub range: TextRange,
    pub role: FontRole,
    pub face: FontFaceId,
}

/// 一个 font resolution 范围解析出的行内度量。
pub struct ResolvedFontMetrics {
    pub range: TextRange,
    pub face: FontFaceId,
    pub role: FontRole,
    pub ascent: f32,
    pub descent: f32,
    pub baseline_class: BaselineClass,
    pub metric_box: MetricBox,
    pub source: FontMetricSource,
}
```

分成两个集合的理由：字体角色与度量是两次独立解析，replay index 读前者的
`role`，富文本几何读后者的度量。合并会隐藏「font debug 装配在
`selected_attempt()` 缺失时跳过记录」与「metric 路径无条件 map」的差异，
使 replay 原本的 `None` 变成 `Some(role)`。

`ResolvedFontMetrics` 保留 `baseline_class` 与 `source`：它们是字体度量的一部分，
调用方可能用于垂直对齐或诊断来源判断，不属于诊断专用字段。

**cluster 几何与标点**

```rust
/// 一个 cluster 解析后的行内几何。
pub struct ResolvedClusterGeometry {
    pub range: TextRange,
    pub base_advance: f32,
    pub body_width: f32,
    pub leading_glue: GlueSpan,
    pub trailing_glue: GlueSpan,
    pub justification_delta: f32,
    pub ruby_spread: f32,
    pub glyph_inline_shift: f32,
    pub resolved_advance: f32,
}

/// 一侧 glue 的自然量与已消耗量。
pub struct GlueSpan {
    pub natural: f32,
    pub consumed: f32,
}
```

`GlueSpan` 替代 `geometry_decisions` 的四个平铺字段。这是本组唯一的改写，
理由是两个量成对出现、算法总是成对读取，与字段数量无关。

`punctuation` 与 `spacing` 直接复用既有的 `PunctuationAtom` 与
`PunctuationSpacingAdjustment`。它们在 `layout` 模块内已是稳定的值类型，
有自己的 `Glue`、`anchor`、`geometry_source` 等完整语义，不需要再包一层。

**注音**

```rust
pub struct AnnotationGeometry {
    pub ruby: Vec<RubyGeometry>,
    pub bopomofo: Vec<BopomofoGeometry>,
}
```

`RubyGeometry` 与 `BopomofoGeometry` 就是现有的 `RubyDecisionInfo` 与
`BopomofoDecisionInfo` 重命名：去掉噪声词 `DecisionInfo`，因为这里的记录
属于几何产出而非决策解释。字段一个不动，含 `glyphs`、`font_families`、
`font_weight`、`locale`、`overhang`、`placements`。

不裁剪的理由：绘制读 `glyphs`，复制导出读 `text`，前端可能读 `font_families`
自己重排注文，`overhang` 是避让计算的输入。这些都是接口的一部分。

**装饰**

```rust
pub struct DecorationGeometry {
    pub emphasis_dots: Vec<EmphasisDotGeometry>,
    pub segments: Vec<DecorationSegmentGeometry>,
}
```

`DecorationSegmentGeometry` 是现有 `DecorationSegmentInfo` 重命名，
字段不动。

`EmphasisDotGeometry` 是目前没有的独立类型。现在着重号的绘制锚点藏在
`DecorationDecisionInfo` 里，与 `reason: EmphasisDotReason`（诊断说明）
混在一条记录中。拆分方式有两种，见「待决策项 D1」。

**空白**

```rust
pub struct WhiteSpaceGeometry {
    pub units: Vec<WhiteSpaceUnit>,
    pub line_adjustments: Vec<LineWhiteSpaceDecisionInfo>,
}
```

两者都直接复用现有类型。`white_space_units` 目前只写进 debug，查询不读，
但它是排版产出的空白单元记录，与 `line_adjustments` 成对，放进 `layout`。

### 产生路径

运行时数据在诊断装配之前产生，诊断从同源结果投影。不能反过来从装配完成的
`debug` 复制，否则构造链仍依赖 debug，阶段二无法关闭。

上游值在 `line_adjustment_stage.rs` 的 `finish_paragraph_layout` 内，该函数在调用
`build_layout_debug_info()` 之前就已持有全部所需输入：

| 组 | 上游值 | 位置 |
| --- | --- | --- |
| `font_metrics` | `plan.metric_decisions` | `finish_paragraph_layout` 内 |
| `font_roles` | `prep.font_resolutions` | 同上 |
| `clusters` | `final_geometry`（`PunctuationGeometryLedger`） | 同上 |
| `punctuation` | `prep.punctuation_atoms` | 同上 |
| `spacing` | `prep.spacing_plan` | 同上 |
| `line_edge_trims` | `resolve_auto_space_edge_trims()` 返回值 | 同上 |
| `annotations` | `resolve_annotation_geometry()` 返回值 | 同上 |
| `decorations` | 同上 | 同上 |
| `white_space` | `prep.white_space_units`、`line.white_space_adjustments` | 同上 |

`geometry_decisions` 在别处也被用到：`resolve_annotation_geometry` 需要它，
`build_layout_debug_info` 也需要它。因此该值必须在使用前就构造好，
或允许共享（`Arc`），不能让 debug 装配成为运行时数据的唯一制造者。

**待决策项 D2** 涉及这里的共享方式。

### 消费点迁移

| 消费点 | 现在读 | 改为读 |
| --- | --- | --- |
| `layout_result_replay_index.rs:88-106` | `debug.font_decisions` | `layout.font_roles` |
| `layout_queries.rs:818` | `debug.metric_decisions` | `layout.font_metrics` |
| `layout_queries.rs:823`、`:830` | `debug.metric_decisions` | `layout.font_metrics` |
| `layout_queries.rs:1360` | `debug.metric_decisions` | `layout.font_metrics` |
| `layout_queries.rs:1293-1321` | `debug.geometry_decisions` | `layout.clusters` |
| `layout_queries.rs:1690` | `debug.geometry_decisions` | `layout.clusters` |
| `layout_queries.rs:1698` | `debug.auto_space_decisions` | `layout.white_space.line_adjustments` 与 autospace |
| `layout_queries.rs:1705` | `debug.line_white_space_adjustments` | `layout.white_space.line_adjustments` |
| `layout_queries.rs:1906-1912` | `debug.geometry_decisions` | `layout.clusters` |
| `layout_queries.rs:1921` 起 | `debug.ruby_decisions` | `layout.annotations.ruby` |
| `layout_queries.rs:349`、`:352` | `debug.ruby_decisions`、`debug.bopomofo_decisions` | `layout.annotations` |
| `layout_queries.rs:969`、`:982` | `debug.decoration_segments` | `layout.decorations.segments` |
| `layout_paint_bounds.rs:88` | `debug.decoration_decisions` | `layout.decorations.emphasis_dots` |
| `layout_paint_bounds.rs:96` | `debug.ruby_decisions` | `layout.annotations.ruby` |
| `renderer.rs:121`、`:145`、`:201`、`:220` | `debug` 四集合 | `layout.annotations`、`layout.decorations` |
| `app.rs:113`、`:137`、`:155`、`:164` | 同上 | 同上 |

迁移完成后，`src/` 与 `examples/` 都不再功能性地读取 `debug`。

### 保持不变的部分

`debug` 内的全部集合与字段保持原样。它们是诊断记录，与运行时数据同源
但不是同一份数据，本轮不合并、不删减。

因此阶段一的总记录数是「保留原 debug，再新增运行时记录」，
并非「4,208 降到 2,104」。新增的 `Vec`、数值拷贝、两份注音与装饰数据都会增加
分配和存活字节。这是阶段一可以接受的成本：阶段二关掉 `debug` 后，
这部分副本就是唯一的运行时开销，总额低于现在的 debug。

## 待决策项

以下四项需要在实施前定下来。此前文档把这些留白，导致实施时只能临时决定，
属于流程缺陷。

### D1：着重号的诊断与几何如何分离

**决策：两处各存一份。**

新建 `EmphasisDotGeometry` 装绘制锚点（`cluster_range`、`kind`、`applied`、
`anchor_x`、`anchor_y`、`dot_diameter`），放进 `layout.decorations.emphasis_dots`。
`debug.decoration_decisions` 原样保留全部字段，dump 逻辑不改。

每页 19 条，副本约 0.8 KB，作为交付开销接受。

### D2：`geometry_decisions` 在两条路径间如何共享

**决策：两份独立产出。**

`PunctuationGeometryLedger` 新增 `to_resolved_geometry()` 产
`Vec<ResolvedClusterGeometry>`，归 `layout.clusters`；原 `to_decision_info()`
继续供 debug 装配。两者互不引用。

每页 168 条，副本约 6.7 KB，作为交付开销接受。

### D3：`font_roles` 与 `font_metrics` 是否分开

**决策：分开。**

`FontResolution::selected_attempt()` 的实现是
`attempts.iter().find(|attempt| attempt.face == self.face)`，`attempts` 为空
或不含选定 face 时返回 `None`。`FontBackendShapingResult::new` 只对此情形
打 warn、不阻止构造，因此该状态可达。

`font_roles`（replay 用的 `range`、`role`、`face`）与 `font_metrics`
（垂直几何用的度量）保持两个集合。

### D4：`PunctuationAtom` 与 `PunctuationSpacingAdjustment` 是否直接公开

**决策：两者都直接复用。**

它们已经是 `Clone + Debug + PartialEq` 的不可变值类型，语义完整。
直接放进 `LayoutOutput`，字段一个不改，含 `Glue` 的 `min`/`natural`/`max`/
`priority`/`penalty` 与 `SpacingReason`。

这些量描述标点的压缩与拉伸预算，是排版库应当对外提供的几何信息。

## 实现阶段

按依赖顺序拆成 7 步，每步可独立编译与验证。

**Phase 0：基线**

决策已定（D1–D4），本步只做两件事：

- 跑 `cargo test --all-targets` 与 `cargo run --release --example paragraph-layout-bench`
  存档当前四项指标；
- 跑 `cargo run --example paragraph-demo --release` 确认基线可正常渲染。

产出：基线数据写入「收益量级」一节。

**Phase 1：容器与类型定义**

- 新增 `LayoutOutput`、`ResolvedFontRole`、`ResolvedFontMetrics`、
  `ResolvedClusterGeometry`、`GlueSpan`、`AnnotationGeometry`、
  `DecorationGeometry`、`WhiteSpaceGeometry`、`EmphasisDotGeometry`；
- `RubyDecisionInfo`、`BopomofoDecisionInfo`、`DecorationSegmentInfo` 重命名；
- `LayoutResult` 加 `layout: LayoutOutput` 字段，构造函数补默认值。

不改任何逻辑，`debug` 不动。

验证：`cargo check --all-targets` 通过；行为零变化。

**Phase 2：产生路径**

- 在 `finish_paragraph_layout` 内、`build_layout_debug_info()` 之前构造
  `LayoutOutput` 的九个分组；
- `PunctuationGeometryLedger` 新增 `to_resolved_geometry()`，与 `to_decision_info()`
  各自独立（D2）；
- `LayoutResult` 构造改用同时接收两者的新构造函数。

验证：`cargo check --all-targets` 通过；`debug` 内容与 Phase 1 逐字节相同
（golden 不变）。

**Phase 3：`src/` 消费点迁移**

按「消费点迁移」表逐一改 `layout_queries.rs`（9 处）、
`layout_paint_bounds.rs`（2 处）、`layout_result_replay_index.rs`（1 处）。

每改一处跑一次相关测试。全部改完后 `src/` 内除 dump 外不再有 `debug` 读取。

验证：`cargo test --lib` 通过；旧／新查询差分逐值一致。

**Phase 4：`examples/` 消费点迁移**

`renderer.rs` 四处、`app.rs` 四处。同时确认 `renderer.rs:952`、`:971`
与 `font_backend.rs:1073` 的测试断言是否应改（它们是测试，可以继续读 debug）。

验证：`cargo run --example paragraph-demo --release` 目视一致；
`cargo test --all-targets` 通过。

**Phase 5：独立性验证**

- 清空 `debug` 后重建 replay index、跑全部查询与 demo 绘制输入，输出一致；
- 匹配语义用例：混合字号、ideographic reference 非首项与缺失、
  resolution 与 cluster 范围不一致、行首 autospace 移除、ruby spread、
  bopomofo placement、`max_lines` 截断。

验证：上述全部通过，且 `cargo test --all-targets` 通过。此步通过后阶段一完成，
可以提交。

**Phase 6：debug 开关**

- `Cargo.toml` 加 `debug-info` feature；
- `LayoutResult.debug` 字段本身、`with_debug()`、`with_layout_and_debug()` 与
  诊断装配入口都用 `#[cfg]` 包裹，关闭时字段不存在，不只置空；
- 关闭时同时跳过 `LayoutDebugStageInput` 的构造，避免为丢弃的装配求值 clone；
- CI 两个构建都跑：默认（关闭）与 `--features debug-info`（开启）。

验证：默认构建不存在 `debug` 字段且功能输出与开启构建逐值相同；开启构建 golden
逐字节一致；四个基准指标各测一次。

**Phase 7：文档收尾**

- 更新本文档的实施状态与实测数据；
- 判断 `docs/architecture.md` 是否需要记录 `LayoutOutput` 的公开形状。

## 实施状态

Phase 0–7 全部完成。

### 新增类型

`src/core/layout_model.rs`：

| 类型 | 用途 |
| --- | --- |
| `LayoutOutput` | 运行时数据容器，九个分组字段 |
| `ResolvedFontRole` | resolution 范围的字体角色与面 |
| `ResolvedFontMetrics` | resolution 范围的已解析度量 |
| `ResolvedClusterGeometry` | cluster 的行内几何 |
| `GlueSpan` | 一侧 glue 的 `natural`/`consumed` 与 `remaining()` |
| `AnnotationGeometry` | 注音几何容器 |
| `DecorationGeometry` | 装饰几何容器 |
| `EmphasisDotGeometry` | 着重号绘制锚点 |
| `WhiteSpaceGeometry` | 空白单元、逐行调整与 autospace |

重命名：`RubyDecisionInfo` → `RubyGeometry`、`BopomofoDecisionInfo` → `BopomofoGeometry`、
`DecorationSegmentInfo` → `DecorationSegmentGeometry`。

`LayoutResult` 新增 `layout: LayoutOutput` 字段与 `with_layout_and_debug()` 构造函数。

### 与设计的三处偏差

1. `WhiteSpaceGeometry` 增加 `auto_space` 字段。设计时把它算在 `line_adjustments` 名下，
   实施时发现 `layout_queries.rs:1698` 读的是 `AutoSpaceDecisionInfo` 的 `side` 与
   `total_reduction`，与 `LineWhiteSpaceDecisionInfo` 是不同结构，需要独立字段。
2. `DecorationGeometry.segments` 的元素类型是 `DecorationSegmentGeometry`（重命名后的
   原类型），字段未裁剪，与 D1 的「两处各存一份」一致。
3. `LayoutOutput` 的逐行空白几何不能从 `debug` 复制：关闭 feature 时 `debug` 不存在。
   因此把「逐行空白调整」的循环抽成 `resolve_line_white_space()`，功能路径与诊断装配
   共用同一份计算。这是在实施中才发现的结构耦合：原先的设计把 `debug` 当作中间的
   数据来源，与「关闭时字段消失」互相矛盾。

### 阶段拆分

feature 开关完成时，`line_adjustment_stage.rs` 达到 1007 行，
`finish_paragraph_layout` 单函数约 700 行，混了行调整、结果装配、诊断装配三件事。
关闭 feature 时还要让 `LayoutOutput` 从上游重算一份空白几何，
两件事实际已不可分，因此同轮拆成两个阶段：

| 文件 | 职责 |
| --- | --- |
| `line_adjustment_stage.rs` | 行调整：push-in、hyphen 补齐、edge trim、两端对齐、technical 重试判定 |
| `layout_assembly_stage.rs`（新增） | 结果装配：glyph run、行盒、注音几何、`LayoutOutput`、`LayoutResult` |
| `layout_debug_assembly.rs` | 诊断记录：类型定义与 `build_layout_debug_info()` |
| `line_geometry_stage.rs` | 行几何：新增 `build_glyph_runs()`、`center_dash_ink()` |

`finish_paragraph_layout` 末尾现在是三件事：算出 `LayoutAssemblyInput`、
构造 `LayoutAssemblyRequest`、调用 `assemble_paragraph_layout()`。
行调整阶段以 `LayoutAssemblyInput` 交出
`trimmed_clusters`、`final_clusters`、`justify_delta_by_cluster`、
`edge_trim_decisions`、`resolved_clusters`、`geometry_decisions`，装配阶段不再向上游回溯。

`build_glyph_runs()` 改为参数化：不再接收 `ParagraphLayoutPrep`，
改收四个具名字典。这样它只依赖 `line_geometry_stage` 已有的导入，
不需要向上引用布局准备结构，避开模块循环。

`paragraph_layout_engine.rs` 未改，调用链保持
`plan_paragraph_lines → finish_paragraph_layout → LayoutResult`。

拆分后行数：`line_adjustment_stage.rs` 677、`layout_assembly_stage.rs` 381、
`line_geometry_stage.rs` 558、`layout_debug_assembly.rs` 461，均在 1000 行以下。

### 产生路径

`layout_assembly_stage.rs` 的 `build_layout_output()` 装配运行时数据，
`build_paragraph_debug()` 装配诊断记录，两者从同一批上游值各建一份。
空白几何由 `resolve_line_white_space()` 产出，两条路径共用。

`PunctuationGeometryLedger` 新增 `to_resolved_geometry()`，与 `to_decision_info()`
各自独立（D2）。

### 消费点迁移

`src/` 与 `examples/` 共 20 处迁移完成。迁移后 `src/` 内 `.debug.` 只剩
`LayoutDebugInfoBuilder` 的写入方法，没有功能性读取。

`examples/paragraph_demo` 的渲染主路径也已改为读取运行时数据：

- `renderer.rs` 的 `paint_decorations()` 从 `debug.decoration_decisions`
  改为 `layout.decorations.emphasis_dots`。`EmphasisDotGeometry` 携带
  `applied`、`anchor_x`、`anchor_y`、`dot_diameter` 与 `kind`，
  是绘制着重号所需的全部字段，`decoration_decisions` 中的 `source_text`、
  `reason`、`id` 属于诊断用途。
- `renderer.rs` 中四处渲染测试的断言同步改为读取 `layout.annotations` 与
  `layout.decorations`，使断言与它们验证的渲染路径使用同一份数据。

迁移后在关闭 feature 的生产构建下运行 `cargo run --example paragraph-demo --release`，
窗口正常启动、无警告输出。

`examples/paragraph_demo` 内仍有三处读取 `debug`：`app.rs` 与 `font_backend.rs`
的字体面、度量来源断言。它们位于 `#[cfg(test)] mod tests` 内，
验证的是 shaping 与 font backend 的决策记录本身，属于诊断用途，保持读取 `debug`。

`tests/core/layout_output_independence_test.rs` 是新增的独立性验证，
覆盖四项：
功能查询在清空 `debug` 后输出不变、
注音与装饰几何完全来自 `layout`、
空 `layout` 时查询稳定、
`Size` 与两者无关。

### emphasis_dots 的 `id` 与行内对象分组

迁移后由下游 crate 的绘制需求反馈了两处补充，都属于绘制必需的行内几何：

1. `EmphasisDotGeometry` 增加 `id: Option<u32>`。`DecorationDecisionInfo` 本来就有该字段，
   只是未进入运行时结构。绘制方按 `(kind, id)` 匹配样式声明，同一 `kind` 的多个声明
   需要靠它区分颜色层，缺失会取错颜色。
2. 新增 `LayoutOutput.inline_objects: Vec<InlineObjectGeometry>`，
   字段为 `range`、`cluster_index`、`line_index`、`advance`、`ascent`、`descent`。
   绘制方按 `range` 回查输入声明取对象身份，按 `cluster_index` 定位落位。

### feature 开关

`Cargo.toml` 新增 `debug-info`，默认关闭。
`LayoutResult.debug` 字段本身受 feature 控制：关闭时字段不存在，不留空值。
`with_debug()`、`with_layout_and_debug()` 同样受控，另提供 `with_layout()` 供关闭态使用。
关闭时 `LayoutDebugStageInput` 的构造也整体不参与编译，不为丢弃的装配做 clone。

`dev-dependencies` 自引用 `tiqian` 并启用该 feature，使测试与 golden 能检查诊断记录。
代价是所有 dev target（含 benchmark）都启用它，测生产路径时需临时注释该行，
见「测量注意事项」。

### architecture.md

`docs/architecture.md` 是上游 Kotlin 仓库的文档，本仓库没有该文件，
因此不记录 `LayoutOutput` 的公开形状。它的字段语义由本轮的代码注释与本文档承担。

## 验证

分两个阶段，各自独立验收。

**阶段一：迁移**

golden 逐字节一致在这里不足以证明正确性：本轮保留原 debug，dump 继续读旧集合，
即使新字段没填对，golden 也可能完全不变。因此阶段一需要以下三类验收：

1. **旧／新查询差分**。对同一输入比较 positioned clusters、富文本几何、paint overhang、
   replay roles、光标、命中、选择框与复制文本。要求精确一致时比较 `to_bits()`，
   不依赖格式化后的文本。
2. **清空 debug 的独立性测试**。布局完成后清空 `debug`，再重建 replay index、
   运行全部功能查询与 demo 绘制输入，验证输出一致。必须清空后重建 index，
   避免预计算缓存隐藏依赖。
3. **匹配语义用例**。覆盖混合字体与字号、ideographic reference 非首项、
   无 ideographic reference、resolution 与 cluster 范围不一致、行首 autospace 移除、
   ruby spread、bopomofo placement，以及 `max_lines` 截断。

另外：`cargo check` 必须覆盖 examples 与相关测试 target，不能只跑 `--lib`，
否则会漏掉本轮最重要的消费者。

**阶段二：默认不构造诊断**

开关机制：新增 cargo feature `debug-info`。默认关闭，`debug` 字段存在但
不装配（保持 `LayoutDebugInfo::default()`），阶段一新增的运行时字段照常构造。

验收：

1. **默认构建**：`debug` 为空，但全部功能查询、绘制与溢出输入与开关开启时相同。
   这正好复用了阶段一第 2 项的清空 debug 独立性测试。
2. **诊断构建**（`--features debug-info`）：golden 逐字节一致，
   dump 与现有诊断能力不变。
3. **两个构建之间的差分**：布局、glyph、几何、replay、选区与导出输出逐值相同，
   确认关闭诊断只减少诊断信息。
4. **四个基准指标**：默认构建与开启构建各测一次，差额即为本迭代的收益。

两个构建都要在验证计划里跑，避免只测开启版本。

## 回滚

两个阶段各自独立提交。

- 阶段一若产生输出差异，回滚该提交，`debug` 未被改动，运行时路径回到原状态；
- 阶段二若产生差异，回滚该提交并默认开启 feature，回到阶段一的状态。

## 附：metric reason 单项测量

背景：本轮之前曾尝试「让诊断文本在默认构建下不构造」。该尝试以
`MetricDecisionInfo.reason` 为对象，同时截断三个环节：`font_metrics.rs` 的产生点、
`layout_debug_assembly.rs` 的 clone、结构体字段本身。该实验已还原。

| 指标（宽 672） | 保留 | 截断 | 变化 |
| --- | ---: | ---: | ---: |
| `layout` median | 2.459 ms | 2.396 ms | -2.6% |
| `alloc_calls` | 55,205 | 50,997 | -4,208（-7.6%） |
| `allocated_bytes` | 10,613,767 | 10,240,093 | -3.5% |
| `window_live_bytes` | 2,378,110 | 2,214,042 | -6.9% |
| `realloc_calls` | 2,065 | 2,065 | 0 |
| debug 记录数 | 7,395 | 7,395 | 0 |

计时为单次反序 A/B（无 2.403 → 有 2.459 → 无 2.396），-2.6% 位于 ±2% 分辨率边界，
未达可确认程度。分配次数与字节是确定性指标。

这条数据说明一件事：单个诊断文本字段的成本约占 `alloc_calls` 的 7.6%，
跳过最终 debug 装配的探针是 11.9%。两者量级接近。差别在于投入方式：
单字段裁剪要逐个改产生点、clone 与字段，后者一次覆盖全部集合。

该字段当前零读取者：fixture dump 不输出 metric 行，golden 不含它。
它编码的信息分散在上游：`LayoutFontMetrics.policy`（`IdeographicBox` 或 `Raw`）
与 `MetricDecisionInfo.role` 的组合可以还原分支，但 `MetricDecisionInfo`
本身没有 `policy` 字段，还原需要回溯上游记录。

## 与上一轮迭代的关系

`docs/iteration/2026-10-24-perf-string-state-and-debug-allocations.md`
用字段类型化降低单条记录成本，本轮改变记录的存在位置。

上一轮有一项结论需要在本轮之后重审：`debug` 集合中仍有若干 `String` 字段
来自可替换的 backend、resolver、analyzer trait（上一轮第 5–9 项）。

「默认不装配 debug 后这些字符串的成本只在诊断构建中发生」这个推论不成立：
它们可能在 backend、normalizer 或 analyzer 内部已经生成，甚至参与上游计算。
要判断这些字段是否还值得类型化，需要按字符串的实际产生点与生命周期重新测量，
不能只看最终是否被保存。
