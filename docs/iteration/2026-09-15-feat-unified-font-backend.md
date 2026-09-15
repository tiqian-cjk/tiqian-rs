# 2026-09-15 统一字体后端与选择 shaping 一体化

> 状态：已完成
>
> 上游依据：Kotlin 上游同样使用独立的 fallback、metrics 与 shaping 接口。本迭代为 Rust 的有意
> 架构调整；实现前新增 Rust ADR，并更新 `docs/key-differences.md`。

## 目标

将当前分开的字体 fallback、字体度量和文本 shaping 接入收敛为一个一致的字体后端 contract。三者本来
都属于字体后端能力；本迭代改变的是它们之间的调用边界，使一次完整 shaping 同时产生可复用的最终
字体 identity 和候选证据。对于每个由 tiqian-rs 传给 backend 的 shaping range，后端按候选顺序执行完整 shaping；第一个不含 `.notdef` 的
结果确定最终 `FontFaceId`。最终 face、glyph、cluster、advance、缺字证据和绘制重放键必须来自同一次
候选尝试。

本迭代解决以下已确认的问题：

- `FallbackResolver` 只能在 shaping 前根据文本 range 推测一个 `FontDecision`；后端若要按覆盖情况选择
  face，必须独立执行 cmap 查询或其他字符映射；
- `TextShaper` 随后仍要执行 glyph mapping 和 OpenType shaping，前一步的结果不能复用；
- cmap 覆盖不能证明 GSUB 替代、variation selector 或 emoji ZWJ 序列 shaping 后不会产生 `.notdef`；
- `FontMetricsResolver` 独立接收字体族和 `face_selection_text`，当前类型无法保证它测量的 face 与
  shaping、glyph replay 的 face 一致；
- `ReplayableFontCatalog` 已定义 `FontFaceId` 和受控字体目录，但没有进入 `ParagraphLayoutEngine` 的
  实际调用链。

完成后，受控字体后端不会为了 fallback 先执行一次不可复用的 glyph coverage 预检查。首选 face 缺字时，
后端完整 shape 后续候选以获得实际 fallback 结果。layout、metrics、glyph replay 使用同一个最终 face
identity。

## 范围与边界

### 包含

- 简体中文横排中的 CJK、Latin 与 emoji；emoji 始终以完整 grapheme 交给一个受控 face shaping，并重放
  该次 shaping 返回的全部 glyph；
- 重新定义主字体后端 contract，使其一次接收文本、source range、display text、文本样式、font role、
  OpenType feature 与按优先级排列的候选 face；
- 后端在每个候选 face 上执行完整 shaping，返回最终单 face 结果、每次尝试的缺字信息和结构化选择理由；
- 让 metrics 请求引用已确认的 `FontFaceId` 和字体实例，禁止再次根据 `font_families` 或选择文本决定 face；
- 将 `ReplayableFontCatalog` 纳入正式调用链，使 `FontFaceId` 成为 fallback、metrics、shaping 与
  `Glyph.render_font_key` 的唯一受控字体键；
- 调整 paragraph shaping、line metrics、ruby 与 bopomofo 重放等调用点，并补充 HarfRust 字体后端的回归测试；
- 将现有宽度无关注释缓存中的旧字体决定替换为最终 `FontResolution`，使缓存结果保留实际选定的 face。

实现收敛后，metrics 只保留一个 `FontMetricsRequest` 类型。该请求直接携带完整 shaping 已选定的
`FontFaceId`、字号、角色和 locale；不再同时保留一个字体选择前请求与一个 resolved 请求，也不再保留
独立的 `FontMetricsResolver` 或仅用于旧路径的 metrics builder。

### 不包含

- 修改 CLREQ font role 分类、range 切分、标点 display substitution、断行、行调整或两端对齐规则；
- 把系统字体枚举、字体文件加载、UI 框架或 renderer 实现放入 tiqian-rs 核心；
- 为每个候选字体预先缓存完整 shaping 结果，或在本迭代定义跨段落 shaping cache；
- 在 Huozi 或其他调用方中实现临时的 cmap 预检查以绕过此接口缺陷；
- 将一次 backend shaping 请求拆成多个 face run，或实现跨 face 的组合标记、复杂脚本和平台字符串绘制；
- 支持 RTL 或婆罗米系文字；
- 改变 Unicode scalar source coordinate、source/display 分离、复制、选择或无障碍语义。

## 当前问题

当前主路径依次调用三个独立 trait：

```text
role 与 range
  -> FallbackResolver::resolve(text, range, FontRequest)
  -> FontDecision
  -> TextShaper::shape(ShapingInput { font_decision, display_text, ... })
  -> ShapingResult

FontDecision + 样式 + display text
  -> FontMetricsResolver::resolve(FontMetricsRequest)
  -> RawFontMetrics
```

