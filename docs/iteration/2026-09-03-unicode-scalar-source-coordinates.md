# 2026-09-03 Unicode scalar source coordinate 迁移

## 目标

将 tiqian-rs 的 source coordinate 从 UTF-16 code unit 迁移为 Unicode scalar value。迁移覆盖
核心输入、布局 pipeline、输出、交互查询、测试 fixture 和 deterministic golden。

本迭代落实 [R0001](../adr/R0001-unicode-scalar-source-coordinates.md)。仓库仍处于开发阶段，
直接切换公开 Rust API 与本地测试资产，不保留 UTF-16 兼容接口或旧 golden。

## 当前状态

截至 2026-09-04，Phase 1 至 Phase 6 已完成。核心生产代码、
demo、`src/**` 内嵌单元测试、`tests/**` 独立测试和本地 fixture/golden 均已切换到 Unicode scalar
source coordinate。

## 范围

### 包含

- `Text` 的 scalar offset 到 UTF-8 byte offset 映射和 scalar-safe slice；
- source range、span、cluster、glyph、line、break offset、debug decision 与 query 的 scalar 语义；
- interaction boundary、cluster role、quote、标点、断行、断词和 paragraph shaping 的 scalar 步进；
- demo、测试辅助代码、53 项本地 fixture 和 golden 的 scalar 坐标更新；
- 公开 API 与相关文档的术语更新。

### 不包含

- Kotlin、JavaScript、Apple 或其他宿主的 UTF-16 互操作层；
- Kotlin fixture、golden、JSON exporter、recorded shaping evidence 或跨仓库比较；
- grapheme segmentation 规则的重新设计；
- 排版规则、字体策略、断行策略或 shaping 后端的无关重构。

## 设计基线

### 坐标职责

| 坐标 | 责任 |
| --- | --- |
| Unicode scalar offset | Rust source API、range、span、cluster、line、break、query 与 debug。 |
| UTF-8 byte offset | Rust `str` 的子串范围、HarfBuzz cluster 和第三方文本 API。 |
| source interaction boundary | selection、emoji shaping、紧急断行、东亚间距和 word selection。 |

source range 一律为半开区间 `[start, end)`。source interaction boundary 是 scalar offset 的一个受
Unicode 规则约束的子集，不能用任意 scalar boundary 替代。

### 关键实现原则

1. `Text` 保持 UTF-8 存储；不为每段文本建立 scalar 字符串副本。
2. source coordinate 与 UTF-8 byte offset 使用显式转换，不能以 `usize` 或 `i32` 隐式互换。
3. pipeline 内的 range 与 break offset 必须原子切换；不得出现 scalar range 配合 UTF-16 map key
   或 break offset 的中间设计。
4. 组合标记、CRLF、区域指示符对、Hangul 序列、emoji modifier 和 ZWJ emoji 的 interaction 规则
   保持现有语义。
5. fixture 与 golden 直接更新为 Rust scalar 语义；不维护 Kotlin 输出兼容或转换路径。

### 已确认的详细设计

#### Source offset 与 range

- 新增公开的 `ScalarOffset(i32)` 类型。它表示从 source text 起点开始的 Unicode scalar 数量，
   不是 UTF-8 byte offset、UTF-16 code unit offset 或容器下标。
- 保留 `TextRange` 名称和半开区间 `[start, end)` 语义，其 `start`、`end` 及公开访问器使用
   `ScalarOffset`。不为这个纯 Rust API 改名为 `ScalarRange`。
- `ScalarOffset` 继续以 `i32` 承载数值，以便与既有 layout、line breaker 和调试模型中的计数类型
   一致。转换到 `Vec`、slice 等 Rust 容器边界时显式转换为 `usize`。
- `scalar_offset(i32)` 与 `text_range(start, end)` 在调用点明确标识 Unicode scalar source coordinate；
   `text_range` 构造半开 scalar source range `[start, end)`。
- 不新增全局输入预扫描、通用 `validate` 或每次 slice 的防御性校验。调用方提供的 span、
   `source_boundaries` 与 range 必须遵守既有 API 不变量；pipeline 内部产生的 range 由阶段间
   不变量保证。迁移只改坐标单位，不以额外运行时检查改变输入可表达范围。

#### `Text` 与 byte 边界

- `Text` 继续共享 UTF-8 `String`。第一次需要 source coordinate 转换时，懒构建并缓存
   `ScalarIndex`；纯字符串传递和不访问 source range 的路径不支付索引成本。
