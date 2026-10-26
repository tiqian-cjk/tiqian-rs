# no-std 支持与可组合 feature

日期：2026-10-26
分类：feat
状态：Phase 1 至 Phase 3 已实施，Phase 4 本地测试及使用文档已完成；目标验证、远程 CI 与性能对比未完成。可移植 Arc 已按用户选择采用安全 Box 转换。

## 目标

使核心 `tiqian` crate 支持 `no_std + alloc`，保留完整布局管线、结构化诊断能力和现有
`Send + Sync` 契约。沿用 `collections-*` 的实现选择模式，将运行环境与组件实现分离，
由 `std`、`no-std` 两个预设提供推荐组合，同时允许关闭预设后自由组合兼容的组件。

主机上的 no-std 配置已通过回归测试。完整目标验证完成前，本迭代保持未完成，不将主机
测试结果表述为裸机或无原子目标已验证。

## 范围

### 纳入范围

- 核心 crate 的 `core`、`alloc` 路径与显式导入，包括类型、转换 trait、宏和单元测试作用域。
- 标准库能力开关、细粒度组件 feature、预设、冲突检查和合法组合验证。
- 浮点取整的 no-std 实现，以及基于目标原子能力的共享指针选择。
- 依赖 feature 审计、必要回归测试、线程安全检查、性能验证及独立 no-std CI 任务。
- 完整支持实现后的开发指南与公开类型接入说明。

### 范围边界

- 以 `alloc` 为必需前提，不设计无堆分配的布局 API；分配器与最终程序的 panic 处理由宿主提供。
- demo、bench、测试 runner 继续使用 std；平台字体 backend 的 no-std 可用性由各实现自行保证。
- 不修改布局算法、文本坐标、断行规则或缓存容量，不自动将 `Arc` 降为 `Rc`。
- 保留文本索引和内置英文断词的惰性初始化，不提前计算索引，不为绕过阻碍裁减功能。
- 无原子目标纳入设计；用户暂不添加验证目标，保留未验证状态，宿主回退仍由用户配置。

## 调研依据

调研时根 crate 为 `tiqian 0.7.0`，默认 feature 是 `collections-hashbrown`；实施后默认改为 `std` 预设。
用户已有未提交修改将 5 个源文件中的部分 std 重导出改为 core，本任务保留这些改动。

2026-10-26 对 `src/**/*.rs` 的扫描结果如下。计数用于估计迁移范围，不作为验收常量。

| 项目 | 结果 |
| --- | --- |
| Rust 源文件 | 64 个 |
| 显式 `std::` | 27 处，涉及 18 个文件 |
| 分配类型或宏的候选文件 | 49 个，包含测试代码 |
| `OnceLock` 存储位置 | 2 处 |
| `.floor()` / `.round()` | 8 个调用点 |

没有检出核心代码直接使用文件 IO、时钟、线程创建、`Mutex` 或 `RwLock`。
Rust 1.98.0 的独立 `#![no_std]` 编译探针确认 `f32::floor()`、`f32::round()` 不可用，
因此这两类调用需要替代实现，不能仅迁移路径。

本机锁定的 `icu_properties 2.4.0`、`hashbrown 0.17.1` 和 `log 0.4.34` 均提供 no-std
路径；当前核心 crate 的 feature 树未发现启用目标运行时 std feature。过程宏在宿主端使用
std 与目标端 no-std 不冲突。尚未完成无 std 目标的整体编译验证。

## 设计

### 1. core 与 alloc

`fmt`、`cmp`、`hash`、`str`、`ops`、`borrow::Borrow` 和 `error::Error` 使用 core。
`String`、`Vec`、`Box`、`Cow`、`Rc`、树集合以及相关 trait、宏使用 alloc。
crate 根显式接入 alloc；模块按作用域导入实际使用的类型与宏，不依赖 std prelude。

是否应用 `#![no_std]` 由底层 `runtime-std` feature 决定，而非由是否启用某个预设决定。
未启用 `runtime-std` 时使用 no-std 编译环境；alloc 始终是核心库的必要能力，不另设可关闭开关。

