# 2026-09-19 线程安全的段落引擎依赖

> 状态：已完成
>
> 分类：feat

## 目标

使 `ParagraphLayoutEngine` 可以自动实现 `Send + Sync`，让使用受控字体后端的调用方能够安全地将
引擎跨线程转移或放入外部同步容器。

## 范围

- 将段落引擎持有的可注入依赖限定为 `Send + Sync`；
- 为 `ParagraphLayoutEngine` 添加编译期线程安全回归测试；
- 在开发指南中声明平台接入约束。

不改变布局算法、字体候选选择、shaping、缓存策略或布局结果；不为引擎增加内部锁，也不使用
`unsafe impl`。

## 设计

`ParagraphLayoutEngine` 在运行时保存字体后端、CLREQ profile resolver、metrics normalizer、font role
classifier、line breaker、hyphenator 和 annotation cache。它们都是可替换的长期依赖，因此 trait
定义直接继承 `Send + Sync`。line breaker 内部的 kinsoku rule 同样采用该约束，保证内嵌 trait object
不会破坏引擎的自动推导。

引擎的 `layout` 仍需要 `&mut self`，因为 annotation cache 会更新。`Send + Sync` 不代表可以绕过可变
借用规则并发执行 layout；需要共享同一实例时，调用方使用外部同步原语。

## 验证与回滚

- 运行 `cargo check`；
- 运行 `cargo test paragraph_layout_engine_is_send_and_sync` 和完整测试；
- 运行 `git diff --check`；
- 若某个平台依赖无法满足线程安全约束，回退该接口的新增限制并重新设计其所有权或同步边界，不使用
  不安全标记绕过编译器。

## 完成记录

- `cargo check` 通过；
- `cargo test paragraph_layout_engine_is_send_and_sync` 通过；
- `cargo test` 通过：1366 个测试通过，0 个失败；
- `git diff --check` 与本次中文文档风格检查通过；
- 新增限制只作用于可注入依赖的类型边界，未改变布局结果或引擎的可变借用语义。