`FallbackResolver` 的输出只包含 `FontCandidate`，无法复用为 shaping 产物；`TextShaper` 没有返回
让 metrics 直接解析的强类型 face instance。虽然 `FontMetricsRequest.font_key` 存在，当前请求仍带有
`font_families` 与 `face_selection_text`，实际实现可以据此重新选择不同的 face。现有 `DemoFontCatalog`
正分别用 fallback、metrics 和 shaping 的独立方法选 face，说明这个风险已存在于示例接入路径中。

问题不应通过“更轻量的 coverage 检查”解决。nominal cmap mapping 仍与 HarfRust shaping 的 mapping 重复，
并且不能作为复杂字体行为的正确性证据。

## 设计

### 后端职责

tiqian-rs 继续拥有所有排版决策：

1. 根据 source text、样式边界、annotation、font role 和既有 emoji shaping 原子性规则确定 shaping range；
2. 构造后端请求，提供 source text、display text、范围、样式、role、locale 与 OpenType feature；
3. 消费后端返回的最终单 face cluster、glyph、最终 face、metrics 与尝试证据，继续执行既有布局流程。

字体后端拥有字体资源决策和实际 shaping：

1. 按文本、样式、role、locale 和受控目录在后端内部确定候选顺序并尝试 face；
2. 对每个候选执行一次完整 shaping，并以 glyph id `0` 作为该候选的缺字证据；
3. 第一个无 `.notdef` 的候选成为最终结果；后续候选不再执行 shaping；
4. 所有候选都有 `.notdef` 时，保留首选 face 的 shaping 结果，记录每个候选的缺字数量。

后端不得把未确认的 face 当作最终结果，也不得要求调用方预先读取 cmap 作为常规 fallback 判断。

### 数据流

目标调用链如下：

```text
LayoutInput
  -> role 与 shaping range
  -> FontBackend::shape(request, catalog)
  -> resolved FontFaceId + GlyphRun + Cluster + candidate evidence
  -> FontBackend::metrics(resolved FontFaceId, instance, size)
  -> line planning / adjustment / LayoutResult
  -> renderer 按 FontFaceId 重放 glyph
```

`FontFaceId` 必须标识 physical SFNT face、collection index 与 variation instance。字号属于请求样式，不属于
face identity。metrics 由最终 face 的同一字体实例读取；它不再接收用于选择 face 的文本或字体族列表。

`FontFaceId` 移入 `core` 下的中立模块，避免 `core::layout_model` 依赖 `shaping` 而形成循环。可绘制的
`Cluster`、`GlyphRun`、`Glyph.render_font_key` 和 metrics debug 使用该强类型 identity。强制换行、零宽软断行
和行内对象等合成 cluster 显式表示“无字体”，不再把现有字符串哨兵值伪装成字体 identity。

本迭代采用“每个 backend shaping 请求独立选择字体”的粒度。段落 shaping 为断行、标点替换或 technical
retry 产生的每个请求都调用一次 `FontBackend::shape`；该请求的 range 同时是候选选择、完整 shaping 和
`FontResolution` 的原子范围。因此同一个旧的 `ResolvedClusterRange` 被按空格、断词、标点或紧急断行切分后，
会产生多个独立的字体 resolution；这些 range 变化属于统一 backend contract 的可观察结果，不表示 CLREQ
或断行切分规则发生了变化。

正文的 `FontResolution` 只记录 paragraph shaping 使用的请求。ruby 和连字符等独立的合成 shaping 请求
直接消费其返回的 face 与 glyph，不加入正文 resolution 集合。

### 缺字与候选尝试

当前目标范围没有证据要求一次 shaping request 内使用多个 face。目标语料中的 Latin 组合文本如 `é` 由
一个 Latin face 完整 shaping；emoji 的非 RGI ZWJ 序列即使没有单独组合 glyph，也可由一个 emoji face 返回
多个有效 glyph 并原样重放。因此本迭代保持一个 backend shaping request 对应一个最终 `FontFaceId`；不同
request 可以独立选择不同 face。

`.notdef` 是候选尝试的 shaping 证据。首选 face 缺字时，后端继续尝试后续候选；全部候选缺字时，最终输出首选
face 的 `.notdef` glyph，并在结构化 debug 中记录候选顺序、每次尝试的 `FontFaceId` 与缺字数量。该结果是
受控缺字，不视为成功 fallback。

### API 迁移原则

具体 trait、struct 与方法名称在实现设计阶段确定；本草案不预先锁定名称。迁移必须满足：

