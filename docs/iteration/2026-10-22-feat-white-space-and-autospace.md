# 2026-10-22 空白处理与自动间距联合实施

> 状态：已完成，Phase 1–5 均已验收
>
> 决策依据：[R0007](../adr/R0007-css-autospace-and-explicit-spaces.md)、
> [R0008](../adr/R0008-white-space-policy-and-wrapping.md)
>
> 验收：全目标编译、全量测试、57 项 fixture golden、浏览器、仓库字体与性能对照均已完成。

## 目标与范围

在同一次迭代中完成自动间距模式调整和可配置空白处理，统一行中折叠、行首尾处理、换行计宽与
光标几何。R0008 对执行顺序和行边行为的细化优先于 R0007 中对应的早期约定。

- 自动间距默认 `Insert`：无空格时添加 gap，已有空格保留；`Replace` 对处理后的一个有效 U+0020
  替换为 gap，对保留的多个空格不替换，无空格时也添加 gap。
- 四种空白模式 `Collapse`、`PreserveBreaks`、`Preserve`、`BreakSpaces`，独立组合 `Wrap / NoWrap`；
  核心默认 `PreserveBreaks + Wrap`，多行输入框推荐 `BreakSpaces + Wrap`。
- 完成 Tab stop、换行序列、空格内部软换行、行尾悬挂与实际占位／溢出范围；原文、scalar range、
  复制内容保持既有约定，空白序列跨 style/source 分片一致处理。
- 接通公开 Rust 输入、布局、结构化 decision、查询及 replay，完成本地测试、fixture、golden 和真实字体检查。
- 仅修改 tiqian-rs；保持既有中西文分类、gap 预设、标点策略及自动间距抑制。元素级
  `white-space-trim`、完整 CSS 样式系统和输入控件本身不在本次范围。

## 实施方式

Phase 按工作依赖划分，各阶段共用一次最终验收。中间阶段允许接口暂未接通、
编译失败和旧测试失败；只在整个迭代完成时要求编译、运行与测试全部通过。

直接建立最终模型并依次替换调用路径，不为保持中间阶段可运行增加兼容层、双路径或临时 feature flag。
测试随所属 phase 编写，能执行的局部测试可随时执行；完整编译恢复、旧预期迁移和 golden 更新集中在
Phase 5。已知断点与下一步写入本文，避免反复编译同一批尚未接通的接口。

顺序为 Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5。后续发现前置模型不足时，直接调整同一迭代
中的模型和调用点；涉及 ADR 行为取舍变化时，先讨论确认。每次推进更新 phase 状态、完成项和剩余问题，
无需为每个 phase 单独创建实施文档。

## 现状与改动位置

| 模块 | 当前情况与本次责任 |
| --- | --- |
| `src/core/text_model.rs`、`src/api/` | `ParagraphStyle` 尚无空白与换行配置；增加公共配置与 builder 接入 |
| `src/layout/paragraph_shaping_stage.rs`、`paragraph_layout_engine.rs` | 接入 source 空白序列及显示投影，保留源坐标，确保可断行和交互边界不被 shaping 合并隐藏 |
| `src/clreq/clreq_profile.rs`、`src/layout/punctuation_geometry_stage.rs` | 当前 `Insert` 仍替换已有空格；按有效空白结果实施 R0007，修正涉及的旧注释 |
| `src/layout/line_break_planning_stage.rs`、`line_breaker.rs`、`paragraph_dp_line_breaker.rs`、`line_optimization.rs`、`line_repair.rs`、`progressive_break_decisions.rs` | 接入新的断行机会、候选行宽与 Tab 位置，移除无条件行尾折叠假设 |
| `src/layout/line_adjustment_stage.rs`、`punctuation_geometry_ledger.rs`、`line_geometry_stage.rs`、`justifier.rs` | 替换 `LineEdgeWordSpaceCollapse`，实现悬挂、对齐与最终占位，避免重复扣宽 |
| `src/core/layout_model.rs`、`layout_queries.rs`、replay 相关模块、`src/layout/layout_debug_assembly.rs` | 明确宽度字段与溢出语义，光标、选区、命中、replay 和 decision 共用布局结果 |
| `tests/layout/`、`tests/fixture_layout/`、`tools/text-autospace-probe.html`、`examples/paragraph_demo/` | 回归矩阵、dump、golden、浏览器对照与本地真实字体消费验证 |

表中是调查得到的主要改动位置，实施前仍需追踪具体调用与缓存键，不要求所有列出的文件都发生改动。

## Phase 1：确定公共模型与剩余细节

状态：已完成。产物：公共输入与空白记录类型，以及下文的字符规则、源映射和几何接口约定。

### 已确认规则与当前进度

- 公共输入使用 `ParagraphStyle.white_space: WhiteSpaceMode`、`text_wrap: TextWrapMode`、
   `tab_size: TabSize`，同时提供 builder 方法；类型沿用 `core::text_model` 公共路径。
   默认值为 `PreserveBreaks`、`Wrap`、`TabSize::SpaceAdvanceMultiple(8.0)`。
- LF、CRLF、独立 CR 为普通换行：`Collapse` 将它们转为可折叠 U+0020，其他模式保留硬换行。
   不根据相邻中文字符消除换行；CRLF 始终保持一个 source 交互单元。VT、FF、NEL、LS、PS
   始终保留硬换行，末尾硬换行继续产生末尾空行。
- `TabSize` 支持 `SpaceAdvanceMultiple(f32)` 与 `LayoutUnits(f32)`，参数要求非负。
   倍数基于段落默认字体的 U+0020 advance；间距零时 Tab 零宽。stop 原点为正文块始端，
   首行及整段缩进计入 pen 位置但不移动网格，混合字号不改变网格。
   下一 stop 距离小于 Tab 当前字体的半个 `0` 字宽时再跳一站；两端对齐改变文字位置后重新解析
   Tab advance，保持 stop 对齐。折叠模式中的 Tab 先转成可折叠 U+0020。
- 宽度无关 annotation 缓存键已加入 `white_space`，以区分后续空白投影和 shaping。
   `text_wrap` 与 Tab 间距由逐行阶段消费，不加入此缓存键。
- 空白转换和自动间距已接入；换行控制、Tab stop、行边处理和最终交互几何仍由 Phase 3–4 接入。
   完成这两阶段前，公共配置的完整行为尚不可用。