std 重导出类型迁移到 core 或 alloc 不改变其类型身份。项目已有 `crate::core` 模块，标准 core
路径需要避免歧义，必要时使用绝对路径 `::core`。

### 2. 细粒度 feature

以下 feature 已写入 manifest，并接入实现与编译期冲突检查。

| feature | 含义及依赖 |
| --- | --- |
| `runtime-std` | 允许依赖标准库，不自动选择组件实现 |
| `collections-hashbrown` | 选择 hashbrown 集合，启用可选 hashbrown 依赖 |
| `collections-std` | 选择标准库集合，自动启用 `runtime-std` |
| `math-std` | 使用标准库浮点数学方法，自动启用 `runtime-std` |
| `math-libm` | 使用 libm 浮点数学实现，启用可选 libm 依赖 |
| `shared-auto` | 有指针宽度原子操作时使用标准 Arc，否则使用可移植 Arc |
| `shared-portable` | 显式使用可移植 Arc，包括原生支持原子的目标 |
| `debug-info` | 保持现有诊断语义，独立于运行环境和组件实现 |

集合、数学、共享指针三个维度各要求恰选一个实现 feature。
`shared-auto` 与 `shared-portable` 互斥，不能同时启用。
可移植 Arc 已采用 `portable-atomic-util`，关闭默认 feature 并启用 `alloc`。

两处初始化在所有组合中统一使用 `OnceBox`，不新增互斥的初始化实现 feature。
once_cell 依赖关闭默认 feature 并启用 `alloc`；无指针宽度原子操作的目标额外启用其
`portable-atomic` feature，与共享指针路径共用宿主提供的原子回退前提。

### 3. std 与 no-std 预设

| 预设 | 启用的细粒度 feature |
| --- | --- |
| `std` | `runtime-std`、`collections-hashbrown`、`math-std`、`shared-auto` |
| `no-std` | `collections-hashbrown`、`math-libm`、`shared-auto` |

默认 feature 调整为 `std`，继续使用 hashbrown，保留现有默认运行环境与数学行为。
`no-std` 表达无标准库约束，并提供推荐组件组合；无预设时也允许通过细粒度 feature 构建
no-std 配置。

Cargo feature 只能叠加，预设是固定组合，不是可被其他 feature 覆盖的默认值。
例如 `std + math-libm` 会同时选择两个数学实现，必须报告冲突。需要自定义组合时，调用方
关闭默认 feature，不选择预设，直接选择所需的底层能力和组件。

| 自定义组合示例 | 预期 |
| --- | --- |
| `runtime-std + collections-hashbrown + math-libm + shared-auto` | 合法，使用 std 环境和 libm 数学实现 |
| `collections-std + math-libm + shared-auto` | 合法，由集合实现自动启用标准库能力 |
| `collections-hashbrown + math-libm + shared-portable` | 合法的 no-std 设计组合，仍需满足宿主同步前提 |
| `no-std + collections-std` | 冲突：无标准库约束与标准库组件同时启用 |
| `std + no-std` | 冲突：互斥预设同时启用 |

冲突或缺少实现时在编译期报告明确错误；不按启用顺序或静默优先级选实现。
`no-std + runtime-std` 同样报错。合法 feature 组合不等于已经具备全部平台能力；
`OnceBox` 等迁移已实施，主机配置验证结果见文末；目标能力仍需单独验证。

### 4. 原子能力探测与共享指针

使用 `cfg(target_has_atomic = "ptr")` 判断目标是否支持指针宽度原子操作，与
`alloc::sync` 的目标条件一致，不依赖运行时 CPU 探测或宿主机器能力。

| 配置与目标条件 | 共享指针实现 |
| --- | --- |
| `shared-auto`，支持指针宽度原子操作 | `alloc::sync::Arc` |
| `shared-auto`，不支持指针宽度原子操作 | `portable_atomic_util::Arc` |
| `shared-portable` | `portable_atomic_util::Arc` |

manifest 已按目标条件引入替代依赖，标准目标的自动路径不启用可选的 portable Arc 依赖。
库内部和公开签名统一使用从 `common` 导出的 `Arc`，延续现有公共集合
导出的组织方式，避免每个模块重复分流。