- 普通 `ParagraphLayoutEngine` 只注入一个主字体后端；fallback、metrics 与 shaper 由该后端统一提供；
- 如保留细粒度 trait，仅作为主后端内部组合或测试实现，不再构成普通平台接入路径；
- `ReplayableFontCatalog` 要么被主后端直接持有，要么与主后端组合为不可分离的构造参数；
- stub 后端继续支持 deterministic fixture，并返回与正式路径同型的单 face、metrics 和缺字证据；
- 公开 API 发生不兼容调整时，更新 API 文档与示例；实现前新增 Rust ADR 记录本项有意架构差异。

## 实施 phase

本迭代按依赖关系推进。Phase 之间不要求程序始终处于可编译或可运行状态；允许先替换类型和调用链，
最后集中完成适配、测试与恢复。每个 phase 的“完成条件”用于确认工作边界，不要求在中间阶段运行完整测试。

### Phase 0：冻结边界与记录取舍

目标：把本迭代的范围固定为可验证的单 face 字体后端，避免实施过程中重新引入没有证据支持的多 face 模型。

工作内容：

1. 新增 Rust ADR，记录 Kotlin 上游仍采用三个独立 trait、Rust 改用统一后端的原因；
2. 在 ADR 中记录候选字体按优先级完整 shaping、首个无 `.notdef` 结果胜出、全候选失败时保留首选结果的规则；
3. 记录简体中文横排的 CJK、Latin、emoji 范围，以及 RTL、婆罗米系文字和跨 face 分段不在本迭代中；
4. 更新 `docs/key-differences.md`，链接 ADR 和本迭代文档。

完成条件：设计边界、缺字语义和回滚边界已有稳定文档记录。此 phase 不修改 Rust 实现。

### Phase 1：建立中立字体 identity 与后端数据模型

目标：先建立不会形成模块循环的基础类型，并确定新后端在布局层交换的数据。

工作内容：

1. 在 `core` 的中立模块定义 `FontFaceId`，标识 physical face、collection index 与 variation instance；
2. 将 `Cluster`、`GlyphRun`、`Glyph`、metrics decision 和 debug 中的字体 key 分为受控 face identity 与无字体
  synthetic cluster 两种语义；
3. 定义统一后端 request/result 的最小字段：source range、source/display text、style、role、locale、
  OpenType features、候选顺序、最终单 face result、候选尝试 evidence 和缺字 evidence；
4. 让 `ReplayableFontCatalog` 使用该中立 `FontFaceId`，保留 catalog 在 shaping/backend 层的归属；
5. 删除模型中把 `mandatory-break`、`inline-object` 等字符串哨兵当作字体 key 的隐含约定。

完成条件：核心数据模型能表达最终单 face 与无字体 synthetic cluster，模块依赖方向清晰。此 phase 允许
旧的 layout、shaping 和测试暂时无法编译，不在本 phase 添加兼容字段或双写路径。

### Phase 2：实现统一字体后端 contract

目标：把候选选择与实际 shaping 放进同一个后端边界，消除独立 fallback 预检。

工作内容：

1. 定义主字体后端 trait 及其 request/result 类型，具体名称以 Phase 1 的模型为准；
2. 后端按候选顺序逐个完整 shaping，不执行独立 cmap coverage 预检查；
3. 第一个无 `.notdef` 的候选成为最终结果，并停止后续尝试；
4. 全部候选均有 `.notdef` 时保留首选候选的 glyph、advance、bounds 和 face identity，记录所有尝试的缺字数量；
5. 依据最终 face 提供 metrics，metrics 请求只接受 `FontFaceId`、字体实例和字号/样式所需参数，不再自行选 face；
6. 将 `ReplayableFontCatalog` 纳入统一后端，保证最终 face 可供 shaping、metrics 和 glyph replay 共同解析。

完成条件：统一后端的请求、结果、候选优先级、缺字和最终 face 语义已确定；此 phase 可暂不接回 paragraph
layout，允许 crate 暂时无法编译。

### Phase 3：迁移核心 layout pipeline

目标：让 paragraph layout 消费统一后端结果，去除普通布局中的三 trait 分别注入。

工作内容：

1. 修改 `ParagraphLayoutEngine` 和默认引擎的依赖组装，只注入一个主字体后端；
2. 修改 width-independent annotation preparation，直接保存统一后端产生的最终 face、shaping result 和
  候选 evidence；
3. 修改 paragraph shaping、line break planning、line adjustment 与最终 glyph run 构造；
4. 修改 ruby 与 bopomofo 的字体请求，使其使用同一后端和最终 face 语义；
5. 保留已有 CLREQ role、display substitution、line breaking、adjustment 和 source coordinate 逻辑，不在本
  phase 借机重构这些算法；
6. 删除普通布局路径对 `FallbackResolver`、`FontMetricsResolver`、`TextShaper` 的重复 face 选择调用。

完成条件：核心 layout 的调用方向已切换到统一后端。此 phase 可暂时存在编译错误，重点是完成调用链迁移，
不通过保留第二套运行路径来维持中间状态。

