# 拆分为多 crate workspace

日期：2026-10-25
分类：refactor

## 目标

把 `tiqian-rs` 从单 package 仓库改为 Cargo workspace，让排版核心、集成测试、
桌面演示与探针程序各自成为独立 crate。核心目标是消除 `tiqian` 自引用
dev-dependency 这一结构，使 `debug-info` 的开关变成显式的依赖声明。

## 范围

### 迁入

| 原位置 | 新位置 |
| --- | --- |
| `src/` | `crates/tiqian/src/` |
| `resources/hyphenation/` | `crates/tiqian/resources/hyphenation/` |
| `tests/` | `crates/tiqian-tests/tests/` |
| `examples/paragraph-demo.rs` 与 `examples/paragraph_demo/` | `crates/tiqian-demo/src/` |
| `resources/fonts/` | `crates/tiqian-demo/resources/fonts/` |
| `examples/*-bench.rs` | `crates/tiqian-bench/src/` |

根目录 `resources/` 与 `examples/` 删除。

### 不迁入

`docs/`、`tools/`、`Cargo.lock`、`README*`、`LICENSE`、`CHANGELOG.md`、`AGENTS.md`
留在仓库根。`crates/tiqian/` 另放一份 `README.md` 与 `LICENSE`，
因为 Cargo 的 `include` 只能引用 crate 内文件，发布包需要自带这两项。

## 设计

### crate 划分

| crate | 职责 | 依赖 |
| --- | --- | --- |
| `tiqian` | 排版核心库，唯一发布的 crate | `icu_properties`、`log`、`hashbrown` |
| `tiqian-tests` | 集成测试与 fixture golden | `tiqian`（含 `debug-info`）、`tiqian-demo` |
| `tiqian-demo` | 桌面演示程序，同时提供 lib | `tiqian`、`harfrust`、`pollster`、`skrifa`、`vello`、`winit`、`mimalloc` |
| `tiqian-bench` | 基准与探针程序 | `tiqian`、`tiqian-demo`、`mimalloc` |

`tiqian` 不再有任何 `[dev-dependencies]`，自引用行随之消失。

### `debug-info` 的归属

`debug-info` 只在 `tiqian-tests` 的依赖声明里开启：

```toml
tiqian = { path = "../tiqian", features = ["debug-info"] }
```

由此：

- `cargo build -p tiqian --release` 直接就是生产构建，不需要注释任何行；
- `tiqian-tests` 里 453 处 `.debug` 读取无需改动；
- `tiqian-demo` 不开该 feature，渲染只消费 `LayoutResult.layout`。

`tiqian-bench` 提供同名透传 feature，需要统计各 decision 集合规模时显式开启：

```toml
[features]
debug-info = ["tiqian/debug-info"]
```

默认关闭，因此探针默认测量生产路径。

### 共享代码

迁移中发现两处跨 crate 的复用，此前靠 `#[path]` 直接引用源文件：

1. `tiqian-bench` 的两个探针使用 demo 的 `DemoFontCatalog` 与 `sample::build_document_demo`；
2. `tiqian-tests` 的 `phase_five_white_space` 使用 demo 的 `DemoFontCatalog`。

原实现是 `#[path = "paragraph_demo/font_backend.rs"] mod font_backend;`。
迁移后新增 `crates/tiqian-demo/src/lib.rs`，Cargo 自动把它识别为 lib 目标，
名称为 `tiqian_demo`，无需在 `Cargo.toml` 里声明 `[lib]`。

`lib.rs` 只导出 `font_backend` 与 `sample` 两个模块。`renderer` 与 `app`
仍只属于二进制目标，因为它们依赖 `vello` / `winit` 的绘制与窗口上下文。
构建方改为 `use tiqian_demo::font_backend;`。

### demo 的 `debug` 断言迁移

demo 不开 `debug-info` 后，三处读取 `debug` 的测试断言按等价运行时数据改写：

| 位置 | 原断言 | 处理 |
| --- | --- | --- |
| `font_backend.rs` | `debug.metric_decisions[..].raw_source == RawTables` | 改为 `layout.font_metrics[..].source`；两者同源于 `raw_metrics.source` |
| `font_backend.rs` | `debug.shaping_decisions` 中 emoji range 的 `font_face` | 删除：紧随其后的 `glyph_runs` 断言已用 `render_font_face` 覆盖同一事实 |
| `app.rs` | `debug.shaping_decisions` 中 span 范围的 `font_face` | 改为 `clusters[..].font_face`，cluster 的面就是绘制实际使用的面 |

`renderer.rs` 的渲染主路径与四处渲染断言在上一轮已迁移到 `layout`，
本轮无需再改。

### 路径修正

`include_str!` / `include_bytes!` 的相对路径以源文件所在位置为基准：

| 资源 | 改动 |
| --- | --- |
| `crates/tiqian/src/linebreak/english_hyphenation.rs` | 无需改动，`../../resources/` 层级不变 |
| `crates/tiqian-demo/src/font_backend.rs` | `../../resources/fonts/` → `../resources/fonts/` |

### 二进制目标

目标名与包名的关系由 Cargo 的默认规则决定，`crates/tiqian-demo` 不需要任何声明：

- lib 名默认为包名把连字符换成下划线，即 `tiqian_demo`；
- bin 取 `src/main.rs`，名默认为包名，即 `tiqian-demo`。

`crates/tiqian-bench` 需要用显式 `[[bin]]` 指定路径，因为七个文件名都带连字符、
也不在 `src/bin/` 下，Cargo 的自动发现找不到：

```toml
[[bin]]
name = "paragraph-layout-bench"
path = "src/paragraph-layout-bench.rs"
```

`autobins = false` 在两个 crate 里都没有实际作用：显式 `[[bin]]` 与自动发现不会
产出重复目标。实测删除后目标数与名称均不变，因此配置中不保留这一项。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo check --all-targets` | 通过（仅剩既有的 `right_overhang` 未读警告） |
| `cargo test --all-targets` | 全部通过，含 1364 项引擎测试与 golden 零 diff |
| `cargo check -p tiqian` | 零错误零警告 |
| `cargo check -p tiqian --features debug-info` | 零错误零警告 |
| `cargo build -p tiqian --release` | 零警告 |
| `cargo check -p tiqian-bench`（默认关） | 零警告 |
| `cargo check -p tiqian-bench --features debug-info` | 零警告 |
| `cargo build -p tiqian-demo --release` | 成功（仅既有警告） |
| `cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --widths 672 --warmup 2 --iterations 5` | 2.185 ms，与迁移前 2.184 ms 持平 |

## 命令约定

仓库根是虚拟 manifest（只有 `[workspace]`，没有 `[package]`），
`workspace_default_members` 就是全部四个成员。因此：

- `--workspace` 在根目录不改变包的选择，日常验证不需要它；
- `--all-targets` 决定每个包构建哪些目标，只有它会构建 bin 与 bench 目标的测试。

日常检查统一写 `--all-targets`。`cargo test --workspace` 不含 `--all-targets` 的语义，
只跑 lib 单元测试与集成测试，会漏掉 bin 与 bench 目标里的测试，
两者混用会让本地验证与 CI 覆盖范围不一致。

## 回滚

本次是纯目录与配置迁移，源码逻辑只动了三处测试断言与两处模块引用。
回滚方式为还原 `Cargo.toml`、把四个 `crates/*` 目录移回原位。
字体资源是 Git LFS 文件，迁移时需确认 LFS 指针仍完好。

## 遗留

- `examples/` 目录已不存在，命中此前命令行习惯的文档需按新命令书写。
- 根目录 `Cargo.lock` 保留并随 workspace 更新。