- `ScalarIndex` 使用一个长度为 `scalar_len + 1` 的 `Vec<u32>`，第 $n$ 项是第 $n$ 个 scalar
   boundary 对应的 UTF-8 byte offset。首项为 `0`，末项为全文 byte length。
- scalar 到 byte 使用表的直接访问；HarfBuzz 等返回的 byte boundary 通过该表二分查找得到
   scalar。非 scalar boundary 的第三方 byte offset 继续视为后端约定错误，不向 source API 泄漏。
- `Text` 子视图保存 source scalar 起止位置和 UTF-8 byte 起止位置，并与全文共享 `ScalarIndex`。
   子视图的局部 scalar offset 先平移到全文 scalar offset，再进行 byte 映射。
- 删除 UTF-16 unit 缓存、UTF-16 与 byte 双向表、代理对中点标记和 `*_compat` 文本访问 API。

#### 字符串访问与交互查询

- 删除 `impl Deref<Target = str> for Text`。`Text::chars()` 用于 Unicode scalar 顺序遍历，
   `Text::scalar_indices()` 返回当前视图内的 `(ScalarOffset, char)`；`Text::byte_len()` 返回明确命名的
   UTF-8 byte 长度。`Text::scalar_len()` 是 source coordinate 的长度来源，禁止以 `byte_len()` 表示
   source 长度。未在 `Text` 上定义的 Rust 字符串 API 继续通过 `as_str()` 调用，避免无选择转发。
- 保留 `Text` 与 `str`、`&str`、`String` 的双向 `PartialEq` 实现。测试和业务代码可继续使用
   `assert_eq!(text, "内容")` 这类内容比较；该便利接口不暴露 `str` 的 byte-length API。
- shaping backend、`str` slice 和外部库只交换 UTF-8 byte offset。byte 到 scalar 的转换集中在
   `Text` API，HarfBuzz cluster 不得作为 source offset 继续传递。
- caret、hit-test、selection 和 query 返回的 source offset 一律是 source interaction boundary。
   组合标记、CRLF、区域指示符对、Hangul 序列、emoji modifier 与 ZWJ emoji 内部都不是可返回的
   caret 位置；这里以算法输出不变量保证，不增加运行时 validate。
- `get_cursor_rect`、`get_line_for_offset` 和单点 `get_bounding_box` 收到 interaction boundary
   内部的 scalar offset 时，先吸附到最近边界；与两侧距离相等时取后一个边界。带
   `SourceBoundaryBias` 的 `coerce_selection_offset` 保持调用方指定的方向。
- `source_boundaries` 仍是调用方提供的独立边界语义。它参与已有 cluster 与几何逻辑，但不会把
   interaction boundary 内部的位置变成可编辑 caret。

#### Fixture 与 golden

- fixture 中难以人工计数的范围改用测试辅助函数按 scalar 计算，例如由文本片段或第 $n$ 次出现
   的片段生成 `TextRange`；简单 ASCII 或单 scalar 范围可保留字面量。
- local fixture 与 deterministic golden 直接更新为 scalar 数字。除坐标数字和由此修正的既有
   UTF-16/scalar 混用外，不接受无关排版决策变化。

## 已识别影响

| 区域 | 主要文件 | 调整重点 |
| --- | --- | --- |
| 文本与几何 | `core/text.rs`、`core/geometry.rs` | scalar range 类型、scalar 与 byte 映射、删除 UTF-16 索引路径。 |
| 输入与输出模型 | `core/text_model.rs`、`core/layout_model.rs` | span、cluster、glyph、line 和 debug range 一致切换。 |
| 交互 | `core/source_interaction_boundaries.rs`、`core/layout_queries.rs` | scalar 坐标上的 interaction-safe caret 与 selection。 |
| 排版 pipeline | `layout/cluster_role_resolution.rs`、`layout/paragraph_shaping_stage.rs`、相关 stage | surrogate 步进改为 scalar 步进，hyphenation offset 统一。 |
| Unicode 规则 | quote、dash/ellipsis、punctuation、number cohesion、line break | 删除代理项特判，保留原有规则优先级。 |
| 验证资产 | `tests/`、`tests/fixture_layout/`、demo | scalar 断言、fixture 输入和完整 golden 更新。 |

