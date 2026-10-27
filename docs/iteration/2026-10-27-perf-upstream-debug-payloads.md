# 上游诊断数据按 debug-info 条件编译

## 目标与范围

关闭 `debug-info` 时，移除上游纯诊断字段、文本构造、记录集合与复制；开启时保持现有诊断输出。覆盖字体度量与 fallback reason、shaping 诊断载荷、角色解析、断点诊断及标点原因文本。实际参与布局判断的原因枚举、缺字与能力证据、断点、字体选择和几何数据继续保留。

## 设计

1. 使用 `#[cfg(feature = "debug-info")]` 同时控制字段与产生表达式，确保关闭时不执行 `format!`、文本切片或 clone。
2. 混合结构保留运行时字段，将成组诊断数据集中在独立结构中，仅对一个 `debug` 字段条件编译；纯诊断集合连同上游传递路径一起条件编译。
3. shaping、cluster 几何与角色解析的运行时构造接口保持固定签名，诊断数据在完整条件编译块中附加。简单的单字符串字段继续使用字段和产生点条件编译。跨 crate 使用主库导出的 `with_debug_info!(开启表达式, 关闭表达式)` 选择诊断代码，避免 Cargo feature 合并后调用方与主库的条件不一致。demo 与 bench 同时提供 feature 转发。
4. 保持所有布局数值、glyph、查询与绘制行为，开启时 golden 保持不变。

诊断字段随 feature 存在，运行时构造签名保持稳定。关闭态不构造诊断数据，不使用占位字符串、惰性闭包或兼容适配层。

## 验证

- 分别检查默认、`debug-info` 与 no-std 主库，运行核心、集成和 demo 测试。
- fixture golden 零差异；补充关闭态针对性回归，覆盖诊断字段混用过的实际行为。
- 使用同配置 release benchmark 比较分配，并用保存的前后产物交替计时。计数运行的时间不用于比较。
- 检查编辑器诊断、目标 diff 和中文文档措辞。

## 基线

默认关闭诊断，宽 672，预热 10 次，测量 30 次，每页 47 次布局：

| 指标 | 优化前 | 优化后 |
| --- | ---: | ---: |
| alloc_calls | 49,152 | 40,613（-17.4%） |
| realloc_calls | 2,026 | 1,900（-6.2%） |
| allocated_bytes | 9,393,868 | 7,252,454（-22.8%） |
| window_live_bytes | 687,171 | 674,403（-1.9%） |

优化前二进制保存在 `target/release/paragraph-layout-bench-before-reason.exe`。`window_live_bytes` 是布局窗口净存活分配，不是进程驻留内存。

不启用分配计数，预热 20 次、测量 200 次，按优化前／优化后／优化前／优化后交替运行：

| layout（ms/页） | 前 A1 | 后 B1 | 前 A2 | 后 B2 |
| --- | ---: | ---: | ---: | ---: |
| min | 2.120 | 1.934 | 2.101 | 1.917 |
| median | 2.222 | 1.992 | 2.168 | 1.991 |

两组 min 均下降约 8.8%。B1 的 p95 受调度波动影响升高，耗时结论采用同轮 min；分配计数与字节下降是确定性结果。本轮主要减少中间临时数据，最终结果窗口内存降幅较小。

## 回滚

撤销本轮条件编译改动即可恢复原有上游诊断成本；运行时数据模型和布局规则保持原样。

## 状态

实施与验证完成。

- 字体度量、字体 fallback、引号与破折号角色原因，shaping 的文本与 feature 说明按 feature 裁剪。
- role override、断点、强制换行、零宽断点、附着标点和 repair 候选诊断集合连同构造及复制一起裁剪。
- justification fallback、标点 fallback 与 placement 文本、几何诊断文本同步裁剪；保留注音和装饰需要的几何数值。
- shaping 缺字、ink bounds 与 capability 证据、emergency reason、已选 repair 和实际 allocations 保留。能力报告的 host 元数据继续公开。

验证覆盖 workspace 全目标、默认主库、no-std 主库与集成测试、默认 demo、开启诊断的 bench。集成测试 1,364 项通过，fixture golden 未更新；主库新增三项双态回归，验证缺字回滚、coverage 未验证回滚和成功显示替换。主库共 29 项测试、默认 demo 的 9 项库测试和 20 项程序测试通过。

编辑器检查和 `git diff --check` 通过，仅有既有未使用代码警告。中文文档执行措辞检查。

### 诊断分组重构

复审后将逐参数条件编译集中到诊断结构与构造块：

- `ShapingDecisionDebugInfo` 集中 shaping 文本和说明，`ShapingDecisionInfo::builder(range)` 固定签名。
- `ClusterGeometryDebugInfo` 集中源文本、显示文本和 placement 原因，几何数值继续直接供运行时消费。
- `RoleResolutionDebugInfo` 统一引号、破折号的来源与原因。常量标签正常传入，转换与动态格式化在诊断边界执行。
- builder 在内部暂存可选诊断载荷，提供完整载荷时直接使用；仅未提供时构造默认值。生产构造显式填写字段，避免默认空 `Text` 分配后被覆盖。

重构后再次通过 workspace 全目标、默认/no-std 主库各 29 项测试、默认 demo 测试和开启诊断的 bench 构建，1,364 项集成测试及 fixture golden 保持原样。本次仅迁移既有测试接口，没有增加测试。

相同参数复测关闭态：`alloc_calls=40,613`、`realloc_calls=1,900`、`allocated_bytes=7,252,454`、`window_live_bytes=674,403`，与分组前完全相同。先前交替计时属于分组前的性能记录，本次复测确认分配成本保持不变。