保持 `Text`、字体身份、查询结果、共享闭包和可注入依赖的线程安全契约；不使用
`unsafe impl Send/Sync` 绕过类型检查。不同 Arc 实现是不同类型，调用方操作相关公开字段时
应使用库导出的 `common::Arc`。同一构建中的实现必须一致。

portable Arc 不支持从 `Arc<闭包>` 到 `Arc<dyn Fn>` 的自动 unsizing。用户已选择安全方案：
先构造 `Box<dyn Fn>`，再通过 `Arc::from` 转换，不采用 unsafe raw 转换。三处闭包已完成适配；
当前 portable 路径每次 annotation 构建增加 3 次临时 Box 分配，实际性能差异尚未测量。

原子能力探测无法决定宿主的安全模拟方式。无原子目标由宿主明确配置 portable-atomic 所需的
回退机制，例如适用的 `critical-section` 实现；库不默认启用单核假设，不自行屏蔽中断，也不
自行安装临界区实现。缺少必要前提时必须编译失败并给出可定位的配置诊断，不静默降级。

本机查询确认 `thumbv7em-none-eabi` 支持指针宽度原子操作，`thumbv6m-none-eabi` 和
`riscv32i-unknown-none-elf` 不支持。这些查询不代表已经对这些目标完成编译或运行验证。

### 5. 浮点数学与依赖

通过内部统一入口封装所需的 floor 和 round 运算，`math-std` 保留原生方法，
`math-libm` 使用对应精度的 libm 函数。不得用整数转换或自制近似算法代替原有取整语义。

8 个调用点分布在 `fitted_line_pattern_geometry`、`layout_queries`、`line_measurement` 和
`width_independent_annotation_cache`，涉及线条拟合、命中测试、Tab 停靠及行长网格。

保留 ICU、hashbrown 和 log，不因 no-std 目标替换现有数据源或日志 facade。
ICU 与 log 可显式关闭默认 feature 并保留实际所需能力；hashbrown 的默认 feature 本身支持
no-std，不为形式上的 feature 最小化删掉现有代码所需的默认 hasher 等能力。

### 6. 统一采用 OnceBox

已将 `src/core/text.rs` 的 scalar 索引和 `src/linebreak/english_hyphenation.rs` 的全局英文
断词表两处 `OnceLock` 统一替换为 `once_cell::race::OnceBox`，std 与 no-std 使用同一实现。
保持现有惰性初始化和共享生命周期，初始化闭包通过 Box 返回数据；调用方继续取得共享引用。

通常调用发生在单个线程，预计首次访问竞争较少。这是成本判断依据，不是安全前提：仍保留
`Send + Sync` 和并发访问能力。竞争时允许多个执行者分别初始化，由原子发布操作保留一个
结果；发布失败的候选释放，成功返回的调用取得同一个已发布值。初始化函数必须允许重复
执行，不引入要求恰好执行一次的外部副作用。

接受每个成功初始化的 cell 增加一次 Box 分配及额外间接访问。文本索引的成本作用于实际
初始化索引的底层 `TextInner`，克隆与切片继续共享同一索引，不为每个视图分别分配。
英文断词表是全局缓存，额外 Box 只需保留一份；首次竞争仍可能暂时增加解析工作和峰值内存。

选择 OnceBox 以避免将完整文本扫描或英文词典解析包在临界区内。有原生原子的目标无需为
cell 额外提供临界区实现；无原子目标仍需 portable-atomic 的宿主回退配置，分配器自身也可能
使用临界区。这里不承诺整个初始化无锁或无中断屏蔽，只要求 cell 不围绕完整初始化取得互斥保护。

初始化闭包 panic 时不会发布其候选；在支持展开的环境中，该次调用的 panic 向上传播，其他
调用仍可发布结果或之后重试。panic-abort 环境遵循宿主终止策略，不提供重试保证。
初始化闭包不递归初始化同一个 cell，避免无限递归；不通过关闭模块或删除功能绕过迁移。

## 分阶段实施