1. 在 `ParagraphStyle` 及 builder 设计空白模式、换行控制和 Tab stop 参数；让公开 API 能构造 ADR
   规定的全部组合，明确配置参与哪些布局缓存键。空白策略首先按段落表达，style 分片不截断空白序列。
2. 确定普通空格、Tab、LF、CRLF、独立 CR 与现有其他 mandatory break 的分类表；确定 `Collapse`
   的 segment break 转换规则。保留 source range，CRLF 的交互边界继续整体处理。
3. 确定 Tab 参数单位、默认值、stop 原点、最小前进距离以及缩进和混合字号的关系；先查既有能力，
   缺失处依据 CSS 形成明确方案。需要用户取舍的语义在本 phase 确认，未确认前不固化到算法。
4. 明确 `natural_width`、`adjusted_width`、`visual_width`、`LayoutResult.size` 的用途，以及调用方获取
   实际占位／溢出范围的接口。区分空白悬挂与已有标点悬挂，统一使用真实几何；不把空格宽度压为零模拟悬挂。
5. 确定空白处理结果和 decision 的最小内部表示：需要 source 到有效显示单元的映射、有效空格数量、
   保留／折叠状态与断行资格。复用既有数据通道，按实际需要新增类型，不复制完整段落或另建布局管线。

完成依据：上述规则和接口已写清，后续 phase 可以直接消费；相关类型允许尚未完成调用点迁移。

### 内部表示与阶段交接

`src/layout/white_space.rs` 定义两类记录；Phase 2 已生成 source 记录，逐行记录由 Phase 3–4 接入：

- `WhiteSpaceUnit { range, kind, action }` 按 source 顺序扫描生成普通空格、Tab、普通换行与固定硬换行，
   annotation 按 `TextRange` 建立索引，源顺序由 cluster 范围保留。
   每个空格、Tab、换行序列一条，CRLF 一条；对象覆盖范围作为不参与空白解析的独立单元。
   `Preserve` 保持类别，`ToSpace` 生成一个 U+0020，`Collapse` 被前一个有效空格吸收，`Remove`
   移除 source 换行两侧可折叠空白。后两者不生成有效空格。
- `LineWhiteSpaceAdjustment { cluster_index, action, occupied_advance, hanging_advance }` 属于一个
   候选行，记录行首尾移除、Tab 展开、无条件或条件悬挂。Tab 与悬挂可在同一 cluster 上分别记录，
   按先展开、再处理行边的顺序消费；最终 advance 直接赋值，悬挂部分只从测量中扣除，不重复扣占位。

有效 U+0020 数由类别与动作确定，不额外存一份计数或显示文本。折叠序列第一个有效空格继承其
source 起点样式，后续被吸收的 source offset 共用该空格的末侧位置。换行周围被移除的空白映射到
对应换行边界；行首尾被移除的空白映射到所属行起点／终点。软换行处沿用既有 offset affinity，
同一个 offset 的两侧位置由查询入口选择，CRLF 内部不增加停点。

Phase 2 在 shaping 前单次扫描生成宽度无关记录，跨 style 分片延续折叠状态；通过 source range
与现有 cluster/glyph range 关联。保留空白必须逐字符可寻址，但不要求每个字符单独调用 backend。
折叠后的单个有效空格和保留的各个空格分别参与 auto-space 判断；被吸收的 source 不截断有效邻接。
Shaping 后不通过原始 `cluster.text` 重新统计空格，使用这份记录及其投影。

断行资格由空白模式、有效类别和现有非空白禁则共同生成：`Preserve` 的空格／Tab 串内关闭软断点，
串末提供机会；`BreakSpaces` 每个保留空格／Tab 后提供机会；折叠模式使用有效空格机会，零宽折叠
source 不制造额外视觉行。`NoWrap` 在断行阶段关闭全部软断点，保留硬断点；不改变缓存中的空白记录。
行边移除、悬挂、Tab advance 和自动 gap 行边消失都保存在候选行结果，不回写 annotation 缓存。

### 宽度与实际占位接口契约

| 字段或接口 | 最终职责 | 接入阶段 |
| --- | --- | --- |
| `LineBox.natural_width` | 候选行起点下解析 Tab 后、repair 和行边处理前的自然宽度；不是段落固定前缀和 | Phase 3–4 |
| `LineBox.adjusted_width` | repair、行边移除及悬挂扣除后，两端对齐前的测量宽度 | Phase 3–4 |
| `LineBox.alignment_width: f32` | 两端对齐及 Tab 重解析后，排除空白与标点悬挂的最终对齐宽度 | Phase 4 |
| `LineBox.visual_width` | 最终 cluster advance 总和，包含保留空白和悬挂；不含 indent 与额外 `hyphen_advance` | Phase 4 |
| `LayoutResult.size` | 维持现有尺寸计算：width 为可见行末端最大值与 max-width 的较小值，height 为最后可见行 bottom；不作为溢出范围 | 保持现状 |

Phase 4 在 `core::layout_queries` 提供 `line_occupied_bounds(line: &LineBox) -> Rect` 与
`occupied_bounds(result: &LayoutResult) -> Option<Rect>`。逐行矩形为
`[indent, indent + visual_width + hyphen_advance] × [top, bottom]`，段落范围为可见行矩形并集，
不按 max-width 截顶，也不包含隐藏行。纯空白行保留宽度，末尾空行保留零宽行高，无可见行返回
`None`；段落查询只扫描行、不分配 cluster 位置。字形墨迹和注音外伸继续使用已有 ink／paint 查询。

两端对齐与末行居中／末端对齐消费 `alignment_width`，绘制、交互与占位查询消费最终 advance。
已有 `hanging_punctuation_advance` 继续只表示标点悬挂；空白悬挂由逐行空白记录表达。
上述字段生产、查询实现与调用方迁移属于 Phase 4，本 phase 完成类型和契约后再依序推进。

## Phase 2：空白序列、shaping 与自动间距

状态：已完成。产物：进入断行前的有效单元、源映射、间距及 shaping decision。

### 实施结果

- `scan_white_space` 单次扫描 source，跨 style/source 边界延续折叠；结果存于 annotation，基础与动态
   shaping 共用。每个空白记录拥有独立范围，CRLF 保持整体，对象覆盖范围不参加扫描。
- 有效普通空格由 backend 测量；转换的 Tab／换行使用 U+0020 shaping，再把 cluster、glyph 和
   decision 范围映射回 source。折叠／移除记录输出零宽 `SyntheticClusterKind::WhiteSpace`，
   保留 Tab 同样使用此类别但保留显示类别，等待 Phase 3 解析宽度，不生成 Tab 字形。
