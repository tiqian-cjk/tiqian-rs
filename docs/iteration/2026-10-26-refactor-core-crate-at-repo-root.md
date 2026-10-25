# 核心 crate 回迁至仓库根

日期：2026-10-26
分类：refactor

## 目标

`cargo smart-release` 以 manifest 所在目录为准处理版本号、`CHANGELOG.md` 与 tag。
上一轮拆分把 `tiqian` 放进 `crates/tiqian`，导致：

- `version.workspace = true` 被改写成字面值，与 `[workspace.package] version` 分叉；
- changelog 写到 `crates/tiqian/CHANGELOG.md`，根文件历史断链；
- tag 从 `v0.x.y` 变成 `tiqian-v0.x.y`，历史无法按 tag 切分。

本次把核心 crate 移回仓库根，三个不发布的 crate 继续留在 `crates/`。

## 改动

| 原位置 | 新位置 |
| --- | --- |
| `crates/tiqian/src/` | `src/` |
| `crates/tiqian/resources/hyphenation/` | `resources/hyphenation/` |

删除 `crates/tiqian/Cargo.toml`（并入根 manifest）、`README.md` 与 `LICENSE`（与根文件相同）、
`CHANGELOG.md`（误生成）。

根 `Cargo.toml` 同时是 package 与 workspace manifest。因为根 manifest 带 `[package]` 后
Cargo 默认只构建根包，需用 `workspace.default-members` 列出全部四个成员。

`[workspace.package]` 移除 `version`；三个内部 crate 各自写 `version = "0.0.0"`，
`tiqian` 依赖路径改为 `path = "../.."`。

`include` 改为 `/src/**/*` 等锚定写法：不带 `/` 的模式在 gitignore 语义下递归匹配，
会把 `docs/adr/README.md` 带进发布包；同时补上 `/CHANGELOG.md`。

`src/linebreak/english_hyphenation.rs` 的 `include_str!` 相对层级不变，无需改动。

文档同步：`AGENTS.md`、`docs/dev-guide.md` 的 crate 表增加路径列。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo check --all-targets` | 通过 |
| `cargo test --all-targets` | 全部通过，含 1364 项引擎测试与 golden 零 diff |
| `cargo package -p tiqian --list` | 只含 `src/`、`resources/hyphenation/`、`Cargo.toml`、`README.md`、`LICENSE`、`CHANGELOG.md` |

## 回滚

把 `src/`、`resources/` 移回 `crates/tiqian/`，恢复该目录的 manifest 与 `README.md`、`LICENSE`，
根 `Cargo.toml` 还原为虚拟 manifest 并恢复 `[workspace.package] version`，
三个内部 crate 的 `path` 改回 `../tiqian`。

## 遗留

`cargo smart-release` 在提交前会 panic：HEAD 的树里没有 `src/`，工具读已发布 tag 的树时
找不到该路径。提交后消失。
