# 2026-09-19 删除未使用的 prepared paragraph JSON

> 状态：已完成
>
> 分类：feat

## 目标

移除 tiqian-rs 中没有外部调用者的 prepared paragraph JSON 公开 API，收敛 Rust crate 的公开输出边界。

## 范围

- 删除 `layout::prepared_paragraph` 模块及其 JSON 序列化、数值格式化和诊断 envelope；
- 删除仅验证该 API 的五个测试模块；
- 从 crate 根模块移除导出；
- 在关键差异文档记录 Rust 不提供 Kotlin Web prepared-plan wire。

不改变 `ParagraphLayoutEngine`、`LayoutResult`、布局决策、glyph replay 或布局查询行为。

## 设计

Kotlin 的 prepared paragraph JSON 为 Web snapshot、prepared-DOM 和浏览器字体回退服务。tiqian-rs 没有
对应的 Web 或 FFI 消费者，且没有外部调用者。保留该 API 会维护一份无消费者的手工 JSON wire 及其专属
回归测试。

Rust 调用方直接消费 `LayoutResult`。未来出现实际消费者时，根据其输入输出定义设计单一接口，避免提前
维护跨平台计划格式。

## 验证与回滚

- 检索 `prepared_paragraph`、`to_prepared_paragraph_json`、`to_plan_with_diagnostics_json` 与
  `ecma_json_number`，确认生产源码、测试索引和公开 API 均无残留引用；
- 运行 `cargo test --test tiqian` 与 `cargo test --all-targets`；
- 运行 `git diff --check` 和本文件、`docs/key-differences.md` 的文档风格检查；
- 如后续出现实际 JSON plan 消费者，恢复该能力时应先定义输入输出和兼容要求，再实现新的单一接口。

## 完成记录

- 删除 `src/layout/prepared_paragraph.rs`；
- 删除五个 prepared paragraph 专属测试模块并移除测试索引；
- `LayoutResult` 保持为 Rust 渲染与调试结果的唯一公开数据源；
- `cargo test --test tiqian` 与 `cargo test --all-targets` 均通过；后者为 1341 个测试通过、0 个失败，
  paragraph demo 的 20 个测试与 paragraph layout benchmark 的 9 个测试均通过；
- `git diff --check` 与本次中文文档风格检查通过。