- auto-space 按有效显示内容判断；被吸收的 source 不截断邻接。`Insert` 保留显式空格，`Replace`
   仅替换一个有效空格，两者在无空格时均插入 gap；保留的多空格互相阻断替换。suppressed range
   和 inline box 的既有边界通道继续生效，原文保持不变。
- 方案 B 已接入：保留模式不跨空格串建立对象—标点附着；折叠模式使用有效空格与零宽折叠记录，
   直接相邻仍可附着。对象边界中断 CRLF，扫描和角色切分使用相同范围。
- 已确认词间空格收缩继续生效：`Preserve / BreakSpaces` 保留字符数量和位置，允许既有 repair
   压窄普通词间空格。自动 gap 与标点 glue 继续遵循各自配置。
- 新增 6 项集成测试，包含 192 个空白／间距组合，以及跨 style、CRLF glyph range、硬换行、Tab、
   suppressed range、对象覆盖及 NBSP／全角空格边界；扫描单元测试 4 项。旧默认预期仍留 Phase 5 迁移。

### 后续接入

Phase 3 消费有效空白及隐藏记录，处理软断点、Tab 候选行计宽和行边测量；Phase 4 完成 source stop、
悬挂、占位查询与专用 debug/dump。当前零宽 source 记录只表达源范围，最终 offset 位置仍需迁移。

1. 在 shaping 前识别空白序列，处理折叠、Tab 转普通空格及换行转换；保留源文本，按有效显示内容
   shaping，字体度量和 glyph replay 同源。保留空白不能因 backend cluster 分组而丢失逐空格位置。
2. 处理跨 source/style 边界的连续空格，记录折叠后有效空格的 source 归属及自然度量；被折叠部分仍
   有可解释的源映射。`BreakSpaces` 提供逐空格的断行边界，保留模式提供正确的交互边界。
3. 让 auto-space 消费同一份有效空白结果，完成 `Disabled / Insert / Replace` 全部组合；多个原始
   空格先折叠成一个有效空格时，`Replace` 可以替换。保留多个空格时完整保留。
4. 同步现有边界分类、suppressed range、inline box/object 及附着标点路径。审查其他具名空格压缩
   规则在保留模式下的适用性；如与“保留作者空白”冲突，先明确优先级，不以局部例外静默绕过。
5. 编写 source 映射和间距组合测试，修正 `clreq_profile.rs` 中与实际模式矛盾的注释；旧的默认行为
   测试预期统一在 Phase 5 迁移，避免重复更新。

完成依据：后续模块不再各自重新识别、折叠同一串空格；未接通断行或几何属于已知中间状态。

## Phase 3：候选行计宽与断行

状态：已完成。依赖 Phase 2。产物：与空白策略一致的 line solution 及已解析的逐行空白布局。

### 当前实现与交接

- `line_measurement.rs` 建立共享前缀和、空白串边界与 Tab 索引；普通候选计宽不分配逐空白记录，
   无 Tab 用前缀差，有 Tab 只遍历区间 Tab。最终行解生成 `LineWhiteSpaceAdjustment`。
   annotation 的 `WhiteSpaceUnit` hash 已接入 prep 和 breaker config。
- greedy、lookahead、Paragraph-DP、repair 和 progressive 计宽已接共享测量；NoWrap、保留空白串、
   逐空白断点、悬挂、自动 gap 行边消失及 Tab 网格已有定向测试。repair 按事件阈值重解析 Tab，
   保留 tier 顺序及同 tier 容量比例；DP 扩展候选先验证压缩实际可行性再进入 promotion 池。
- 本轮补齐 repair 边界约束、硬换行 source 吸收、压缩后尾空白记录和共享有效 gap 成本。
   修复共享 greedy 将行首零宽控制符独立成行的问题。Phase 4 最终几何消费未修改。
- 已核查条件悬挂：尾空白非负时，硬断／段末在只悬挂溢出部分后能容纳，当且仅当正文宽度能容纳。
   因此 compression 截到 tail、`shrink_is_effective` 排除尾空白无需区分 reason；完整行仍按真实
   reason 复测。新增采样包含硬断、段末、软断、纯空白、Tab 和尾空白机会，未发现过滤损失可行解。
- 修复压缩 profile 错用正文末端判断自动 gap 的问题：正文后有保留空白时，该 gap 仍在行内。
   旧逻辑会在试算中提前扣除 gap，最终复测不通过后用满容量，造成过量压缩。反证测试中旧逻辑
   压缩 0.75，修复后为所需的 0.5；完整行复测与逐空白记录保持一致。
- 已完成三种 breaker、repair、progressive 全路径审阅。无 Tab 且无 repair 分配的候选宽度为
   O(1) 前缀差；含 Tab 时只访问候选内 Tab，repair 另遍历分配。普通试算不生成逐空白记录，
   最终行解才生成记录。测量索引的各向量分别用于宽度、空白串、可见邻居、Tab、gap 或源范围统计，
   均在规划入口构造；压缩的 tier、固定分配和 profile 向量按机会构造，不按空白字符构造。
   DP 在共享测量存在时停止构建重复的宽度、gap 和容量索引；独立无 measurement 调用保持原路径。
   空行由独立入口生成，最终记录和 remeasure 跳过空 range；本轮没有新增生产 validate 或 assert。

### 当前验证

- `cargo test --test phase_three_white_space`：20 项通过，release 同样 20 项通过，包含三种 breaker、Tab 候选起点、正文块和
   首行缩进、严格半字宽阈值、零阈值／零 interval、repair 吸收与跳站、小 interval 和多 Tab 抽样、
   tier 容量比例、硬断尾条件悬挂、隐藏 source、DP promotion 可行性、条件悬挂穷举对照、
   正文尾 gap 的精确压缩和 progressive 引擎 Insert／Replace 对照。
- `cargo test --test tiqian phase_two_`：6 项通过；`cargo test --lib`：24 项通过；
   `cargo check --all-targets` 通过。涉及文件编辑器无诊断，`git diff --check` 通过。
- 完整 `cargo test --test tiqian -- --quiet` 最终重跑：1345 项通过、15 项失败，退出码 101。
   `cargo test --no-fail-fast` 也已执行：库 24 项、Phase3 20 项与 doc test 通过，tiqian 和独立
   golden target 失败；两个 golden 入口报告同一个 fixture 首差异。未更新旧断言或 golden。
