# 富文本链接与行内对象标识

> 状态：已完成
>
> 日期：2026-09-22
>
> 分类：feat

## 目标

让 Tiqian 的既有链接语义和行内对象保存调用方提供的可选 `id`，并随 `LayoutInput` 保留到 `LayoutResult`。

## 范围

- `RichTextSemantic::Link` 增加 `id: Option<String>`；
- `InlineObjectSpan` 增加 `id: Option<String>`；
- `ParagraphBuilder` 的链接和行内对象入口接收可选 `id`；
- 保持 `id` 不参与 shaping、断行、行调整、几何计算和布局缓存键；
- 为 builder 和最终布局结果补充回归测试。

## 设计

链接已经使用 `RichTextSemantic::Link` 保存其 `target` 和 source range；`id` 是同一链接的调用方标识，直接保存为该变体字段。行内对象已经使用 `InlineObjectSpan` 保存 range 与度量；`id` 直接保存为该对象字段。

`LayoutResult` 原样持有 `LayoutInput`，因此调用方可以从最终结果的既有链接语义和对象中读取 `id` 与 range，再结合 `positioned_clusters` 消费最终几何。Tiqian 不增加新的交互语义、命中 API 或布局结果类型。

缺少或为空的 `id` 由上层调用方表示为 `None`。Tiqian 不校验、去重、合并或记录 `id`。

## 验证

- builder 对链接和对象保留 `id`；
- `LayoutResult.input` 保留链接和对象的 `id`；
- `cargo test --test paragraph_builder`；
- `cargo test`；
- `git diff --check`。

## 实施结果

- `RichTextSemantic::Link` 与 `InlineObjectSpan` 已保存可选 `id`；
- `ParagraphBuilder` 的链接与行内对象入口已接收并保留该字段；
- 默认对象构造器继续产生 `id: None`，不改变已有布局输入；
- 已运行 `cargo test --test tiqian` 与 `cargo test`，1,343 项测试全部通过；
- 当前文档的风格检查与 `git diff --check` 均通过。
