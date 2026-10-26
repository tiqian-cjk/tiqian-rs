# 开发与调试指南

本文面向 tiqian-rs 的开发者和维护者。

## 环境

- 任意支持 Rust 的操作系统，Windows 下需要 Bash 支持（WSL/MSYS2皆可）。
- Rust toolchain，建议使用最新版。

## 日常检查

在仓库根目录运行：

```shell
cargo check --all-targets
cargo test --all-targets
```

仓库是 Cargo workspace，核心 crate 位于仓库根，其余在 `crates/` 下：

| crate | 路径 | 用途 |
| --- | --- | --- |
| `tiqian` | 仓库根 | 排版核心库，唯一发布的 crate |
| `tiqian-tests` | `crates/tiqian-tests` | 集成测试与 fixture golden |
| `tiqian-demo` | `crates/tiqian-demo` | 桌面演示程序 |
| `tiqian-bench` | `crates/tiqian-bench` | 基准与探针程序 |

`debug-info` feature 只在 `tiqian-tests` 默认开启；`tiqian` 自身的构建保持关闭，
因此不再需要注释依赖行来测量生产路径：

```shell
cargo build -p tiqian --release
```

开启诊断记录时：`cargo test -p tiqian-tests`（默认已开启）或
`cargo run -p tiqian-bench --features debug-info --bin paragraph-layout-bench`。

运行桌面段落示例：

```shell
cargo run -p tiqian-demo --release
```

示例使用 HarfRust、SkRifa 和 Vello 实现 `FontBackend` 并完成字体接入、字形 shaping 与绘制；生产引擎通过 `ParagraphLayoutEngineBuilder` 显式接收该 backend。

## no-std

主库默认启用 `std` 预设。依赖方使用 no-std 时，设置 `default-features = false`，并选择
`features = ["no-std"]`。该配置仍需要 `alloc`，分配器及最终程序的 panic 处理由宿主提供。

| feature | 作用 |
| --- | --- |
| `runtime-std` | 允许主库使用标准库，不选择组件实现 |
| `collections-hashbrown` / `collections-std` | 选择集合；后者启用 `runtime-std` |
| `math-std` / `math-libm` | 选择浮点数学实现；前者启用 `runtime-std` |
| `shared-auto` / `shared-portable` | 按目标原子能力选择 Arc / 强制使用 portable Arc |
| `debug-info` | 结构化诊断，独立于运行环境 |

`std` 预设选择 `runtime-std,collections-hashbrown,math-std,shared-auto`；`no-std` 预设选择
`collections-hashbrown,math-libm,shared-auto`。Cargo feature 只能叠加：每类组件必须且只能选一个，
`no-std` 与 `runtime-std`（含 `std`、`collections-std`、`math-std`）互斥。
自定义组合须关闭默认 feature、不选预设，再直接选择各组件，不能用另一组件覆盖预设。

在仓库根目录验证：

```shell
cargo test -p tiqian --lib --no-default-features --features no-std
cargo test -p tiqian-tests --no-default-features --features no-std
```

no-std CI 仅选择主库和 tests，不引入 demo 或 bench；测试 harness 可使用 std，主库不启用
`runtime-std`。`tiqian-tests` 默认启用的 `demo-tests` 仅用于 std 字体集成测试，并自动启用
`std`；上述命令关闭默认 feature，排除该路径。

公开类型统一使用 `tiqian::common::Arc`：`shared-auto` 在有指针宽度原子的目标使用
`alloc::sync::Arc`，否则使用 `portable_atomic_util::Arc`；`shared-portable` 始终使用后者。
portable Arc 不支持标准 Arc 的自动 unsizing；构造 trait object 时先得到 `Box<dyn Trait>`，
再调用 `Arc::from`。当前 portable 路径每次 annotation 构建因此增加 3 次临时 Box 分配。

无原子目标需要用户配置 portable-atomic 的宿主回退机制；库不默认假设单核，也不提供临界区实现。
用户暂不添加无原子验证目标，该路径尚未验证。文本索引和英文断词表统一使用
`once_cell::race::OnceBox`：每个成功初始化的 cell 额外保留一次 Box 分配，竞争时可能重复
初始化并释放未发布候选，初始化函数须允许重复执行。

## Fixture 与 golden

`crates/tiqian-tests/tests/fixture_layout/` 保存 deterministic stub fixture 与完整 layout decision golden。日常回归测试使用 Rust 仓库内的 fixture，不依赖 Kotlin 仓库。

运行 fixture golden 测试：

```shell
cargo test -p tiqian-tests --test tiqian layout_fixture_golden_test
```

需要确认布局预期发生变化时，显式更新 golden：

```shell
TIQIAN_UPDATE_LAYOUT_GOLDENS=1 cargo test -p tiqian-tests --test tiqian layout_fixture_golden_test
```

更新后逐项检查文本 diff。更新模式会重新生成现有 fixture 的 golden，不会替代缺失的 fixture 或删除未知文件。

## 性能测量

`paragraph-layout-bench` 使用桌面示例的段落和平台字体后端，在不打开窗口的情况下测量布局：