- progressive 两项已用 Insert／Replace 和三种 breaker 对照。短句 `中 ab/cd` 保持 Syllable
   断点：Insert 的作者空格为 8px，首行正好 100px，无 justification；Replace 空格为 2px，
   具有 2px justification。长句 lookahead 在 Insert 下为 `Serialization … swift_`，显式
   Replace 恢复 `erialization … swift_dy`；技术串外 CJK 拉伸仍不超过 0.001，Emergency
   decision 仍存在。结果支持默认语义变化，未发现这两项来自 Phase3 缺陷或 Phase4 漏消费。
- 新回归曾因测试循环移动 ranges 出现 E0382，已改为借用复制，后续编译通过。旧 gap 逻辑反证
   运行按预期失败（0.75 对 0.5），恢复修复后通过。其余验证仅有既有 unused 警告；
   没有运行 fmt、指定 target-dir 或执行 Git 写操作。
- 文档风格脚本退出码 1，报告 15 处措辞候选，已逐条人工复核：真实字体／几何、接口契约、
   失败归因、空格归一和数值刚好填满均为具体技术表述；“跑通”命中来自“复跑通过”子串。
   无需改写或修改脚本词表；`git diff --check` 退出码 0，相关文件编辑器无错误。

### 完整集合的逐项失败归因

下表使用测试名末段；完整模块路径见测试日志。旧断言迁移和 golden 更新均留 Phase 5。

| 测试 | 失败依据与归属 |
| --- | --- |
| `auto_space_replaces_typed_space_at_cjk_latin_boundary` | 默认 Insert 保留 8px 作者空格，旧断言要求 Replace 的 2px |
| `autospace_still_fires_between_latin_and_cjk_text_even_with_punctuation_nearby` | 显式空格下 Insert 不生成 gap decision，旧断言要求两条 |
| `numbered_labels_have_different_typed_space_widths_before_cjk_and_latin` | `01 基础组件` 的空格实为 8px，旧断言要求 2px |
| `one_typed_space_becomes_one_autospace_gap` | 默认 Insert 保留空格，旧断言要求 2px 替换 |
| `two_typed_spaces_at_boundary_still_collapse_to_one_gap` | 新源映射逐空白记录，已无 text 为双空格的单 cluster；旧查找 unwrap 失败 |
| `three_typed_spaces_still_one_gap` | 同上，已无 text 为三个空格的单 cluster；默认折叠后的有效空格也保留 Insert 语义 |
| `inline_object_over_the_cr_walks_the_lf_with_a_cr_behind_it` | 对象覆盖 CR 后中断 CRLF，剩余 LF 是硬断；旧断言要求不硬断，属于 Phase2 已确定规则 |
| `typed_sino_western_spaces_stretch_in_tier_two` | Insert 保留作者空格后首行无 justification，旧测试直接索引第零条；Replace gap 的 tier 假设待迁移 |
| `tiny_technical_tracking_stays_below_the_rejection_threshold` | 软断行尾空格在选行时移除，首行吸收空格，range 从 0..5 变成 0..6；测量 96px，source 保留，属于 Phase3 目标行为 |
| `progressive_technical_break_falls_through_structural_tier_before_overstretching_outside_text` | Insert 首行恰好填满，Replace 对照恢复 2px justification，Syllable 断点不变 |
| `progressive_technical_emergency_is_exposed_by_current_line_stretch_not_full_measure` | Insert 作者空格较宽改变前行断点；Replace 对照恢复旧断点，Emergency 与正文拉伸约束通过 |
| `sino_western_gap_shrink_floors_at_eighth_em` | Insert 下作者空格不是替换 gap；共享计宽也在选行时排除行尾空格，旧 CarryPrevious／12px gap capacity 前提已改变 |
| `auto_space_gap_at_line_end_is_trimmed_like_any_line_edge_blank` | 保留的行内作者空格为 8px，测量为 72px，旧 Replace 断言为 66px |
| `authored_space_inside_verbatim_range_is_not_normalized` | control 在默认 Insert 下不再生成 Replace decision，旧断言要求一条；显式 Replace 的 suppressed 回归通过 |
| `layout_fixture_goldens_match` | 首差异为 `adjacent-curly-quote-list-context`；共享测量在选行时扣除行边 gap，改变旧断点／自然宽度及行调整记录；完整 fixture 迁移留 Phase5 |

Phase4 的逐行几何、空白记录消费与交互仍待接入；本次只验收候选计宽及 line solution，未据此
宣称完整公开布局或全部 golden 已通过。

1. 将软换行资格接入实际断行入口：`NoWrap` 保留硬换行并允许溢出；`Preserve` 只在整串空白结束处
   提供相应机会；`BreakSpaces` 允许连续空格内部换行。保证纯空白、超窄行和段末始终有进展。
2. 候选行计宽同时考虑行首尾移除、软换行无条件悬挂、硬换行／段末条件悬挂及自动 gap 的行边消失，
   输出足够的记录供最终几何重放；不能先按旧宽度选行，再靠最后 trim 改变适配结果。
3. Tab 使用候选行上的 pen 位置解析宽度；候选行起点改变时重新按 stop 解析。审查固定 advance
   前缀和与 DP 成本缓存的适用范围，为无 Tab 的常见路径保留快速计算，避免对每个候选重扫整行。
4. 在 greedy、Paragraph-DP、repair 和 progressive technical 成本中使用相同的空白计宽约定，
   移除 `progressive_break_decisions.rs` 等处“行尾空格必折叠”的假设。
5. 编写换行、Tab、悬挂与边界成本测试，覆盖相同文本在不同候选起点和不同宽度下的结果；记录算法
   时间与分配风险，必要时复用前缀索引或按行状态缓存，不引入无关算法重构。

完成依据：选行和实际行内空白占位有一致依据，Tab 的位置依赖已处理；最终 renderer 尚未接通可继续下一 phase。

## Phase 4：最终几何、交互与消费路径

状态：已完成。依赖 Phase 3。产物：公开结果可完整表达并消费新行为。

### 实施结果

- `line_adjustment_stage.rs` 消费候选行的空白记录，移除无条件 `LineEdgeWordSpaceCollapse`。
   行边空白直接赋最终占位，自动 gap、标点 glue、对象 discardable glue 分别消费，单 cluster
   不重复扣首尾。对齐前 Tab 按 repair 与行边处理后的 pen 解析，对齐后再解析一次。
