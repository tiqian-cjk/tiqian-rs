# 2026-09-18 运行时布局局部回退

> 状态：进行中
>
> 依据：`AGENTS.md` 的“取舍”章节与
> [R0005 运行时布局的局部回退](../adr/R0005-runtime-layout-local-fallback.md)。

## 目标

移除运行时布局路径中为确认数据正确性而执行的 validate、assert、expect 与 panic。
异常输入、异常 backend 输出或异常策略数据只在当前操作点触发局部回退，后续 cluster、行、段落和
下一次 layout 继续处理。回退分支可以输出固定文本 `warn`，但不得为日志额外扫描、排序、clone、收集
或格式化复杂上下文。

本迭代的主目标包含公开运行时行为变化，因此分类为 `feat`；性能收益来自删除完整输入验证、
冗余中止路径和不必要分配。

## 审计基线与判定

审计范围为 `src/**/*.rs` 的非测试代码，检索 `assert!`、`assert_eq!`、`assert_ne!`、`expect` 和
`panic!`；排除 `#[cfg(test)]` 模块后的初始静态清单包含 106 个中止点。该清单用于定位问题，不代表
运行时调用次数；是否属于热路径按每次 layout、每个 scalar 或每个 cluster 的常规处理判断。

本迭代区分完整 validate 与操作点条件判断：前者遍历或检查输入以确认完整约束，再决定是否处理；后者
只在读取 slice、计算几何或推进循环时处理当前值，使本次操作有定义。运行时仅保留后者。测试辅助对象
不属于生产容错边界；没有生产调用点的 helper 应保留在 `tests/`。

## 已确认决策

1. 删除 `validate_layout_input` 的完整输入扫描、clone 和排序；range、inline object 与数值只在消费点
   有界访问、截断、默认值回退或跳过。
2. 构造器和 `ParagraphBuilder` setter 不因数据问题中止；对象保留原始值，消费点退化，配置更新可保持
   原配置或忽略本次更新。
3. backend、shaper 与 line breaker 的异常输出按 cluster/source 区间隔离；使用局部 fallback，不返回
   整段错误。
4. 并行 slice 不做完整关系检查；缺失字段采用字段默认语义，额外辅助数据忽略。
5. 查询与 renderer 参数异常返回局部空结果或跳过 layer；生产 shaper 路径不包含测试专用的未实现对象。
6. Unicode 分类器统一接收 `char`，删除每次分类前的 scalar assert。
7. 通过控制流绑定、直接索引或默认分支消除局部已证明关系的运行时 `expect`；复杂内部关系只允许
   debug 专用检查。
8. 已进入局部回退分支时记录固定文本 `warn`；正常分支不产生日志 I/O、格式化或额外分配。
9. 运行时 `warn` 使用 `log` facade；调用方负责安装 logger，布局 crate 不维护日志回调或限频状态。

## 范围

### 包含

- `src/` 中段落 layout、输入消费、字体/shaping、断行、标点、查询和 Unicode 分类器的运行时中止路径；
- 对应 unit test、fixture 与 golden 的异常输入/局部回退验证；
- 本迭代文档、R0005 与需要同步的持续维护文档。

### 不包含

- 改变正常输入的布局规则、字体选择、断行和 golden 几何；
- 为日志采集输入全量上下文、排序诊断数据或维护布局路径内的限频状态；
- 一次性重构无关模块、格式化无关文件或处理既有 warning。

## 实施阶段

### Phase 1：删除 `LayoutInput` 全量验证

1. 找到 span、inline object、paragraph style 数值的实际消费点；
2. 将每个消费点改为有界访问、局部截断、默认值或跳过，并在实际回退分支输出固定文本 `warn`；
3. 删除 `validate_layout_input`、其调用和为该函数存在的辅助逻辑；
4. 将 panic 测试迁移为局部退化、后续 source range 继续布局和下一次 layout 可执行的测试；
5. 运行定向测试、fixture/golden 与 benchmark，记录性能和布局结果。

已完成（2026-09-18）：

- 删除 `validate_layout_input`，包括递归 retry 时重复执行的输入扫描、inline object clone 与排序；
- 使用 `log` facade 在实际回退分支输出固定 `warn`；
- inline object 按 source start 消费，同 start 时保留首个输入；后续落在已消费范围内的对象不重复进入
   cluster；坏 range 回退为 source text，坏几何回退为零，前导边界回退为 fixed，尾随 shrink/discard
   截断到当前 advance；
- annotation 准备阶段只建立既有的 source-start 查询索引；range 索引只为 resolver 已实际消费的对象
   建立，几何和 boundary 不在输入遍历中集中检查或规范化；