各 phase 按工程依赖顺序推进，不要求阶段结束时程序可编译、可运行或全部测试通过。
中间状态允许调用方尚未迁移、测试配置尚未接通；不为阶段性可运行添加临时兼容层、占位实现
或功能裁剪。最终完成时必须满足下文完整验收要求，中间产物不作为 no-std 功能交付。

实施中可用局部编译和测试定位问题，但不将每阶段全量验证设为推进门槛。
每阶段结束更新本节状态，记录完成内容、遗留接线与已知问题，明确下一阶段的接续位置。
遇到需要改变已确认设计的情况再讨论，不因常规编译接线问题反复停下来确认。

### Phase 1：feature 与基础适配入口

状态：已实施。依赖：现有调研与设计。

1. 调整根 manifest 的默认 feature、两个预设、细粒度选择及依赖配置，加入必要的冲突检查。
2. 在 crate 根接入 alloc 与条件 no-std，建立统一的 Arc 导出和内部数学入口。
3. 配置 once_cell 的 alloc 能力及无原子目标的 portable-atomic 接线，统一原子回退依赖。
4. 已明确宿主负责原子回退，不自行选择不安全单核假设；目标验证未完成，用户暂不添加无原子验证目标。

阶段产物：后续模块可以依赖的配置和统一入口。此时允许原有模块仍引用 std 或旧类型，
不要求 workspace 与 no-std 路径构建通过。

### Phase 2：核心模块迁移

状态：已实施。依赖：Phase 1 的入口与类型选择。

1. 在用户原有修改上完成 core/alloc 路径及类型、trait、宏的显式导入，处理单元测试作用域。
2. 将内部及公开签名中的 Arc 统一接到公共导出，保留线程安全约束和共享身份语义。
3. 将两处 OnceLock 替换为 OnceBox，保留惰性索引、共享切片与内置英文断词。
4. 将 8 处浮点取整接到统一数学入口，覆盖生产和 debug-info 路径，不改变布局算法。

阶段产物：核心源码迁移完成。测试 crate、demo 消费方和 CI 的接线问题留给后续阶段处理，
不为保持它们在本阶段可运行而保留第二套核心实现。

### Phase 3：消费者、必要测试与 CI 接线

状态：已实施。依赖：Phase 2 的公开类型与核心配置。

1. 调整 tests 的 feature 转发与默认依赖，将 demo 依赖及其字体测试设为可选；demo、bench
	保持 std，按需适配公共 Arc 导出等接口变化。
2. 补数学边界测试，检查已有 Text 索引覆盖，仅在缺失时补共享视图首次访问顺序的回归。
3. 复用现有集成测试与 golden，整理各合法配置的运行方式，不复制测试集、不重测 OnceBox 内部机制。
4. 在现有 workflow 新增只选择主库和 tests 的 no-std 任务，保留原 std 任务并隔离 demo 依赖。

阶段产物：最终验证所需的测试入口与 CI 配置。新增 CI 任务在最终验收前允许尚未通过，
其命令应能在本地复现，不依赖远程运行来完成基本调试。

### Phase 4：集中验证与文档收尾

状态：本地桌面验证和使用文档已完成；闭包转换阻碍已通过用户选择的安全 Box 方案解决。
用户暂缓裸机验证，迁移前后性能对比尚未完成，故 Phase 4 尚未完成。依赖：Phase 1 至 Phase 3 的全部接线。

1. 运行下文构建矩阵，核对实际依赖 feature，修复 std、no-std、自定义组合及非法组合诊断。
2. 运行主库和 tests 回归、golden 与原 std workspace 测试，要求最终程序可运行、可测试，
	布局行为及 golden 零 diff；在本地执行新增 CI 任务的等价命令。
3. 完成裸机目标编译及可获得的宿主回退验证，分别记录编译、链接和目标运行证据；
	缺少必要环境时明确标记阻塞，不将未验证路径写成已通过。
4. 按既定范围检查时间与分配成本，更新开发指南、公开类型说明及本迭代的实际结果，
	最后审阅完整 diff 与编辑器诊断。

阶段产物：实现、测试、CI 配置和使用说明一致的完整交付；满足验收后才将迭代标记为完成。
本地验证与远程 CI 执行结果分开记录，远程操作由用户负责。