- `line_measurement.rs` 复用空白串与 Tab 索引提供有效对齐区间、最终 Tab 和悬挂记录。
   Tab 使用正文块原点，首行和整段缩进参与 pen，对齐 inset 不移动网格；严格半个当前字体
   `0` 字宽阈值与 Phase3 相同。软断尾空白退出拉伸区间，真实 advance 保留。
- 含 Tab 的最终行在既有 justification 阶段与容量下求解预算。`justifier.rs` 共用同一机会收集
   函数，阶段仅收集一次；试算复用行内 delta 缓冲，不 clone 段落、不重复创建机会或原因字符串。
   Tab 终点随 pen 单调非减，两个至多 32 次的数值搜索先找不溢出的最大实际宽度，再取达到该
   宽度的最小预算，避免 Tab 吸收形成的平台上无效拉伸。无 Tab 行保持单次 justification。
   最终 delta、Tab pen 的累加顺序与试算一致，decision 的剩余 deficit 按最终宽度回填。
- `LineBox.alignment_width` 用于末行对齐；`visual_width` 为最终 advance 总和，包含空白与标点
   悬挂，不含 indent／hyphen。`adjusted_width` 使用对齐前实际几何扣除悬挂；`natural_width`
   继续来自候选起点的自然测量。`size` 算法保持不变。占位查询只遍历可见行，O(lines)、无
   cluster 位置分配，不按 max-width 截顶，保留纯空白宽度、尾空行行高及 hyphen。
- positioned、selection、hit 和背景装饰继续共用最终 cluster advance。折叠／移除 source 记录
   为零宽，各 offset 自然共用有效空格末侧或所属换行／行边端点；BreakSpaces 每空格独立。
   replay cursor 补齐 interaction boundary 吸附，CRLF 内部不产生可用停点，复制继续读 source。
   核心裁切授权加入空白悬挂，标点原有返回值不变。已核查示例 renderer 按 `draw_x` 重放，
   hyphen 使用 `indent + visual_width`，背景装饰使用核心 segment，无需前端补偿或改写。
- `LayoutDebugInfo.white_space_units` 按 cluster 源顺序输出转换／折叠／换行移除记录；
   `line_white_space_adjustments` 输出行边、最终 Tab 与最终悬挂，auto-space 保留既有独立 decision。
   fixture dump 增加上述记录及 alignment／occupied 输出；golden 未更新。

### 验证与范围

- 新增 `tests/phase_four_white_space.rs`，17 项测试覆盖四种模式、空行／单空格、条件与软断
   悬挂、fit／alignment／occupied／size 区别、Tab 加 justification、缩进网格、源映射与 CRLF、
   copy、selection／hit／replay、背景逐 cluster 几何、对象／标点空白边界、max-lines、尾 Tab
   悬挂记录、标点悬挂、阶段缓存跨预算等价，以及零／小 interval、多 Tab 跳站采样。
   debug 与 release 各 17 项通过。
- Phase3 20 项、Phase2 6 项、库测试 24 项通过；`cargo check --all-targets` 通过。
   `justifier` 定向集合为 80 通过、1 失败，失败为前表已有的默认 Insert 旧预期。
- 最终 `cargo test --no-fail-fast`：库 24、Phase4 17、Phase3 20、doc test 通过；
   tiqian 1345 通过、15 失败，失败名称与 Phase3 前表一致，退出码 101。
   独立 golden target 同样失败；当前首差异为 `basic-pause-stop` 新增 `line-geometry` 输出。
   golden 格式与既有行为预期统一留 Phase5，不以完整测试未通过宣称全量验收完成。
- 实施中发现并修复 Tab 吸收拉伸后仍报告零 deficit、首尾 auto-space decision 顺序变化、
   标点裁切返回值被 viewport 放大三项问题；另将 Tab 预算搜索下界改为零分配，覆盖初始
   deficit 拉伸触发跳站的情形。对应定向回归已通过。编译期间的闭包所有权、
   测试索引及单位构造错误均已修复，无新增生产 validate／assert。
- 热路径 review：最终记录与普通行几何为线性处理；含 Tab 求解最多固定 64 次行内数值试算，
   仅收集一次机会和一次行内缓冲，不重复扫描整段，不排序／clone 全段。Tab 密集文本的绝对
   性能测量及真实字体、浏览器对照仍属于 Phase5 的统一验收，本阶段未提前执行或宣称完成。

本阶段改动文件：`src/core/layout_model.rs`、`layout_queries.rs`、`layout_result_replay_index.rs`、
`layout_paint_bounds.rs`，`src/layout/line_adjustment_stage.rs`、`line_geometry_stage.rs`、
`line_measurement.rs`、`justifier.rs`、`layout_debug_assembly.rs`，以及
`tests/phase_four_white_space.rs`、`tests/fixture_layout/dump.rs` 和 `docs/dev-guide.md`。
Phase1–3 既有未提交改动保留；ADR Accepted / Pending、上游同步终点与旧 golden 不变。

文档检查：`git diff --check` 通过。风格脚本对两份改动文档执行，初次命中 23 处，
已将新增的“落地宽度”改为“最终宽度”；其余候选为具体字体／几何表述、接口约定、
失败分析和既有脚本检查记录，逐项人工复核后保留。未修改脚本白名单。

1. 使用逐行空白布局替换无条件 `LineEdgeWordSpaceCollapse`；检查单 cluster 独占一行时首尾重复
   扣宽的边界，保持标点 glue、inline object 和自动 gap 的消费互不重叠。
2. 让对齐及 justification 使用约定的测量宽度，同时保留悬挂空格的真实 advance；按 Phase 1 的
   模型输出实际占位／溢出范围，明确 max-lines 截断后的范围仅对应可见行。
3. 接通 positioned clusters、source stops、光标、选区和命中测试。`BreakSpaces` 中每个保留空格
   都有可用位置；软换行边界、末尾硬换行、纯空白行及被折叠空白的 offset 归属均有明确结果。
4. 检查 replay index、逐 cluster 背景／装饰与示例绘制是否误把用于对齐的宽度当成裁剪边界；只做
   消费新结果所需的修改，布局决定仍由核心提供，不构建输入控件。
5. 接通结构化 debug 和 fixture dump，分别呈现空白转换／折叠、行边移除、Tab stop、悬挂与
   autospace decision；编写几何与复制保真测试。

完成依据：输入到结果及现有消费路径已接通，进入统一编译恢复和验收；发现遗漏回到对应 phase 修正。

## Phase 5：统一集成、回归与文档收尾