当前直接 UTF-16 索引 API 调用分布在 21 个 `src/` 文件，合计 139 处。迁移应按依赖方向完成，
不以逐文件恢复编译为目标。

## 实施阶段

### 测试迁移时序

`src/**` 中以 `#[cfg(test)]` 或 `#[cfg(test)] mod tests` 编写的就地单元测试，必须随其覆盖的
Phase 1、Phase 2 或 Phase 3 生产代码同时迁移。它们固定新 API 与局部 Unicode 算法，不留到后续
阶段集中修改。

Phase 1 至 Phase 3 不要求编译或运行测试。`TextRange`、`Text` 与 pipeline 的坐标切换会让尚未迁移的
`src/**` 调用点阻止测试 crate 编译；`tests/**` 中的独立单元测试、跨模块测试、fixture 与 golden 也可
暂时失效。完成全部 `src/**` 代码和就地单元测试后，先让 demo 编译并运行，以验证 `HarfRust` shaping、
绘制与公开调用路径；随后集中迁移 `tests/**` 与 fixture/golden，最后统一恢复测试运行。

### Phase 1：确定类型与 `Text` 基础能力（已完成）

1. 定义 scalar offset 与 range 的公开表示，确定 `TextRange` 的最终名称和字段类型。
2. 为 `Text` 提供 scalar 长度、scalar 与 byte 的双向映射、scalar-safe slice 和子视图。
3. 删除核心对 UTF-16 unit、代理对中点和 UTF-16 byte 映射的依赖。
4. 同步调整 `core/text.rs` 与 `core/geometry.rs` 的就地单元测试；覆盖 ASCII、BMP、补充平面字符、
   组合标记、ZWJ emoji、byte 映射和子视图。

### Phase 2：贯通数据模型与 layout pipeline（已完成）

1. 迁移 `LayoutInput`、authoring span、cache key、annotation、prep、`ShapingInput` 和 `LayoutResult`。
2. 同步迁移 cluster、glyph、line、hyphen offset、progressive break offset 与所有结构化 decision。
3. 确保不存在混用坐标的公开字段、map key 或 stage 参数；不为迁移增加全局输入验证层。
4. 同步调整受影响 `src/**` 模块的就地单元测试。

### Phase 3：迁移 Unicode-sensitive 算法（已完成）

按以下顺序迁移并保持既有规则：

1. source interaction boundary 与 cluster role resolution；
2. quote pair、contextual quote、dash/ellipsis 与 Unicode punctuation boundary；
3. paragraph shaping 的 structural cut、technical break、hyphenation 与 emergency break；
4. East Asian spacing、number-symbol cohesion 与基础 line break。

所有 surrogate `$+2$` 步进改为 scalar `$+1$` 步进。不得将 interaction 或 grapheme 规则降为
逐 scalar 边界。每项算法迁移同时调整其所在 `src/**` 的就地单元测试。

### Phase 4：迁移 demo 并确认可运行（已完成）

1. 更新公开 layout query、demo 与 shaping backend，使 HarfBuzz byte cluster 经集中 API 映射到 scalar。
2. 运行 `paragraph-demo`，确认编译、启动、shaping 和绘制路径可走通。
3. 本阶段不以 demo 的视觉结果、layout 结果或 dump 数字作为正确性验收；这些在最终测试与 golden
   验证时统一检查。

### Phase 5：迁移独立测试、fixture 与 golden（已完成）

1. 集中迁移 `tests/**` 中独立单元测试和跨模块测试的 `TextRange` 构造、offset、interaction boundary、
   query 与 selection 预期；删除已失效的 UTF-16 辅助用法。
2. 更新 53 项 fixture 输入、测试辅助函数与完整本地 golden。
3. 审阅 golden diff，确认坐标变化符合 scalar 语义，排版决策未出现非预期漂移。

### Phase 6：完成迁移与验证（已完成）

1. 搜索并处理核心和测试中的 UTF-16 API、`encode_utf16`、`len_utf16`、surrogate 与旧坐标术语。
2. 运行相关测试、`cargo test --test tiqian layout_fixture_golden_test`、`cargo test --all-targets` 与
   `git diff --check`。
3. 更新 `docs/key-differences.md`、`docs/tracking.md`、本迭代记录和 API 文档，使其与实际实现一致。
4. 对相关文档运行文档风格检查，并人工审阅输出。

Phase 1 至 Phase 5 之间允许项目暂时无法编译或运行；Phase 6 完成时必须恢复完整可用、可测试状态。