## 验证与验收

### 构建矩阵

所有组合隔离构建核心 `tiqian` crate，避免 workspace 其他成员的默认 feature 合并掩盖问题。
以下合法组合分别覆盖关闭和开启 `debug-info`；互斥设计不采用 `--all-features` 作为成功检查。

| 类别 | 验证组合 |
| --- | --- |
| 默认路径 | 默认 `std` 预设，hashbrown 与原生数学；`cargo check --all-targets` 通过 |
| 自定义 std | std 集合与 libm；hashbrown 与 libm；显式可移植 Arc |
| no-std，有原子 | `no-std` 预设，以及不使用预设的等价细粒度组合 |
| no-std，无原子 | 自动可移植 Arc，宿主提供同步回退；缺少回退配置时检查失败诊断 |
| 非法组合 | 互斥预设、重复实现、缺少实现、`no-std` 与标准库能力冲突 |

`wasm32-unknown-unknown` 下的 no-std check 已通过，但该目标不是裸机验证证据；它不能证明裸机目标或无原子回退已验证。
目标验证计划覆盖至少一个不提供 std 的有原子目标和一个无原子目标，目前均未完成。
用户暂不添加无原子验证目标，该项保留未验证，不据此将整个迭代标记为完成。
`wasm32-unknown-unknown` 支持 std，不能单独作为无 std 依赖的证据。裸机库编译检查之外，
无原子回退还需由带真实宿主同步实现的最小消费者验证链接与行为，不能仅验证配置分支。

### 行为与性能

- 复用核心单元测试、`tiqian-tests` 集成测试及 fixture golden，在代表性 feature 组合下分次运行，
	要求布局行为与 golden 零 diff，不为不同实现复制测试或新增一套样张。
- 仅新增数学封装的必要边界测试，覆盖正负半整数、整数边界、负零与非有限值；同一份测试分别
	在 `math-std` 和 `math-libm` 下运行。Tab、网格、线条与命中测试优先复用已有覆盖。
- 检查已有 Text 索引测试；若缺少父文本与共享切片按不同顺序首次访问索引的覆盖，仅补对应回归。
	保留现有 `Send + Sync` 编译期断言，并在所选配置下复用相关测试。
- 不单独重测 OnceBox 内部的竞争发布、候选释放和 panic 重试机制；这些由上游库保证。
	本项目验证自身初始化接线和共享行为，不因本次移植新增硬件测试框架。
- 对比 Text 构造、首次索引、切片、英文断词表冷启动及布局热路径的时间和分配次数，遵循开发指南的测量条件。

### no-std CI

在现有 `.github/workflows/test.yml` 中新增独立 no-std 测试任务，保留现有 std 构建和测试任务。
新增任务仅选择主库 `tiqian` 与测试 crate `tiqian-tests`，不选择 demo 或 bench，也不使用
workspace 全体构建。测试 harness 和 runner 可使用 std，被测试核心库不得启用 `runtime-std`。

`tiqian-tests` 对 `tiqian-demo` 的依赖及使用其字体后端的 `tests/phase_five_white_space.rs`
已由默认启用的 `demo-tests` feature 控制；该 feature 自动启用 `std`，仅用于 std 字体集成测试。
no-std 任务关闭默认 feature，排除该路径，
复用其余现有集成测试和 golden。demo 保持依赖 std，不要求迁移其字体后端或桌面运行代码。
仅指定测试包不会自动排除其传递依赖，因此需要实际切断此依赖路径。

测试 crate 对核心库关闭默认 feature，通过测试配置转发 std 或 no-std 选择并保留所需的
`debug-info`。no-std CI 分别运行主库单元测试和不依赖 demo 的集成测试；检查最终依赖 feature，
确保没有通过其他路径重新启用核心库的 std 预设或 `runtime-std`。

桌面上的 no-std 配置测试提供可执行的行为回归，裸机目标编译检查提供目标依赖证据；两者
分别记录。无原子回退的编译、链接和目标运行证据也分别记录，不把桌面可移植 Arc 测试视为
无原子硬件回退已经运行通过。

### 文档与检查