状态：已完成。依赖 Phase 4。产物：整个迭代可编译、可运行、可测试。

### Phase5 实施与测试

- 旧测试明确区分默认 `Insert` 与显式 `Replace`。编号原文首空格均保留自然 advance；默认多个
   空格按逐 source range 验收折叠。替换、verbatim suppression、tier-two stretch、shrink floor
   及两项 progressive 算法测试显式使用 `Replace`，保留原核心断言。
- 对象覆盖 CR 后，LF 作为独立 mandatory break。尾空格在选行时被吸收后，technical 首行范围
   增加一个 source 单元；高拉伸拒绝对应 `Whitespace` tier，低拉伸仍不拒绝。行尾空格测试同时
   检查 8px glyph 自然 advance、12px justification 后占位与 `RemoveEnd`，不混淆两种宽度。
- `tests/phase_five_white_space.rs` 新增跨 style/source 的 192 组合、64 个 mode/wrap/换行组合，
   以及纯白、0.5/8/320px、两种 Tab 参数、NBSP、全角空格、尾 break、copy/cursor/replay 矩阵。
   该入口 13 项通过，其中 9 项来自复用的真实字体 backend 既有测试，4 项为本阶段新测试。
- 最终补充矩阵前的全量 `cargo test -- --quiet`：库 24、独立 golden 1、Phase5 12、Phase4 17、
   Phase3 20、主集成 1360 全通过；补充矩阵后 Phase5 13 通过。全目标编译已通过，最后完整复跑
   与文档检查结果在本节末补充。既有 unused 警告保留。

### Golden 更新及逐项审阅

按开发指南仅在 runner 调用期间设置进程级 `TIQIAN_UPDATE_LAYOUT_GOLDENS=1`，在 finally 中移除；
更新成功后立即以非更新模式复跑成功。53 个既有 fixture 加 4 个空白 fixture，共 57 个，每个都输出
greedy、lookahead、Paragraph-DP；注册表和文件名集合由既有 runner 检查。

所有既有 golden 均增加 alignment/occupied 等诊断输出；空白记录由独立行表达，旧无条件
`LineEdgeWordSpaceCollapse` 记录被替换。以下为排除这些诊断增删后的逐项分类：

| 分类 | Fixture 与审阅结论 |
| --- | --- |
| 仅诊断变化 | `adaptive-short-line-indent`、`adjacent-punctuation-spacing`、`ascii-brackets-in-cjk`、`ascii-point-mark-impossible-measure`、`basic-pause-stop`、`bibliographic-numeric-locator-break`、`bopomofo-tone-em-box`、`contextual-curly-quotes`、`ellipsis-and-dash`、`emphasis-marks`、`fallback-roles`、`first-line-indent`、`greedy-multi-line`、`interlinear-lines`、`justify-cjk-paragraph`、`justify-mixed-paragraph`、`latin-existing-hyphen`、5 个 `mandatory-*`、`mixed-script-quote-paragraph-language`、`nested-quotes`、`parenthetical-dash-pairs`、`progressive-technical-alpha-numeric`、`progressive-technical-hash-fill`、`ruby-line-height`、`unicode-scalar-source-coordinates`。断行、宽度与原有 decision 保持一致。 |
| 仅 repair/candidate 诊断变化 | `adjacent-curly-quote-list-context`、`ascii-point-mark-in-cjk`、`indent-opening-quote`、`justify-unbreakable-number-symbol`、`kinsoku-carry-previous`、`kinsoku-push-in`、`latin-camelcase`、`line-end-kinsoku`、`lookahead-avoids-repair`、`lookahead-future-push-in`、`zero-width-space-soft-break`。候选计宽已扣行边，原需最后处理才 fit 的候选可返回零分配 PushIn；空分配浮点 sum 的 `-0.0` 与零数值等价，几何不变。 |
| 默认 Insert 的自然宽度和拉伸变化 | `contextual-dash-ellipsis`、`latin-opaque-url-token`、`progressive-technical-inline`、`quote-digit-boundaries`、`western-hyphenation`：作者空格从旧 2px 替换 gap 恢复 8px，相关 deficit 与 allocation 相应调整；这组行范围保持不变。`quote-digit-boundaries` 最终宽度 730→742，两个空格各增加 6px。 |
| 行边 gap 及压缩 | `mi10s-adjacent-curly-quote-wrap`：三种 breaker 行范围保持，第三行候选直接扣行边自动 gap 后 fit，不再先缩 2px 又拉伸 2px。 |
| DP 源行范围变化 | `latin-hard-break`：`0-3/3-5/5-8`→`0-1/1-5/5-8`，行边自动 gap 的测量与成本改变；`latin-word-wrap`：第三行末端 43→42，空格归下一行后移除；`mi10s-western-bracket-citation-wrap`：`0-17/17-33/33-50/50-63`→`0-16/16-33/33-49/49-63`；`mourning-frame`：`3-6/6-9`→`3-7/7-9`，新的候选可容纳整段姓名；`unmatched-curly-quotes`：`0-13/13-28`→`0-12/12-28`，空格归下一行，前行分号行尾压至 8px。DP 现在在选路时使用可实际压缩的候选，不沿用旧前缀容量近似。 |
| 默认 Insert 与 DP 成本共同改变 | `real-paragraph-1`：greedy/lookahead 范围保持，含 espresso 行自然宽度 306→312；DP 中段范围重新选择，仍为 10 行、320×240。`progressive-technical-current-line-emergency`：greedy/lookahead `35-74/74-112`→`35-73/73-110`；DP `0-33/33-73/73-111`→`0-35/35-74/74-111`，仍为四行；新增默认作者空格宽度影响技术断点和 Emergency 拉伸。显式 Replace 的原算法回归仍通过。 |
| 新增 | `white-space-collapse`、`white-space-preserve-breaks`、`white-space-preserve`、`white-space-break-spaces`，使用同一编号、Tab、多空格、CRLF、尾 LF 原文。三 breaker 分别为 4/6/6/7 行；保留模式记录 Tab 与悬挂，BreakSpaces 保留逐空格占位。 |

上述变化的 source 范围、总行数、alignment/occupied 与相关算法回归已交叉检查；没有通过删除
核心断言掩盖失败。零收缩 repair 是诊断差异，未作为压缩性能收益。

### 浏览器与真实字体证据