- 非有限 inline box edge 回退为零；无效 emphasis gap 和 inline object clearance 回退为各自默认值；
- prepared JSON 不导出非有限 inline edge 或未生效 object 的原始几何；
- `cargo test --test tiqian`：1368 passed；`layout_fixture_golden_test`：1 passed；
- release benchmark（Windows x86_64，`--iterations 1000 --warmup 50 --widths 672,360,960 --scale 1`）的
   layout 中位数为 2.909 ms/页，$p95$ 为 4.100 ms/页。没有本次修改前的同机基线，不计算加速比例。

### 后续阶段

1. 构造器和 `ParagraphBuilder`；
2. backend、shaper、line breaker 与并行 slice；
3. 查询与 renderer；
4. Unicode 分类器与局部已证明的 `expect`；
5. 全量验证、文档同步和完成记录。

Phase 2 进展（2026-09-19）：

- `ParagraphLayoutEngineBuilder` 没有运行时中止路径；
- `ParagraphBuilder` 在 source text 写入后更新 `text_style`、`paints`、`paragraph_style` 或
   `profile_id` 时保留原配置、忽略本次更新并输出固定 `warn`，不再 panic；
- 回归测试覆盖四项配置都保持原值，以及后续文本继续写入和 `build()` 成功。
- `ScalarOffset` 将负值规范化为零，`TextRange` 将反向端点排序，`LayoutConstraints` 将非正宽高和
   行数规范化为零；每次规范化输出固定 `warn`，避免异常坐标或约束进入 slice、索引和布局管线后中止；
- 回归测试覆盖 geometry 规范化，以及零约束段落和后续独立段落均可继续 layout。
- `FontFaceId` 保留空资源标识，使 catalog 查询按未解析 face 处理；variation instance 删除空 tag、非有限
   value 和重复 tag 的后续设置，并保留首个有效 axis、按 tag 排序后作为稳定 face identity；每次删除输出固定
   `warn`。
- 行内对象 boundary adjustment 构造器保留原始数据；preferred stretch 构造器将非有限或负 natural 规范化
   为零，并将非有限或不递增 target 规范化为 natural，使容量为零并输出固定 `warn`。shrink 与 line-end
   discard 继续在当前 cluster advance 的消费点截断。
- `Glue` 构造器将非有限 natural 规范化为零，再将非有限或越过 natural 的 min/max 收拢到 natural
   advance，并输出固定 `warn`，使布局继续使用已有的自然标点间距。

Phase 3 进展（2026-09-19）：

- `FontBackendShapingResult` 与 `FontResolution` 的 selected attempt 改为可选值；空、缺失或顺序不一致的
   candidate evidence 输出固定 `warn`，debug 组装跳过对应 font decision，段落其余结果继续生成；
- cluster role 覆盖关系遇到跨越、间隙、不连续或重叠范围时输出固定 `warn` 并停止处理当前异常区间，不再中止
   段落；
- 同一 shaped cluster 的 OpenType feature 冲突保留首个 feature，缺失 font resolution evidence 仅记录固定
   `warn`；pinyin ruby 缺少测量 geometry 时跳过该 span 的 spread；
- glyph run 缺少直接 face 时先使用覆盖 group 的 resolution face，仍缺失时跳过该 run；ruby geometry 缺失时
   跳过对应 ruby decision；
- Greedy、Lookahead 与 paragraph DP breaker 在 natural/adjusted cluster 数量不一致时使用 adjusted 共同前缀，
   natural 尾部保留 natural advance；Lookahead 的负窗口与 horizon、DP 的负 candidate window 均规范化为零并输出
   固定 `warn`。DP candidate window 只在入口规范化一次，候选枚举不再重复执行该分支；
- 回归覆盖异常 candidate evidence、feature 冲突、异常 coverage、缺失 cluster 对齐和负断行窗口；`cargo test
   --test tiqian`：1367 passed；`layout_fixture_golden_test`：1 passed；`cargo test --all-targets` 通过，包含
   paragraph demo 的 20 项测试；`git diff --check` 通过。

Phase 4 进展（2026-09-19）：

- `Justifier`、标点 geometry stage、punctuation geometry ledger 与 attached inline Unicode boundary resolver 删除
   并行 slice 的完整长度断言；
- 缺失 cluster role 使用 `Unknown`，缺失 East Asian spacing edge 使用 `Other/Other/false` 的中性 edge；因此
   不生成依赖该辅助信息的 CJK、标点或自动间距边界；
- 缺失 attachment 使用 `None`，只读取能够覆盖 cluster 的 attachment 前缀；额外 attachment 不参与 virtual
   boundary；缺失 line-break cluster 时 attached mark kinsoku 使用同索引 natural cluster 的 advance；
