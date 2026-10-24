# ADR R0007：CSS 风格自动间距与显式空格

- Status: Accepted
- Date: 2026-10-22
- Implementation: Completed
- Relates: Kotlin ADR 0009 `AutoSpacePolicy`

## Context

Kotlin ADR 0009 将 `Insert` 定义为 `Replace` 的超集：无空格的中西文边界增加 gap，已有一个或多个
U+0020 的边界统一为一个 gap。Rust 旧实现也采用该行为。默认字号 16、gap 为 1/8em 时，旧实现曾将
`01 基础组件` 中数字后的空格缩为 2，而 `02 Input 输入框` 中对应空格保持字体原始宽度；
该差异现已按本 ADR 决策处理。实现与验收进度见
[2026-10-22 空白处理与自动间距联合实施](../iteration/2026-10-22-feat-white-space-and-autospace.md)。

[CSS Text 4](https://drafts.csswg.org/css-text-4/#text-autospace-property) 与
[MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/text-autospace)
将自动间距与空白处理分开：默认 `insert` 尊重已有空格，`replace` 允许替换 U+0020，
连续空白是否折叠由 `white-space` 控制。当前 CSS 对多个保留空格的替换细节缺少明确说明；
Rust 需要为该情况作出可测试的约定。

## Decision

Rust 的自动间距模式采用以下语义。W/N 指当前 `East_Asian_Spacing` 分类的适用边界，方向可反转；
表中的空格均为 U+0020，宽度为行内调整前的基础宽度。

| 边界输入 | `Disabled` | `Insert`（默认） | `Replace` |
| --- | --- | --- | --- |
| WN，无空格 | 不添加 gap | 添加一个 gap | 添加一个 gap |
| W 与 N 之间恰好一个空格 | 保留空格 | 保留空格，不添加 gap | 用一个 gap 替换该空格 |
| W 与 N 之间两个或更多空格 | 全部保留 | 全部保留，不添加 gap | 全部保留，不添加 gap |

1. `cjk_latin` 和 `cjk_digit` 默认值均确认采用 `AutoSpaceMode::Insert`。
2. `Replace` 只匹配两个适用 source 单元之间恰好一个 U+0020。连续多个空格整体保留，
   不从中抽取一个替换，也不逐个替换。该数量约束是 Rust 的明确假设；浏览器不支持本次测试所用
   的显式 `insert` / `replace` 组合值，故不将其表述为浏览器实测结论。
3. 其他空格分隔字符阻止跨该字符添加或替换自动间距；保留其字体宽度与既有断行语义。
4. 保留现有边界分类、自动间距抑制、行边处理与宽度预设：默认 gap 为 1/8em、拉伸上限为 1/3em，
   `AutoSpacePolicy::clreq()` 为 1/4em 与 1/2em。此处对齐 CSS 的模式语义，宽度仍使用 Rust 的 em 配置。
5. source text 和 range 保持原样，替换仅改变布局占位，并记录实际 `Insert` 或 `Replace` decision。

决策已由上述联合迭代实施；以下实现证据不改变本节取舍。`white-space` 配置与自动间距的执行顺序
按 R0008 及该迭代记录执行。

## Consequences

- 默认自动间距会保留作者输入的单个和多个空格，与已测浏览器默认 `normal` 行为相符。
- 两条编号原文中的首个空格将保留字体原始宽度；列表正文的严格列对齐仍由编号区域或缩进等布局结构负责。
- 行为实施后，自动间距不再提供将多个 U+0020 折叠为一个 gap 的能力；后续可通过独立空白配置恢复折叠能力。
- 与上游 ADR 0009 的模式语义形成有意差异。同步时按本 ADR 保留 Rust 决策，不能机械恢复旧行为。
- `Replace` 的多空格规则未来如获得明确规范或实现证据，再通过新 ADR 决定是否调整。

## Alternatives considered

- **维持任意数量空格归一为 gap。** 不采用：默认自动间距会改变作者显式间隔，且承担了空白折叠职责。
- **多个空格中替换一个，其余保留。** 不采用：本决策将连续多个空格视为应完整保留的显式间隔。
- **同时引入完整 `white-space` 配置。** 后续单独讨论，当前先确定自动间距与显式空格的边界。

## Verification

当前证据与验收：

1. 自动间距回归矩阵覆盖四种空白模式 × 三种自动间距模式，并覆盖双向 W/N、字母／数字、
   0–3 个 U+0020 及跨 style/source 分片；对应新增用例和联合验证记录见
   [当前迭代](../iteration/2026-10-22-feat-white-space-and-autospace.md)。
2. 使用 `DemoFontCatalog` 的 Inter、Source Han 字体与 HarfRust 验证两条编号原文；两条首空格
   均为 4.5px。另以 `Input` 后三个空格验证 cursor x 为 42.742188、47.242188、51.742188。
3. 浏览器实测环境为 Chromium 150.0.7871.250、Electron 43.7.7、Windows、CSS 字体请求 Microsoft YaHei 32px、
   `normal`：无空格的 `中A` 增加 4px；`pre-wrap` 下两个显式空格不产生自动间距增量。
   显式 `insert` / `replace` 的 `CSS.supports` 检查为 false，因此相关组合不计入浏览器实测证据。
4. 已覆盖空白折叠、行边行为及布局 decision；全目标编译、全量测试和 57 项 fixture 的三 breaker
   golden 非更新验证通过，浏览器、字体与 A/B 性能证据见当前迭代。