- Windows VS Code 1.141.0、Electron 43.7.7、Chromium 150.0.7871.250 实际打开
   `tools/text-autospace-probe.html`。UA 已读取；CSS family 请求 `Microsoft YaHei, sans-serif`，
   autospace 字号 32px，空白样本 16px/24px、宽 64px、Tab size 4。未逐 glyph 识别实际 fallback face。
- autospace 四种 white-space 共 288 项，空白/wrap/alignment 共 240 项，结果暴露于
   `globalThis.autospaceProbe`。`normal` 的 `中A` 总宽 58.520836px，相对关闭增加 4px；
   pre-wrap 双空格样本与两条编号原文增量均为零。显式 insert/replace 组合值 CSS.supports 为 false，
   输出 null，不使用 fallback 结果验收 Replace。
- `aaaa` 后八空格：pre-wrap 的尾矩形 right=72.916672px，高 24px，start/center/end 均保留
   溢出空白；BreakSpaces 高 48px，尾空格在第二行。加末尾 b 时两种保留模式 Wrap 都是 48px，
   NoWrap 高 24px、scrollWidth=83px；normal/pre-line 折叠后高 24px。`a\t\tb` 保留模式
   b 右端约 47.760422px，折叠模式约 23.46875px。硬换行样本在 pre-line/保留模式高 48px。
   浏览器 DOM 末尾换行高度与 Rust 明确保留末尾空行的契约分别记录。
- Rust 复用 `DemoFontCatalog` 的仓库字体、HarfRust 与 Skrifa；两条编号原文 scalar 2..3 的
   Inter glyph 和最终空格均 4.5px，无 Replace decision，所有 glyph 有 face 且非 missing glyph。
   `Input` 后三空格在 Preserve/BreakSpaces 的 cursor x 为 42.742188/47.242188/51.742188px，
   逐格增加 4.5px，replay 与 cursor 相同，末端等于 occupied right。

### 性能证据与审阅

新增 `examples/white-space-bench.rs`，复用真实字体；System allocator 记录申请次数、累计申请
bytes（含 realloc），不代表峰值存活内存。20 次预热、100 次采样，每次独立计 layout；输入 clone、
引擎构造与结果析构在计时外。cold 每次重建引擎以测 annotation miss，hot 使用同一引擎。
两份构建使用相同 runner；基线仅移除旧 API 没有的 WhiteSpaceMode 配置。

HEAD `1178786` 经只读 `git archive` 解压到 `target/phase5-baseline`，各自使用默认 target；
未修改当前源码或 Git index，不使用指定 target-dir。release 同窗口按 baseline/current/baseline/current
顺序运行。普通正文为 1152 scalar 无空白中文，两边均 28 行、1152 clusters，输入与行为可比。

| 场景 | baseline min ms，两轮 | current min ms，两轮 | 申请次数 baseline→current | 累计申请 bytes baseline→current |
| --- | --- | --- | --- | --- |
| 普通正文 hot | 1.8317 / 1.7516 | 1.8308 / 1.7571 | 24857→24906 | 5427534→5568618 |
| 普通正文 cold | 21.5810 / 21.4920 | 21.3946 / 21.2790 | 158562→158611 | 144499977→144641101 |
| Tab 密集 hot，640 scalar | 1.0038 / 1.0693 | 4.7532 / 4.8395 | 16911→46069 | 3016164→21334386 |
| Tab 密集 cold | 11.5179 / 11.2557 | 11.5437 / 11.7561 | 96338→103494 | 78553903→74163409 |
| 长空白 hot，2050 scalar | 2.8723 / 2.9563 | 3.2627 / 3.1445 | 43646→44036 | 9891819→10541240 |
| 长空白 cold | 42.6566 / 43.0156 | 42.2663 / 41.3938 | 431262→429384 | 215039375→215753271 |

普通正文 hot min 差异 −0.05% / +0.31%，新增 49 次申请与约 141KB；未观察到普通正文运行时间
退化。Tab 的新 stop、逐空白与 justification 数值求解改变了输出（15→22 行），不要求旧行为同速。
current Tab 640/1280/2560 scalar hot min=4.7532/9.3157/19.3279ms，申请 46069/91737/183136，
规模增长接近线性。长空白 2050/4098/8194 scalar current hot=3.1445/7.4509/19.9836ms，
cold=41.3938/85.0217/170.7675ms，申请量近线性；补测 baseline 4098/8194 hot=7.4003/17.7278ms，
cold=85.8503/171.1837ms，旧路径也存在类似热耗时增长，不能只从当前数值断定新增二次扫描。

代码审阅确认新增 source scan、无 Tab 候选前缀计宽、最终空白记录、无注解行几何均为线性，Tab
只处理候选内 Tab，最终求解次数有固定上限。缓存命中复制 Arc；已有 cache key、布局输出和 backend
仍有线性复制。未发现本轮新增不必要的全段重复扫描；既有 repair 压缩机会筛选、debug 与复杂注解
成本维持本轮范围，未借 Phase5 顺手重构。上述性能是完整 debug 输出及真实 backend 的 layout 成本。

### 最终验收结果

- `cargo check --all-targets` 退出码 0；`cargo test -- --quiet` 退出码 0：库 24、独立 golden 1、
   Phase5 13、Phase4 17、Phase3 20、主集成 1360 全通过，doc test 0 项。随后
   `cargo test --test tiqian layout_fixture_golden_test` 非更新模式再次通过。
- 编辑器仅报告原有 tests/support unused 警告；新增测试、性能程序与浏览器探针无错误。
   `git diff --check` 退出码 0。中文注释审阅包含未跟踪新文件，新增及改写注释均为中文。
- 五份文档风格检查脚本已执行，退出码 1，43 处措辞候选逐项复核。字体、几何、有效单空格数量、
   source 契约、旧阶段检查记录等均为具体技术语义；已纠正开发指南中 fixture/backend 和构建 profile
   的两处事实表述，并把新增泛化对比改成指标说明。最终复检为 42 处候选，逐项人工复核后保留；
   脚本词表与历史迭代不变。
- 仅本仓库执行本地操作；既有暂存与未暂存改动保留，未运行格式化、指定 target-dir、Git 写操作、
   发布或安装环境，上游同步终点未推进。隔离基线源码与构建产物保留在 target 下供复核。

1. 一次性完成调用点和测试构造迁移，检查编辑器诊断并恢复全目标编译；删除被替换的旧分支和临时
   调试代码。先执行定向测试，再执行全量测试。
2. 按下表补齐回归；将既有复现测试改为新行为验收。auto-space 的“保留多空格”测试显式采用保留
   空白模式，默认正文测试单独验证折叠，避免把两个功能混为一个预期。
