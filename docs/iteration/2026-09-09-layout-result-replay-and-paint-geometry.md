# 2026-09-09 LayoutResult 重放索引与绘制几何

> 状态：已完成
>
> 上游依据：Kotlin `f4aecd42`（`LayoutResultReplayIndex`、`LayoutPaintBounds` 与
> `FittedLinePatternGeometry`）。

## 目标

将 Kotlin 上游的三项跨平台派生几何能力移植到 Rust：

- 从一个最终 `LayoutResult` 构造不可变 replay index，避免 renderer 与交互层在同一结果上反复派生 cluster、
  rich-text、glyph 与 decision 查询；
- 计算位于 viewport 内可见内容的绘制 overhang，供 renderer 保留已由 engine 输出的 glyph、ruby 与
  emphasis dot 裁切；
- 为 rich-text dashed / dotted line 提供端点对齐的图样几何。

这些能力只读取最终 `LayoutResult`，不产生 layout decision，也不改变 cluster、glyph、line、size、debug dump
或 fixture golden。

## 范围与边界

### 包含

- `LayoutResultReplayIndex`：positioned cluster、按行 cluster、rich-text segment、background / decoration segment、
  cluster range 对应 glyph、OpenType feature 与 font role 的只读索引；
- replay index 上的 selection offset、selection word range、cursor rect 与 selection box 查询；
- `LayoutPaintOverhang`、`visible_paint_overhang` 与 `legal_hanging_punctuation_clip_edge`；
- `fitted_dotted_line_centers` 与 `fitted_dashed_line_segments`；
- Kotlin 定向回归和 Rust scalar source coordinate 的测试。

### 不包含

- 修改 `LayoutInput`、ParagraphBuilder、R0002 rich-text lowering、shaping、font fallback、metrics、line breaking、
  justification、annotation geometry 或 debug decision；
- 将 replay index 缓存到 `LayoutResult`，或定义跨 result 的 cache / invalidation / concurrency 语义；
- Android View、Compose renderer、logical document selection、Android font catalog、host glyph replay 或 baseline profile；
- 改变 renderer 的 clip policy。core 只给出 visible paint overhang 与 engine 选定的 hanging clip edge，宿主决定何时使用。

## API 设计

### Replay index

在 `src/core/layout_result_replay_index.rs` 定义：

```text
LayoutResultReplayIndex
├─ positioned_clusters
├─ positioned_clusters_by_line
├─ rich_text_segments
├─ rich_text_background_segments
├─ rich_text_decoration_segments
├─ glyphs_by_cluster_range
├─ open_type_features_by_cluster_range
└─ font_role_by_cluster_range
```

`to_replay_index(&LayoutResult)` 每次从该 result 派生一个独立 index。它只持有 clone 的输出数据，不借用
`LayoutResult`，因此调用方必须在 result 或其 rich-text 声明改变后自行重建 index。Rust 的 result 输入已经携带
rich-text，故函数不接收 Kotlin 旧模型的外部 span 列表。

为避免第二份交互算法，index 查询复用 `layout_queries.rs` 的 interaction boundary 规则和 `PositionedCluster`
的 source-stop 规则。公开函数为：

```text
selection_offset_for_position(index, result, x, y)
selection_word_range_for_position(index, result, x, y)
cursor_rect(index, result, offset)
selection_boxes(index, result, range)
```

所有 offset 与 range 保持 R0001 的 Unicode scalar coordinate。index 只优化 repeated replay query；普通
`layout_queries` API 保持现有签名与行为。

### Paint geometry

在 `src/core/layout_paint_bounds.rs` 定义：

```text
LayoutPaintOverhang { left, top, right, bottom }
legal_hanging_punctuation_clip_edge(&LineBox, viewport_width)
visible_paint_overhang(&LayoutResult, viewport_width, viewport_height, positioned_clusters)
```

`visible_paint_overhang` 只计算 viewport 内 occupied cluster 对应的 glyph ink、已应用 emphasis dot 和 ruby。
返回值从 occupied box 到 paint extent 的四侧最大距离；没有可见 overhang 时全部为零。输入的 positioned cluster
由调用方传入，以便 renderer 复用 replay index；普通调用使用 `positioned_clusters(result)`。

### Fitted line pattern

在 `src/core/fitted_line_pattern_geometry.rs` 定义：

```text
fitted_dotted_line_centers(span_left, span_right, kept_left, kept_right, dot_diameter, gap_length)
fitted_dashed_line_segments(span_left, span_right, dash_length, gap_length)
```

两个函数与 Kotlin 同型：点线以完整圆点过滤 skip-ink 区间，但图样始终按原 span 拟合；虚线在至少两条 dash 可容纳时
固定 dash length 并平均 gap，短 span 退为一条覆盖全 span 的 dash。非法非有限或非正参数沿用 Kotlin `require`
语义，在 Rust 中 panic。

## 验证与回滚

新增的 Rust 测试至少固定：

- replay index 的 positioned cluster、rich-text segment、selection offset 和 line-local selection box 与现有 query
  同源；
- glyph、emphasis dot 与 ruby 的可见 overhang，及不可见 occupied cluster 不计入；
- hanging punctuation clip edge 仅在 engine 已选择 hanging punctuation 时扩展；
- Kotlin 五项 dotted / dashed pattern 向量；
- scalar offset 的 selection query 与普通 `layout_queries` 一致。

完成后运行相关 core 测试、完整 `cargo test`、fixture golden、`git diff --check` 和两份改动文档的风格检查。

若 replay index 与普通 query 输出不一致，先修正共享算法或 index 构造，不能通过 renderer 偏移补偿。若 layout dump、
cluster、glyph、line、size 或 debug decision 改变，撤回本迭代，因为它们不属于派生几何的允许影响范围。

## 实施结果

- 新增 `src/core/layout_result_replay_index.rs`。index 保存 positioned cluster、按行 cluster、rich-text
  segment、cluster range 对应的 glyph、OpenType feature 与 font role；selection offset、word range、cursor
  rect 与 selection box 复用 `layout_queries.rs` 的既有规则；
- 新增 `src/core/layout_paint_bounds.rs`。实现 glyph、emphasis dot 与 ruby 的可见绘制超出量，以及由 engine
  hanging decision 授权的裁切右边界；
- 新增 `src/core/fitted_line_pattern_geometry.rs`。实现 Kotlin 同型的点线圆心和虚线线段计算；
- `layout_queries.rs` 仅将 `nearest_line_for_position`、`x_for_offset`、`offset_for_x` 提升为
  `pub(crate)`，供 replay index 与原有查询共享；
- 新增 replay index、paint bounds 与 line pattern 的 core 回归测试。所有 source offset 与 range 保持
  R0001 的 Unicode scalar coordinate；
- `docs/iteration/2026-09-09-desktop-demo-replay-geometry.md` 后续将 replay index 与 line pattern
  geometry 接入 Vello demo；该 demo 迭代不使用有限裁切矩形的 `visible_paint_overhang` 代替整页绘制范围计算。

2026-09-09 已通过：

```text
cargo test --test tiqian fitted_line_pattern_geometry
  5 passed
cargo test --test tiqian layout_result_replay_index
  1 passed
cargo test --test tiqian layout_paint_bounds
  3 passed
cargo test
  1349 passed
cargo test --test tiqian layout_fixture_goldens_match
  1 passed
cargo check --example paragraph-demo
  passed
cargo test --example paragraph-demo
  20 passed
git diff --check
  passed
```

fixture golden 无变化。检查期间只有既有 `DpContext.gap_boundaries` 未读取 warning；本迭代未修改该字段。