### Phase 4：绑定 annotation cache 并迁移实现后端

目标：恢复可用的字体后端、缓存和示例实现。

工作内容：

1. 迁移 deterministic stub，使 fixture 继续获得稳定的最终 face、metrics 和缺字 evidence；
2. 迁移桌面 HarfRust/SkRifa 示例，将字体 bytes 和别名交给一个 catalog/backend；
3. 迁移现有测试实现，删除只为旧 trait 注入而存在的适配代码；
4. 修复所有编译错误，恢复 `cargo check`、相关示例和核心测试可执行；

完成条件：Rust crate 恢复可编译；stub、桌面 HarfRust/SkRifa 示例和 annotation cache 均已接入统一后端。此 phase 结束
前不更新 golden，避免把中间状态写入稳定验证资产。

### Phase 5：回归验证与文档完成

目标：证明本迭代只改变字体后端边界，不改变既有排版规则和交互语义。

工作内容：

1. 新增候选尝试次数、首选缺字后的完整 fallback、全部候选缺字和最终 face 一致性的定向测试；
2. 使用现有目标语料验证中西混排、拉丁组合文本 `é`、emoji modifier、ZWJ、旗帜和 keycap；不新增没有语料
  依据的汉字组合标记案例；
3. 验证 glyph replay 使用最终 `FontFaceId`，metrics 与 glyph advance/bounds 来自同一字体实例；
4. 验证 annotation cache、technical retry、display substitution rollback、hyphen、ruby 和 bopomofo
  等现有额外 shaping 路径仍有明确语义；
5. 运行完整测试、fixture golden、桌面示例测试、`git diff --check` 和文档风格检查；若 golden 变化，逐项确认
  变化仅来自预期的字体 identity/evidence 调整；
6. 更新 API 设计报告、`docs/dev-guide.md`，并补齐 ADR、`docs/key-differences.md` 和本迭代的实施结果。

完成条件：crate 可运行、可测试，统一后端的 HarfRust/SkRifa 字体接入和 deterministic fixture 均通过验证；本迭代文档从
“草案”更新为“已完成”，并记录实际测试结果。

## 验证与验收

新增或调整测试至少覆盖：

- 首选候选无 `.notdef` 时，后端只对该候选执行一次用于布局的 shaping；metrics 与 glyph replay 使用相同
  `FontFaceId`；
- 首选 face 缺字而后续受控 face 完整覆盖时，最终 glyph、metrics 与 `render_font_key` 指向同一个 fallback
  face，且没有 cmap 预检查；
- 全部候选缺字时，最终输出首选 face 的 `.notdef` glyph，并在 debug 中保留全部候选的缺字证据；
- 中西混排、拉丁组合文本 `é`、emoji modifier、ZWJ、旗帜与 keycap 保持既有 source/display range、scalar
  offset 和单 face 重放语义；
- annotation cache 命中时，缓存的 shaping、最终 face 与候选尝试信息保持一致；
- deterministic stub fixture、桌面 HarfRust 示例与 glyph replay 保持 source/display range 和 scalar offset
  约定。

完成后运行相关 unit/integration 测试、`cargo test`、layout fixture golden、桌面示例测试、`git diff --check`
和本迭代文档的风格检查。若布局规则未变却出现 fixture golden 差异，必须先定位为 face、metrics、glyph 或
debug evidence 的预期变化；不得通过 renderer 偏移或修改断行规则规避差异。

## 实施结果

- 采用方案 A：每个 `FontBackend::shape` request 独立完成候选选择和完整 shaping，request range 同时作为
  `FontResolution` 的范围；不同 request 不重新聚合为旧的 layout range。
- fixture golden 已按该 request 粒度更新。变化集中在 `font` decision 的范围和数量；行、cluster、尺寸和
  断行结果保持一致。
- 修复多个 resolution 具有相同起点时的确定性排序，避免 `HashMap` 迭代顺序影响 debug 和 metrics decision。
- `cargo test --all-targets`：1348 passed；fixture golden 单独执行并通过；`cargo check --examples` 通过；
  `git diff --check` 通过；本迭代涉及文档的风格检查通过。

## 兼容性与回滚

当前 crate 仍处于开发阶段，可以替换尚未稳定的三个字体 trait；不提供双路径运行时兼容层。需要保留的兼容性是
结果语义：source range、display text、glyph placement、可重放 face identity 和结构化决策必须保持可解释。

若新后端不能在受控字体资源、deterministic fixture 和受控 glyph replay 上同时通过验证，回滚本迭代的调用链和
公开接口改动，恢复当前独立 trait 路径；不在调用方新增 cmap 预检查、多 face 分段或外置绘制作为替代方案。