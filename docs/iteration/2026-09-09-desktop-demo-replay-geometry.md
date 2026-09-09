# 2026-09-09 Desktop Demo 重放几何复用

> 状态：已完成
>
> 上游依据：Kotlin `f4aecd42` 将 replay index、line pattern geometry 与 paint bounds 移入
> `engine`，并由 Compose 与 Android renderer 消费。

## 目标

让 desktop Vello demo 成为 Rust `LayoutResultReplayIndex` 与 fitted dashed / dotted line geometry 的
实际消费者。每个已布局的 demo 文本块保留一个 replay index，renderer 使用其中的 positioned cluster 和
rich-text segment，避免在绘制每个 block 时重复派生相同数据。

同时删除 `examples/paragraph_demo/renderer.rs` 中与 core 重复的虚线、点线坐标计算，统一使用
`core/fitted_line_pattern_geometry.rs`。

## 范围

- 为 demo page 内每个 `LayoutResult` 保存同步构造的 replay index；
- body glyph、rich-text background、rich-text line 和需要 skip-ink 的 decoration 从该 index 读取
  positioned cluster 或 rich-text segment；
- rich-text dotted line 将每个保留区间传入 core 函数，确保 skip-ink 不绘制被截断的圆点；
- 通过 renderer 单元测试和 demo target 验证，并补充 Kotlin `platforms/` 的审计记录。

## 不包含

- 不改变 layout、shaping、字体选择、line breaking 或任何 layout result；
- 不移植 Android View、Android selection、accessibility、生命周期或 Android native font catalog；
- 不用 `visible_paint_overhang` 替代 `app.rs` 的整页绘制范围计算。

`visible_paint_overhang` 的输入是一次绘制的有限裁切矩形，且只覆盖 core 已定义的 glyph、ruby、emphasis dot。
desktop demo 的整页范围还依赖 bopomofo、annotation glyph、line-end hyphen、decoration segment 和 Vello
笔画端点等实际绘制内容，继续由 demo 计算。

## 验证与回滚

验证新增 demo renderer 测试、现有 core 测试、`cargo test`、fixture golden、`cargo check`、`git diff --check`
与本迭代文档的风格检查。不运行 format 命令。

如 demo 使用 index 后出现缺字、错误位置、富文本分段变化或点线进入 skip-ink 区间，撤回 demo 接入，保留
已有 core helper 与其独立回归。

## 实施结果

- `DemoPageBlock` 为每个正文、列表 marker 与列表 body layout 保存一个 replay index；页面整体范围计算复用
  index 的 positioned cluster，页面范围仍由 Vello demo 负责；
- `DemoRenderer` 使用 index 的 positioned cluster、glyph grouping、background segment 与 decoration segment；
- 删除 demo 内重复的 fitted dash / dot 函数。虚线使用 core flat coordinate pair，点线按每个 kept interval
  调用 core，跳过会与 glyph ink 相交的圆点；
- renderer 的 line-end hyphen 测试在清空 `glyph_runs` 后重建 index，保持测试构造的 result 与 index 一致。

2026-09-09 已通过：

```text
cargo check --example paragraph-demo
cargo test --example paragraph-demo
  20 passed
cargo test
  1349 passed
cargo test --test tiqian layout_fixture_goldens_match
  1 passed
git diff --check
  passed
```