已更新 `docs/dev-guide.md`，说明 feature 选择、公共 Arc 接入、宿主前提及初始化成本。
改变稳定架构取舍时按需新增 ADR、
同步关键差异说明，不改写历史迭代。检查编辑器诊断与目标 diff，记录实际运行结果。

## 兼容性与风险

默认环境继续为 std，集合继续为 hashbrown；core/alloc 路径迁移保持类型身份。
feature 配置新增默认 `std` 预设，自定义消费者需要关闭默认 feature 并选择合法组合。

可移植 Arc 与标准 Arc 的类型身份不同，公开签名必须统一导出并说明。Cargo feature 合并可能
使不同消费者请求的实现发生冲突，此时明确报错，由依赖图使用方统一配置。

no-std 不保证低内存、硬实时、无分配失败或无 panic；本迭代不扩展为运行时输入校验或无堆分配
重设计。目标没有原生原子操作时，性能和安全依赖宿主同步机制，必须保留实际证据。

## 回滚

按组件撤销本迭代引入的 feature 接线、数学封装和共享指针替代，恢复原默认 hashbrown 与 std
路径，将 OnceBox 恢复为原有 OnceLock；移除仅由本迭代引入的依赖。已确认等价的 core/alloc
路径整理可单独保留。
回滚不得覆盖用户原有修改或其他任务变更，也不修改历史迭代记录。

## 当前完成记录

### 已实施与共识

- manifest、alloc、数学封装、公共 Arc 与两处 OnceBox 迁移已完成。
- 测试 feature 已隔离 demo，no-std CI 已接线；开发指南已补充使用说明。
- portable Arc 的三处闭包已按用户选择采用安全 `Box<dyn Fn>` → `Arc::from` 转换；每次 annotation 构建增加 3 次临时分配。
- 用户暂不添加无原子验证目标；该路径保留未验证，不标记整个迭代完成。

### 本地验证已通过

| 配置或检查 | 结果 |
| --- | --- |
| 默认 std：`cargo check --all-targets` | 通过 |
| 默认 std：最终重跑 `cargo test --all-targets` | 通过 |
| `wasm32-unknown-unknown` no-std check | 通过；该目标不构成裸机验证证据 |
| `no-std` 预设 | 主库 26 项、tests 主入口 1364 项测试通过 |
| 独立 no-std 测试入口 1、17、20 | 全部通过 |
| 无预设：`collections-hashbrown,math-libm,shared-auto,debug-info` | 主库 check 通过 |
| 默认 std 与 no-std 主库 check | 通过 |
| 非法 feature 组合 | 按预期编译失败 |
| no-std 依赖树 | 未启用 `runtime-std`，未引入 demo |

### 当前性能样本

bench 与 demo 已增加主库 `std` / `no-std` 配置转发，默认继续选择 std。
bench 关闭默认 feature 选择 no-std 时，demo 依赖也关闭默认 feature 并转发同一配置，避免
重新启用主库 std。bench 和 demo 自身继续使用 std，no-std CI 范围保持主库与 tests。
已运行 no-std 分配探针，参数为 `iterations=30`、`warmup=10`、`widths=672`：
每页中位数为 49,160 次 alloc、2,026 次 realloc、9,394,252 bytes；默认 bench 全目标检查通过。

当前样本使用 `iterations=10`、`warmup=2`、`widths=672`、`scale=1`：layout 最小值
2.148 ms，中位数 2.171 ms。独立 `count-allocations` 每页分配统计中位数为
49,160 次 alloc、2,026 次 realloc、9,394,252 bytes。分配统计为独立运行，计时不可比较。
没有迁移前基线，因此不宣称性能不变；Text 单项及冷词典迁移前后对比尚未完成。

### 未完成

| 项目 | 状态 |
| --- | --- |
| 裸机目标验证 | 未运行；无原子目标暂不添加，编译、链接及目标运行均未验证 |
| 远程 CI | 未运行；本地已通过对应测试命令 |
| 迁移前后性能对比 | 未完成；缺少迁移前基线，不宣称性能不变。当前性能样本仅作现状记录；Text 单项与冷词典对比尚未完成 |