## 实施记录

### Phase 1 至 Phase 4

- `ScalarOffset` 和 scalar `TextRange` 已成为 Rust source coordinate 的统一表示；`Text` 负责
   UTF-8 byte 与 Unicode scalar 的集中映射，并提供 scalar-safe view。
- layout pipeline、交互查询、Unicode-sensitive 算法、demo 和 HarfRust byte cluster 映射已经完成
   scalar 化；核心不再保留 UTF-16 source coordinate、代理项中点或 `*_compat` 文本访问路径。
- 按正确性优先原则清理了若干顺序 scalar 扫描：line break、interaction extender、dash/ellipsis
   role context、number-symbol cohesion 等路径改用 iterator 或直接遍历文本 view；这些改动不改变
   已记录的排版规则。
- `cargo test --lib` 当前通过 16 项测试；`cargo test --example paragraph-demo` 通过 19 项测试；
   `cargo check --examples` 和 release demo 运行通过。
- 代码诊断和 `git diff --check` 已在相关修改后通过；当前仅保留 `paragraph_dp_line_breaker.rs`
   中既有的 `gap_boundaries` 未读取 warning。

### Phase 5 进入条件

- 不修改 Kotlin 仓库、独立测试之外的 fixture/golden 资产或已完成的生产 pipeline。
- 先盘点 `tests/**` 中的 UTF-16 辅助函数、`TextRange` 构造、offset 预期和 interaction boundary
   断言，再按模块迁移测试辅助代码与测试用例。
- 独立测试迁移完成后，再更新 52 项 fixture 和 deterministic golden，并逐项检查坐标变化与排版
   决策变化。

### Phase 5 完成记录

- `tests/**` 的独立测试与跨模块测试已全部切换为 scalar source coordinate；emoji、variation
   selector、emoji modifier、ZWJ、tag sequence 和补充平面字符的手写范围均按 scalar 重新计数。
- selection endpoint 与双击选词分别覆盖 interaction boundary 吸附和 raw scalar 所属 unit 查询，
   避免将 caret 的 nearest 规则错误用于选词。
- 原有 52 项 fixture 的显式 source range 均覆盖 BMP 或 ASCII 文本；以 scalar pipeline 重放后，
   它们的 deterministic golden 保持一致。新增 `unicode-scalar-source-coordinates` fixture 覆盖补充
   平面字符、组合标记、emoji style variation、emoji modifier、ZWJ、区域指示符、keycap 和 emoji
   tag sequence，并固定这类文本的 scalar range 输出。
- `cargo test --test tiqian -- --test-threads=1` 通过 1327 项测试；fixture golden 测试通过。

### Phase 6 完成记录

- 已搜索 `src/**`、`tests/**` 和 `examples/**` 中旧 UTF-16 API。生产代码不再包含 UTF-16
   offset 或 surrogate 中点逻辑；保留的 surrogate 字样仅用于 Unicode scalar 拒绝校验和历史测试名称。
- `cargo test --all-targets` 通过：独立测试 1327 项、`paragraph-demo` 测试 19 项，fixture golden
   测试包含在独立测试中并通过。
- 已运行 `git diff --check`、编辑器诊断和相关文档风格检查。`paragraph_dp_line_breaker.rs` 的
   `gap_boundaries` 未读取 warning 以及 demo 未读取 overhang 字段均为本迭代前既有问题。

## 验证与验收

- 每个 source range、break offset、cluster range 与 query offset 都使用 scalar coordinate；caret 与
   selection offset 同时必须是 interaction boundary。
- Rust 核心不再维护 UTF-16 source coordinate 或代理对中点语义。
- byte offset 仅保留在 `str` 和 shaping 边界，且经集中转换进入 source coordinate。
- interaction boundary 回归覆盖并保持 selection、emoji shaping 和 emergency break 的安全性。
- 所有本地 fixture golden 与 Rust 测试通过；golden 的改动经过逐项审阅。
- 迁移不引入 Kotlin checkout、fixture 或 golden 测试依赖。

## 回滚

当前已完成 Phase 1 至 Phase 4，下一阶段为 Phase 5。若迁移独立测试、fixture 或 golden 时发现
scalar source coordinate 无法满足 Rust 公开 API 或 layout 不变量，应先停止在当前阶段，记录具体
约束并重新讨论 R0001；不得通过保留双坐标核心或临时 UTF-16 adapter 继续推进。