- 回归覆盖多余与缺失 role、edge、attachment、line-break cluster，以及 attached run 两端缺少 role/edge 的
   neutral fallback；`cargo test --test tiqian`：1368 passed；`layout_fixture_golden_test`：1 passed；`cargo test
   --all-targets` 通过，包含 paragraph demo 的 20 项测试；`git diff --check` 通过。

Phase 5 进展（2026-09-19）：

- `positioned_clusters_for_line_box` 收到不属于当前 `LayoutResult` 的 `LineBox` 时输出固定 `warn` 并返回空
   cluster 列表；同一结果中的后续所属 line 查询不受影响；
- rich-text 背景圆角查询遇到非背景 layer 时返回全零方角，非有限或负 inset 回退为零；装饰线查询遇到非装饰
   layer 时返回零坐标，非有限或负 stroke width 回退为零；
- dot/dash pattern 的坐标、尺寸或间距无效时输出固定 `warn` 并跳过当前 pattern，不影响后续 pattern 计算；
- 回归覆盖 foreign line、错误 layer、异常 inset/stroke 与异常 pattern 参数，并确认同一调用序列中的正常查询或
   pattern 保持有效；`cargo test --test tiqian`：1369 passed；`layout_fixture_golden_test`：1 passed；`cargo test
   --all-targets` 通过，包含 paragraph demo 的 20 项测试；`git diff --check` 通过。

Phase 6 进展（2026-09-19）：

- East Asian spacing、script evidence、word character 和标点 line-break 四个 Unicode 分类器的公开输入统一为
   Rust `char`，由类型系统保证传入值是 Unicode scalar，删除分类器内部每次调用的整数范围与 surrogate
   断言；
- `Text` 增加零分配的 `char_at_or_none` 与 `char_before`；分类器消费者直接读取 `char`，其余仍使用整数码点的
   规则保持原接口和行为；
- 分类回归测试改用有效 scalar，保留 BMP、补充平面、组合标记、Common、Inherited 与各类 East Asian script 的
   覆盖；不再把无法构造的非标量整数作为运行时 panic 行为测试；
- `cargo test --test tiqian`：1368 passed；`layout_fixture_golden_test`：1 passed；`cargo test --all-targets` 通过，
   包含 paragraph demo 的 20 项测试；`git diff --check` 通过。

Phase 7 进展（2026-09-19）：

- 清理 scope 收尾、注音解析、数字与货币连续区间、引号配对、固定标点候选、Greedy/Lookahead/DP 断行、
   emergency tracking 对齐、TeX 连字符模式解析与 prepared JSON 序列化中的 29 处局部 `expect`；
- 相邻 `peek`、`last`、过滤条件或固定三项候选已能证明正常路径时，改为同一控制流内的 `Option` 绑定、首项 fold
   或局部值复用；不再因证明关系在未来调用链中失效而终止运行时 layout；
- DP promotion 的内部映射意外缺失时保留原候选池继续断行；段尾空行仅在仍能读取最后一个 cluster 时生成；数字
   JSON 和连字符等级只处理已可读取的 ASCII 字符，异常关系退化为当前局部结果；
- 既有定向测试覆盖 scope、CLREQ、quote、标点、三种断行器、DP promotion、连字符模式与 prepared JSON 的正常、
   段尾和边界路径，无需为等价的局部绑定新增仅验证语法的测试；`cargo test --test tiqian`：1368 passed；
   `layout_fixture_golden_test`：1 passed；`cargo test --all-targets` 通过，包含 paragraph demo 的 20 项测试和
   paragraph-layout bench 的 9 项测试。

Phase 8 结论（2026-09-19）：

- `UnimplementedTextShaper` 没有生产调用点，仅用于 `tests/shaping`；已移至测试文件，保持测试辅助对象的
   明确未实现语义；
- 不为不存在的生产 target 路径新增逐字符 shaping fallback，也不改变正式 `TextShaper` 的 API 或布局行为。
- `cargo test --test tiqian "shaping::explainable_stub_text_shaper_test"`：8 passed；
   `cargo test --test tiqian`：1368 passed；`git diff --check` 通过。

每个阶段完成后先汇报验证结果；遇到无法从当前调用链确定的局部回退语义时，停止并请求决策。

## 验证与回滚

每个修改点至少验证：

- 正常输入的相关 fixture/golden 无差异；
- 异常输入只影响当前 span、cluster、line 或绘制 layer；
- 同一段后续 source range 仍可生成布局；
- 后续独立 layout 仍可执行；
- 运行时不执行为数据正确性而新增的全量扫描、排序或 clone；
- 回退日志为固定文本，且只出现在实际回退分支。

若局部回退改变了无关正常输入的 geometry、无法保证后续处理继续，或需要新增全量 validate，撤回当前
子修改并重新确认该消费点的退化语义。