```shell
cargo run -p tiqian-bench --release --bin paragraph-layout-bench
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --iterations 1000 --warmup 50 --widths 672,360,960 --scale 1
```

比较实现时保持构建配置和参数一致，主要观察各宽度的 `layout` 指标。

bench 默认选择 std 主库；测量 no-std 主库时，关闭默认 feature 并选择 `no-std`：

```shell
cargo run --release -p tiqian-bench --no-default-features --features no-std --bin paragraph-layout-bench -- --count-allocations --widths 672 --warmup 10 --iterations 30
```

该 feature 同时转发给 demo 复用依赖，确保主库配置一致。bench、demo 字体后端本身仍使用 std；
不需要修改根 manifest。计时测量去掉 `--count-allocations`，分配计数运行的时间不用于比较。

`--replay` 可以额外测量整页重放索引；`--cluster-queries` 会在每页布局后调用两个逐 cluster 查询并单独计时：

```shell
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --replay
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --cluster-queries
```

分配统计需要单独运行，`--count-allocations` 开启后每次分配都有原子开销，该运行的时间不可用于比较：

```shell
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --count-allocations
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --annotation-cold --widths 672
cargo run -p tiqian-bench --release --bin paragraph-layout-bench -- --fresh-engine --widths 672
```

计数覆盖 `engine.layout()` 调用，转发自 mimalloc 的全局分配器计数；
不包含字体库等原生代码的直接分配，`process_live_bytes` 也不能当作进程总内存。
`--annotation-cold` 只清空 annotation cache，`--fresh-engine` 每样本换新的 cache 状态并重建引擎，
两者互斥；它们都不同于动态 shaping，后者由窄宽度和 technical span 触发。

该机器上同一二进制的 `layout` median 在不同时段可相差 10% 以上（实测 2.46 ms 到 2.75 ms），因此：

- 跨运行比较以 `min` 为主，`min` 在同一环境内稳定在约 2% 以内；
- 对比两个实现时，把两份构建产物放在同一时间窗口交替运行（A/B/A/B），不要用两次单独运行的结果比较；
- 不要跨机器或跨时段比较绝对值，只比较同一轮交替运行中的相对差异。

需要采样时使用 `cargo flamegraph -p tiqian-bench --bin paragraph-layout-bench -- --iterations 1000`。火焰图包含进程启动、预热、输入准备和析构；分析核心时筛选 `engine.layout` 调用树，不要把采样运行时间直接与普通 release 运行比较。

专项工具：`crates/tiqian-bench/src/white-space-bench.rs` 测量空白策略的耗时与累计分配；
`tools/text-autospace-probe.html` 用于浏览器空白与自动间距对照。具体测量条件和结果保存在对应迭代记录中。

## 覆盖率

在提供 Bash 的环境中运行：

```shell
bash tools/coverage.sh
```

HTML 报告位于 `target/llvm-cov/html/index.html`。

## API 接入注意事项

- 优先使用 `api::ParagraphBuilder` 按内容顺序构造段落；`build()` 返回 `LayoutInput`。
- `TextRange` 和布局查询使用 Unicode scalar offset；UTF-8 byte offset 仅用于 Rust 字符串的底层索引。
- `ParagraphLayoutEngineBuilder::new(Box<dyn FontBackend>)` 是生产引擎的唯一构造入口；`FontBackend` 及 builder 可替换的 profile、断行器、hyphenator 和 annotation cache 实现必须为 `Send + Sync`，以使引擎可安全跨线程转移或置于外部同步容器。确定性 backend 仅位于 `tests/support/`，供 fixture、golden 和 integration test 使用。
- 接入平台字体时，实现一个 `FontBackend`，由它统一提供候选选择、完整 shaping、metrics 和可重放的 face identity。
	段落 shaping 的每个 backend request 都是独立的字体选择原子范围；同一个旧 range 被内部切分后，不应在调用方
	重新合并 `FontResolution`。桌面示例中的字体 backend 提供了参考实现。
- `LayoutResult` 同时包含行、cluster、glyph replay 数据和结构化 debug 信息。renderer 应重放布局结果，不应自行重新计算断行或标点几何。
- 获取实际占位范围时使用 `core::layout_queries` 中的 `line_occupied_bounds` 或 `occupied_bounds`；`LayoutResult.size` 的宽度可能受 `max-width` 限制，小于实际占位宽度。
- 自动间距和空白规则见 [R0007](adr/R0007-css-autospace-and-explicit-spaces.md) 与 [R0008](adr/R0008-white-space-policy-and-wrapping.md)。

## 文档与同步

涉及上游同步或架构取舍时，按以下顺序查阅并更新对应文档：

- `docs/tracking.md`：Kotlin 上游跟进状态与同步记录。
- `docs/key-differences.md`：Rust 与 Kotlin 的有意差异。
- `docs/adr/`：已经确定的架构决策。
- `docs/iteration/`：当前迭代的目标、设计和验证记录。

提交前至少运行 `git diff --check`，并确认只包含当前任务相关的改动。行为改变时，补充或更新相关测试；涉及布局行为时，同时检查 fixture golden。
