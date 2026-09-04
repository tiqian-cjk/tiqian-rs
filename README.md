# 提椠 Tíqiàn (Rust Port)

`tiqian-rs` 是 [提椠 Tíqiàn](https://github.com/tiqian-cjk/tiqian) 中文横排排版核心的 Rust 移植。当前使用确定性 stub shaping、stub font metrics 和 Rust 本地 layout fixture golden 验证移植行为。

## 开发环境

- Rust toolchain（Cargo，edition 2024）

## Fixture 验证

本仓库在 `tests/fixture_layout/` 保存全部 52 项 deterministic stub fixture 与对应 golden。每项 fixture 都使用 greedy、lookahead、paragraph-DP 三种 breaker，并比较完整 layout decision dump。

运行本地 fixture golden 测试：

```shell
cargo test --test tiqian layout_fixture_golden_test
```

默认模式只读取 golden，缺少、额外或不匹配的 golden 都会失败。更新预期输出时显式设置环境变量：

```shell
TIQIAN_UPDATE_LAYOUT_GOLDENS=1 cargo test --test tiqian layout_fixture_golden_test
```

更新会重新生成全部 52 个 Rust 本地 golden，不会删除未知文件；结束时仍检查 fixture ID 与 golden 文件名集合相等。更新后逐项审阅文本 diff。Kotlin 的 fixture 定义和普通 golden 仅在上游同步时作为对照来源，不参与日常 Rust 测试。

## 本地检查

Rust 侧常规编译与测试：

```shell
cargo check
cargo test
```

运行全部 Rust target 并生成 HTML 覆盖率报告：

```shell
bash tools/coverage.sh
```

报告入口位于 `target/llvm-cov/html/index.html`。覆盖率路径只依赖 Rust toolchain。