3. 新增必要的本地 fixture 并同步 fixture/golden 清单；新行为稳定后统一更新 golden，逐项解释 diff，
   复跑非更新模式确认稳定。
4. 扩展现有浏览器探针，对照四种模式、软／硬换行、Tab、行尾悬挂和对齐，记录 UA、字体、宽度与
   测量结果。Rust 使用真实字体验证两条编号原文与尾随空格光标几何；浏览器不支持的 replace 仍按项目假设验收。
5. 进行性能检查和最终 review，更新开发指南、关键差异及两份 ADR 的实施状态和验证证据；只在全部
   最终条件满足后将本迭代标记完成。历史迭代文档不变，上游同步终点不推进。

## 最终验证矩阵

| 类别 | 必须覆盖 |
| --- | --- |
| 空白与间距组合 | 四种空白模式 × 三种 auto-space 模式；双向 W/N、字母／数字、0–3 个 U+0020、跨 style/source 分片；两个编号原文 |
| 换行与字符 | 两种换行控制；LF、CRLF、独立 CR 及既有 mandatory break；Tab stop；NBSP、全角空格；纯空白、超窄、段末与末尾硬换行 |
| 位置与宽度 | 行中／首／尾、软换行与硬换行；悬挂的 fit／对齐／占位差异；光标、选区、命中、溢出、max-lines 及 replay |
| 既有排版 | 标点 glue、自动 gap 行边处理、kinsoku、repair、hyphenation、progressive technical、ruby、inline box/object、装饰与原文复制 |
| 证据 | deterministic 测试、fixture dump/golden、浏览器对照、Rust 真实字体及性能结果；证据不得把未支持的 CSS fallback 当成实测 |

最终执行仓库已有的 `cargo check --all-targets`、`cargo test` 和
`cargo test --test tiqian layout_fixture_golden_test`，以及文档检查与 `git diff --check`。
Golden 更新方式沿用 [开发指南](../dev-guide.md)，更新与非更新验证分开记录。

性能比较使用相同配置和未受行为变化影响的普通正文，按开发指南的同窗口交替测量方式检查新增扫描与
分配开销。Tab 密集和长空白文本另作针对性检查；检查目标是避免新增不必要的全行重扫或高阶复杂度，
不要求新旧行为不同的文本得到相同行数或运行时间。

## 兼容性与回滚

当前处于开发阶段，公开输入与宽度模型允许直接调整，不保留旧的空格归一模式或隐藏兼容开关。
默认行为和 golden 的变化在本迭代统一记录，下游 Rust 调用点随新 API 迁移。

若实施中需要暂停，保留本文的当前 phase、已改文件、已知编译／测试断点和下一步，ADR 继续标记待实施。
需要撤回时以本联合迭代的代码、测试及 golden 为完整范围，保留其他任务的改动；版本控制操作由用户负责。

## 执行记录

- 2026-10-22：创建联合迭代计划；随后开始 Phase 1，加入公共空白、换行、Tab 配置与空白模式缓存键。
   R0007 与 R0008 仍为 Accepted / Pending，实际布局行为尚未变更。
- 前期证据：auto-space 定向测试 17 项通过，行几何与行调整测试 41 项通过，均固定当前旧行为，
  不代表新行为已完成验证。
- 本轮验证：`cargo check --all-targets` 通过；修改的 Rust 文件无编辑器诊断，既有 unused 警告未处理。
- 已确认方案 B：`Preserve / BreakSpaces` 保留对象与标点之间的作者空格，停止跨非空空白串的特殊附着；
   `Collapse / PreserveBreaks` 保留既有附着策略，对象紧邻标点的行为不变，普通标点禁则继续生效。
   此项安排在 Phase 2，与有效空白序列统一实施。
- 阶段纠正：曾提前加入附着记录过滤，已撤回；实际附着行为未变，未新增对应行为测试。
   随后继续完成 Phase 1 的内部记录类型与几何接口约定。
- Phase 1 验收：公共 API、字符规则、Tab stop、缓存责任、源映射、断行资格及宽度接口均已明确；
  `white_space.rs` 纳入编译，`cargo check --all-targets` 通过，编辑器无新增错误，文档措辞候选已人工复核。
  本阶段未实施空白算法，未更新测试预期或 golden。
- Phase 2 验证：`cargo test --test tiqian phase_two_` 6 项通过，`cargo test --lib` 24 项通过，
   `cargo check --all-targets` 通过；仅有既有 unused 警告。直接调用 shaping 的 6 处测试构造已补齐参数，
   未修改旧行为断言、fixture 或 golden。
- 相邻通道回归：`inline_box_layout_test` 2 项通过；`verbatim_range_auto_space_test` 1 项通过、1 项失败。
   失败位于 `authored_space_inside_verbatim_range_is_not_normalized` 的 control，仍要求默认 `Insert`
   对显式空格生成 Replace decision，属于 Phase 5 待迁移旧预期。新增定向测试使用显式 `Replace`
   验证 suppressed range，已通过。修改文件无新增编辑器诊断，`git diff --check` 通过。
- 规范纠正：重新阅读 `AGENTS.md`，将 Phase 1–2 新增及改写的英文注释全部改为中文，
   包括此前被改成英文的自动间距说明。自动间距改用单调迭代器查找有效邻居，移除两张逐 cluster
   邻接表的分配，保持 O(n)。定向测试 6 项、库测试 24 项复跑通过，编辑器无新增错误。
   本轮未推进 Phase 3，未运行格式化工具，未修改历史迭代文档或旧行为断言。
- Phase 3 验收：20 项定向测试在 debug／release 通过；完成 progressive 对照、条件悬挂采样、
   压缩 gap 修复及反证、共享路径与分配审阅。完整 tiqian 重跑 1345 通过、15 失败，逐项归因见上表。
   Phase3 已完成，Phase4 未开始；逐行几何与交互继续按 Phase4 接入，旧断言与 golden 留 Phase5。
- Phase 4 验收：最终几何、对齐与占位 API、Tab 加 justification、交互和 replay、结构化空白
   诊断及 fixture dump 已接通。17 项定向测试在 debug／release 通过，Phase3 20 项、Phase2
   6 项、库 24 项及全目标检查通过；完整旧预期与 golden 的失败保留 Phase5。当前迭代仍实施中，
   本轮止于 Phase4，未修改 ADR 状态、旧测试断言或 golden，未运行格式化或 Git 写操作。