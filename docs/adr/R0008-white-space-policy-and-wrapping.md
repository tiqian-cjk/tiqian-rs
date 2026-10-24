# ADR R0008：空白处理策略与独立换行控制

- Status: Accepted
- Date: 2026-10-22
- Implementation: Completed
- Relates: [R0007 CSS 风格自动间距与显式空格](R0007-css-autospace-and-explicit-spaces.md)

## Context

R0007 将自动间距与显式空格折叠分开；本 ADR 的空白模式与换行控制已由
[2026-10-22 空白处理与自动间距联合实施](../iteration/2026-10-22-feat-white-space-and-autospace.md)
接入实现。旧实现曾沿用上游 `LineEdgeWordSpaceCollapse`：在行调整阶段扣除行首、行尾纯 U+0020
cluster 的整个 advance，不区分软换行、硬换行或段落结束，也不受自动间距开关控制。

光标、选区和点击命中使用最终 cluster advance，因此行边空格虽然仍存在于 source 中，却失去交互宽度。
仅实施 R0007 无法解决输入框的行尾空格问题；行中折叠与行边处理需要统一的可配置策略。

本决策参考 CSS Text 4 已讨论的空白模式与处理顺序，不承诺实现整个 CSS 排版模型。依据为
[空白处理规则](https://drafts.csswg.org/css-text-4/#white-space-rules)、
[行边处理](https://drafts.csswg.org/css-text-4/#white-space-phase-2) 与
[处理顺序](https://drafts.csswg.org/css-text-4/#order)。

## Decision

### 空白模式与换行控制

空白模式独立于自动间距，并与 `Wrap / NoWrap` 组合。下列名称已由公开 Rust API 实现。

| 空白模式 | 行中连续普通空格 | source 换行 | 行首普通空格 | 行尾普通空格 |
| --- | --- | --- | --- | --- |
| `Collapse` | 折叠 | 按 segment break 规则转为空格或消除 | 移除 | 移除可折叠空格 |
| `PreserveBreaks` | 折叠 | 保留为硬换行 | 移除 | 移除可折叠空格 |
| `Preserve` | 保留 | 保留为硬换行 | 保留 | 开启软换行时允许按 CSS 规则悬挂 |
| `BreakSpaces` | 保留 | 保留为硬换行 | 保留 | 始终计宽，不悬挂 |

`Wrap` 允许在合法机会处软换行；`NoWrap` 禁止软换行，内容过宽时溢出，但保留下来的硬换行仍生效。
空白配置不使用额外的行首、行尾 trim 布尔值；上述模式统一定义行中折叠与行边行为。
CSS `white-space-trim` 的元素边界裁剪能力不在本次决策范围。

`Preserve + Wrap` 对连续空格和 Tab 序列通常仅在整串结束处提供软换行机会；软换行处的行尾空白
无条件悬挂，硬换行及段落结束处按 CSS 的条件悬挂规则处理。`Preserve + NoWrap` 保留空白且不作
此类悬挂。悬挂表示相应宽度不参与行内容适配、对齐和两端对齐的测量，空格仍保留占位及交互几何。

`BreakSpaces` 保留空白，并在每个保留空格及 Tab 后提供软换行机会，包括连续空格内部；其他空格
分隔字符依 CSS 对应规则处理，仍须尊重 NBSP 等不可断行约束。保留空白不能因行边折叠而失去 advance。
开启软换行后可出现纯空格视觉行；单个空格都无法容纳时允许溢出。

### 默认值与使用组合

| 使用场景 | 空白模式 | 换行控制 | 自动间距 |
| --- | --- | --- | --- |
| 核心默认与普通正文 | `PreserveBreaks` | `Wrap` | 默认 `Insert` |
| 多行输入框推荐配置 | `BreakSpaces` | `Wrap` | 默认 `Insert`，可独立关闭 |
| 代码或严格预格式文本推荐配置 | `Preserve` | `NoWrap` | 通常关闭 |

核心默认继续尊重调用方输入的换行，对应 CSS `pre-line`，不采用 CSS 初始值 `normal`。
输入框需要行尾空格悬挂、避免连续空格逐个换行时，可显式选择 `Preserve + Wrap`。
引擎不根据“是否可编辑”暗中切换配置，由调用方选择组合。

### 与 R0007 的执行顺序

1. 按 source 空白序列解析折叠、保留、硬换行及软换行机会，保留 source text 和 scalar range。
2. 自动间距按空白处理后的有效空格判断。`Replace` 的“恰好一个 U+0020”指有效序列，而非原始输入数量。
3. 断行候选计算考虑空白模式所定义的移除、计宽或悬挂，行调整使用相同规则。
4. 最终输出统一的绘制与交互几何；悬挂时区分排版测量宽度与实际占位、溢出范围。

例如 `中␠␠A` 在保留模式下仍有两个普通空格，`Insert` 与 `Replace` 都保留它们；在折叠模式下先成为
一个有效普通空格，`Insert` 保留其自然宽度，`Replace` 再替换为一个 gap。source 中两个空格始终不变。

自动 gap 只在同一行的适用边界存在；保留作者空格不保留跨行自动 gap。标点几何的行边压缩继续由
标点策略负责，不因空白保留而整体关闭。

本 ADR 细化 R0007 的单空格判定阶段，并以可配置空白策略替代其维持既有行边空格处理的约定；
R0007 的默认 `Insert`、保留模式下多空格不替换及 gap 宽度预设继续有效。

### 实施约束

- 空白序列判断跨越 source/style 分片，不依赖字体 backend 恰好如何划分 cluster；保留空白应提供
  对应 source interaction boundary 的位置，`BreakSpaces` 的空格内断行机会不能被合并 cluster 隐藏。
- 普通空格、Tab 与换行分别处理；NBSP、全角空格不参加普通空格折叠。保留 Tab 使用 tab stop，
  不以固定数量空格代替。换行序列与 `Collapse` 的 segment break 转换规则在实施设计中明确。
- 替换无条件 `LineEdgeWordSpaceCollapse`，同步调整断行成本中的行尾折叠假设。结构化 decision 区分
  空白折叠、行边移除、悬挂和自动间距替换，前端不自行重算或补偿宽度。
- source 与原文复制语义保持既有 Rust 约定；不因引入 CSS 风格布局规则改写复制内容。

## Consequences

- 正文可折叠行中和行边空白，输入框可保留每次空格输入对应的占位、光标与选区宽度。
- `Preserve` 的悬挂需要区分排版测量与实际几何；输入框的滚动或裁剪消费方需能获取实际溢出范围。
- `BreakSpaces` 会改变断行机会和行数，核心默认也会改变既有连续空格的显示；相关 fixture 与 golden
  必须在实施时审查更新。
- Rust 不再无条件沿用上游行边空格折叠，作为已接受的决策差异登记。

## Alternatives considered

- **只关闭行边 trim。** 不采用：未统一行中折叠、断行成本、连续空格换行机会与交互几何。
- **空格保留由 auto-space 开关控制。** 不采用：空白格式与中西文间距需要独立组合。
- **核心默认 `Collapse + Wrap`。** 不采用：输入中的换行应默认保持为硬换行。
- **所有保留模式均让行尾空格计入对齐。** 不采用：保留 `Preserve` 的 CSS 风格悬挂能力，始终计宽
  由 `BreakSpaces` 表达。

## Verification

实现与当前验证：

1. 四种空白模式分别覆盖 `Wrap / NoWrap`，并与三种自动间距模式组合；另覆盖 8 种 mandatory
  break 字符（包括 CRLF）、纯空白窄行、查询与 replay。相关实现及回归由联合迭代 Phase 3–5
  验收记录追踪；新增 4 个 fixture 后总数为 57。
2. Chromium 150.0.7871.250、Electron 43.7.7、Windows 浏览器探针中，仅记录 CSS 字体请求
  `Microsoft YaHei, sans-serif`，不声称已识别实际逐 glyph 字体。宽度 64px、字号 16px 时，
  `pre-wrap` 的 `aaaa        ` 右边界为 72.9167px、高度 24px；`break-spaces` 高度 48px；
  `nowrap` 的 `aaaa        b` `scrollWidth` 为 83px。
3. 边界说明：Rust 末尾 CRLF 仍产生额外空行；浏览器 DOM 末尾换行不形成同等高度。这两者不作为
  等价行为或互相替代的验证证据。
4. 最终全目标编译、全量测试及 57 项 fixture 的三 breaker golden 非更新验证通过；
  Phase5 13 项测试通过。性能 A/B 与规模检查及完整结果见
  [当前迭代](../iteration/2026-10-22-feat-white-space-and-autospace.md)。