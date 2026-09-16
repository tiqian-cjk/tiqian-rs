# ADR R0004：显式字体后端的段落引擎构造

- Status: Accepted
- Date: 2026-09-16
- Implementation: Complete
- Relates: [2026-09-16 显式字体后端引擎入口](../iteration/2026-09-16-feat-explicit-font-backend-engine.md)

## Context

R0003 已将字体选择、完整 shaping、metrics 和 glyph replay 收敛到 `FontBackend`。但当前
`ExplainableStubParagraphLayoutEngine::default()` 仍在 `src/` 中直接构造确定性 stub backend。该 backend
用于 fixture 和 golden，不读取平台字体资源，也不执行 OpenType shaping。

这使测试实现成为生产 crate 的默认依赖，并把无参 stub engine 暴露为 README、benchmark 和示例的入口。
将 stub 直接移动到根 `tests/` 不能解决问题：integration test crate 依赖库代码，`src/` 无法反向依赖
`tests/`。

## Decision

生产段落引擎采用显式构造：

- `api::ParagraphLayoutEngineBuilder` 要求调用方提供 `Box<dyn FontBackend>`；
- builder 使用已有的内置 profile、metrics normalizer、贪心断行器、justifier、hyphenator 与缓存组装
  `ParagraphLayoutEngine`；
- `ParagraphLayoutEngine` 是完整生产 pipeline 的唯一公开实现；
- 生产引擎不提供无参 `Default`，也不依赖 deterministic stub；
- deterministic stub backend 位于 `tests/support/`，测试直接通过生产 builder 创建引擎。

桌面 demo 和 benchmark 直接向 builder 传入 `DemoFontCatalog`。fixture、golden 和 integration test 将
`tests/support/` 的 deterministic backend 传入同一 builder。`src/`、README 和生产示例不引用测试 support。

## Consequences

### 正面影响

- 生产调用方必须明确选择能够提供平台字体证据的 `FontBackend`；
- 发布 crate 不包含 fixture backend；
- 测试与生产使用同一 builder 和 pipeline，只有字体 backend 不同；
- desktop demo 不再创建后立即替换 stub backend 的临时 engine。

### 需要接受的变化

- 删除 `ExplainableStubParagraphLayoutEngine::default()` 及其直接修改公有组件字段的接入方式；
- README 的最小示例不能再在没有 `FontBackend` 的情况下执行 layout；
- fixture runner 与 integration test 需要从 `tests/support/` 导入 deterministic backend；
- 本决定与 Kotlin 保留历史 stub engine 名称的做法不同，属于 Rust 的有意 API 边界调整。

## Alternatives considered

- **仅将 `DeterministicStubFontBackend` 移入 `tests/`。** 否决：生产 engine 仍会反向依赖 tests，库无法编译。
- **保留无参 `ExplainableStubParagraphLayoutEngine`，只将它改名为测试类型。** 否决：测试 stub 仍会随
  生产 crate 发布，且普通调用方仍能把它当作默认入口。
- **建立独立的 `tiqian-test-support` crate。** 暂不采用：当前仓库的 integration test 已能承载 support，
  单独 crate 会增加 workspace、发布和版本维护成本。

## Verification

已完成以下验证：

1. `src/` 不再引用 deterministic stub 或测试 support；
2. production builder 只能在明确提供 `FontBackend` 后构造引擎；
3. desktop demo 和 benchmark 使用 `DemoFontCatalog`；
4. fixture 和 golden 通过 `tests/support/` 的 deterministic backend 进入相同 layout pipeline；
5. fixture golden 无差异；
6. `cargo test --all-targets --no-fail-fast --color never` 通过；
7. `cargo check --examples`、`git diff --check` 与文档风格检查通过。
