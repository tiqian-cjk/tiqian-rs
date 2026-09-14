# 2026-09-03 Rust 本地 layout fixture 与 golden 迁移

## 目标

将 Kotlin `EarlyLayoutFixtures` 的全部 52 项 deterministic stub 回归迁移为 Rust 仓库内、由 `cargo test` 直接执行的 layout fixture golden 测试。完成后，日常 Rust 测试、覆盖率报告和 golden 更新不依赖相邻 Kotlin checkout、Gradle、JSON exporter 或 shell fixture runner。

本轮迁移保留每项 fixture 的完整输入配置、三个 line breaker 和完整 decision dump 比较。既有按规则编写的 Rust 单测继续保留，但不作为 fixture golden 的替代依据。

## 已确认的事实

### 当前普通 fixture 验证

Kotlin 的 `engine/src/commonTest/kotlin/org/tiqian/test/LayoutFixtures.kt` 定义 52 项 `EarlyLayoutFixtures.all`。每项包含文本、约束、段落样式、注音、装饰、技术断行 span、连字符策略和禁则配置。

Kotlin `layoutFixtureDump` 与 Rust `examples/fixture-layout-dump.rs` 均按以下顺序运行同一 fixture：

1. `GreedyLineBreaker`；
2. `LookaheadLineBreaker`；
3. `ParagraphDpLineBreaker`。

普通 fixture 使用 deterministic stub text shaper 与 stub font metrics。当前 Rust 的 `tools/verify-fixture.sh` 从 Kotlin `exportLayoutFixture` 读取 JSON，交给 `fixture-layout-dump` example，再将结果逐字节与 Kotlin `golden/layout-dumps/<id>.txt` 比较。

普通 dump 除文本和尺寸外，还覆盖 line、cluster、font decision、role override、标点几何、spacing、autospace、断行机会、line repair、justification、mandatory break、zero-width break、ruby、bopomofo、decoration、line height 等结构化结果。

### 当前 recorded 路径

Kotlin recorded 路径复用同一 52 项 fixture 输入，但使用 Skia/HarfBuzz 录制的 shaping 与 font metrics evidence，输出另一套 recorded dump。当前 evidence 约 1.29 MB，recorded dump 约 648 KB。

recorded 路径验证的是给定 Kotlin 录制 shaping/metrics 答案时的布局回放。Rust 不能独立录制、更新或审阅这套 Kotlin 平台 evidence；它不是本轮 Rust 本地 deterministic fixture golden 的输入来源。

### 不适用的旧结论

此前按“已有局部 Rust 单测”或“fixture 额外覆盖的源代码行数”将 52 项划分为保留、排除和暂不迁移的做法无效。局部单测和 coverage 只说明局部行为或执行路径，不能替代完整 fixture 输入、三种 breaker 和 decision dump 回归。

本轮不再对 52 项 fixture 进行必要性筛选。

## 决策

### 迁移范围

- 迁移全部 52 项普通 deterministic stub fixture。
- 每项完整保留当前 fixture 输入和三个 breaker 的输出比较。
- Rust 本地 fixture 与 golden 成为日常回归的唯一测试资产。
- Kotlin fixture 定义和普通 golden 在初始迁移、后续上游同步时作为参考来源，不参与 Rust 日常测试。

### 不迁移的内容

本轮不迁移以下 Kotlin 测试资产：

- `shaping-evidence.json`；
- `layout-dumps-recorded/`；
- recorded evidence resolver；
- Kotlin JSON exporter；
- Kotlin 端的 golden 更新命令；
- Kotlin 跨 target parity 机制。

本轮也不改变 `TextRange` 的 UTF-16 语义，不引入 Unicode scalar value range。

### Rust 本地测试资产

新增下列测试目录：

```text
tests/
  fixture_layout/
    mod.rs
    cases.rs
    dump.rs
    golden/
      <fixture-id>.txt
  layout_fixture_golden_test.rs
```

`tests/tiqian.rs` 注册 `layout_fixture_golden_test` 测试模块。`layout_fixture_golden_test.rs` 通过 `#[path = "fixture_layout/mod.rs"]` 引入测试支持模块。测试资产位于 `tests/`，不进入 crate 的生产 API 或发布包。

| 文件 | 职责 |
| --- | --- |
| `tests/fixture_layout/cases.rs` | 52 项 fixture 的原生 Rust 定义。 |
| `tests/fixture_layout/dump.rs` | fixture 执行、三个 breaker 的选择、完整 dump 格式化、golden 路径与差异报告。 |
| `tests/fixture_layout/mod.rs` | 测试支持模块的内部出口。 |
| `tests/fixture_layout/golden/<id>.txt` | 一项 fixture 的三个 breaker 完整预期输出。 |
| `tests/layout_fixture_golden_test.rs` | 遍历 fixture、读写 golden、比较结果的测试入口。 |

### 输入格式

fixture 输入使用 Rust 类型定义，不使用 JSON、TOML、YAML 或自定义文本解析格式。

测试支持代码定义一个仅供测试使用的 fixture 描述类型，至少包括：

- `id: &'static str`；
- 构造 `LayoutInput` 所需的数据；
- `use_english_hyphenation`；
- `pin_basic_no_hang`。

每项 fixture 使用现有 Rust 类型构造完整输入：

- `Text`、`TiqianTextContent`、`LayoutConstraints`；
- `TextStyle`、`ParagraphStyle`、`LayoutProfileId`；
- `LineLengthGrid`、`LineBreakSpan`、`LineBreakPolicy`；
- `DecorationSpan`、`DecorationKind`；
- `RubySpan`、`RubyKind`、`RubyLineHeightMode`；
- 必要的 source boundaries 与 autospace-suppressed ranges。

fixture 执行器每次 layout 都创建新的 `ExplainableStubParagraphLayoutEngine`，再按 fixture 配置：

- 安装当前 breaker；
- 按 `use_english_hyphenation` 选择 `english_hyphenation::en_us()` 或 `NoHyphenator`；
- 按 `pin_basic_no_hang` 使用当前 `FixtureProfileResolver` 等价的 fixed basic kinsoku 配置；
- 其余部分使用 deterministic stub 默认实现。

本轮 52 项输入不包含非空 `TextSpan`、`InlineBoxSpan` 或 `InlineObjectSpan`。fixture 描述类型不为这些未使用的输入建立字段或解析分支；将来确有 fixture 使用时再按实际输入扩展。

### Golden 格式

每项 fixture 对应一个 UTF-8 文本文件：

```text
fixture: <id>
text: <escaped source text>
maxWidth: <one-decimal float>
== greedy ==
<greedy decision dump>
== lookahead ==
<lookahead decision dump>
== paragraph-dp ==
<paragraph-dp decision dump>
```

Rust `dump.rs` 定义格式化规则，成为该格式的唯一实现。初始迁移阶段，输出需与当前 Kotlin 普通 stub dump 逐字节兼容，以便逐项校对迁移；完成迁移后，该格式和本地 golden 由 Rust 仓库维护。

格式化必须保留当前 dump 的下列规则：

- 浮点数固定一位小数，按绝对值 half-up 舍入，并保留负零符号；
- `NaN`、正无穷和负无穷分别输出 `NaN`、`Infinity`、`-Infinity`；
- 换行、回车、VT、FF、NEL、LS、PS、ZWSP 使用现有转义表示；
- 字段输出顺序与当前 Rust runner 一致；
- line、cluster、font、role、punctuation、geometry、spacing、break、annotation 与 decoration 输出不得省略。

每项 golden 只包含一个 fixture，三个 breaker 结果连续放在同一文件中。不得将所有 fixture 合并为一个大 golden，也不得将完整 dump 降级为少量局部断言。

### Golden 读取、比较与更新

正常测试通过 `CARGO_MANIFEST_DIR/tests/fixture_layout/golden/<id>.txt` 读取 golden。读取失败、fixture 缺少 golden、golden 文件没有对应 fixture，均必须使测试失败。

正常模式不得写入任何 golden。比较失败的输出至少包含：

- fixture ID；
- expected 与 actual 的首处不同；
- 统一 diff 或等效的逐行差异；
- 所属 breaker 的上下文。

更新仅在环境变量 `TIQIAN_UPDATE_LAYOUT_GOLDENS=1` 明确设置时允许执行。更新模式：

1. 重新生成全部 52 项本地 golden；
2. 只写入 `tests/fixture_layout/golden/`；
3. 不删除未知 golden 文件；
4. 结束后仍检查 fixture ID 与 golden 文件名集合相等；
5. 不调用 Gradle，不读取 `TIQIAN_ROOT`，不读取 Kotlin checkout。

更新命令应限定到 fixture golden 测试入口：

```text
TIQIAN_UPDATE_LAYOUT_GOLDENS=1 cargo test --test tiqian layout_fixture_golden_test
```

更新后必须逐项审阅文本 diff。环境变量是生成预期输出的显式确认，不代表行为变化自动正确。

## Fixture 清单

以下 52 个 ID 必须各有一个 Rust fixture 定义和一个对应 golden：

1. `basic-pause-stop`
2. `ellipsis-and-dash`
3. `nested-quotes`
4. `adjacent-punctuation-spacing`
5. `contextual-curly-quotes`
6. `mixed-script-quote-paragraph-language`
7. `adjacent-curly-quote-list-context`
8. `mi10s-adjacent-curly-quote-wrap`
9. `mi10s-western-bracket-citation-wrap`
10. `bibliographic-numeric-locator-break`
11. `unmatched-curly-quotes`
12. `fallback-roles`
13. `greedy-multi-line`
14. `kinsoku-carry-previous`
15. `kinsoku-push-in`
16. `lookahead-future-push-in`
17. `lookahead-avoids-repair`
18. `justify-cjk-paragraph`
19. `justify-mixed-paragraph`
20. `justify-unbreakable-number-symbol`
21. `ascii-brackets-in-cjk`
22. `ascii-point-mark-in-cjk`
23. `ascii-point-mark-impossible-measure`
24. `real-paragraph-1`
25. `latin-word-wrap`
26. `emphasis-marks`
27. `ruby-line-height`
28. `bopomofo-tone-em-box`
29. `first-line-indent`
30. `latin-camelcase`
31. `latin-existing-hyphen`
32. `latin-hard-break`
33. `latin-opaque-url-token`
34. `zero-width-space-soft-break`
35. `western-hyphenation`
36. `progressive-technical-inline`
37. `progressive-technical-hash-fill`
38. `progressive-technical-alpha-numeric`
39. `progressive-technical-current-line-emergency`
40. `adaptive-short-line-indent`
41. `mandatory-single-newline`
42. `mandatory-blank-lines`
43. `mandatory-leading-trailing-newline`
44. `mandatory-crlf`
45. `mandatory-wraps-long-line`
46. `indent-opening-quote`
47. `line-end-kinsoku`
48. `interlinear-lines`
49. `mourning-frame`
50. `contextual-dash-ellipsis`
51. `parenthetical-dash-pairs`
52. `quote-digit-boundaries`

实现前必须从 Kotlin `LayoutFixtures.kt` 和当前 exporter 输出逐项交叉核对：ID、文本、UTF-16 range、约束、样式、装饰、ruby、line break span、hyphenation、kinsoku 和 grid 配置均不得遗漏。

## 实施 phases

各 phase 的中间状态可以暂时不可构建或不可测试。仅 Phase 5 结束时要求项目恢复为完整可构建、可测试状态。

### Phase 1：建立 Rust 测试基础

实施：

1. 创建 `tests/fixture_layout/` 和 `tests/layout_fixture_golden_test.rs`。
2. 将现有 runner 中与普通 deterministic stub 路径相关的执行和 dump 逻辑移入测试支持模块。
3. 删除测试支持代码对 JSON wire 和 recorded evidence 的依赖。
4. 实现 fixture ID 与 golden 文件名集合检查、正常比较模式和显式更新模式。
5. 先用一个最简单的 fixture 验证测试入口和 golden 路径；此时 corpus 未完整迁移，测试可以暂时失败。

完成条件：测试支持模块能独立构建，且不从 Kotlin checkout 读取输入、golden 或 evidence。

### Phase 2：迁移全部输入与普通 golden

实施：

1. 将 52 项 Kotlin fixture 的完整输入逐项写入 `cases.rs`。
2. 将 52 个普通 `layout-dumps/*.txt` 迁入 Rust golden 目录。
3. 逐项运行 Rust fixture 测试，与迁入 golden 比较。
4. 对初始迁移中的每项差异，先确认是 Rust fixture 输入、Rust dump 格式或 Rust layout 行为的差异；确认原因前不得重写 local golden。
5. 在普通 corpus 全部逐字节匹配前保留旧跨仓库验证路径作为迁移校对工具。

完成条件：52 个 fixture 定义、52 个 golden 文件和 golden 文件名检查全部存在；Rust 本地测试在不设置更新变量时通过。

### Phase 3：移除跨仓库 fixture 实现

实施：

1. 删除 `examples/fixture-layout-dump.rs`。
2. 删除 `examples/fixture_layout_dump/recorded_shaping_evidence.rs`。
3. 删除 `tools/verify-fixture.sh`、`tools/verify-all-fixtures.sh`、`tools/verify-recorded-fixtures.sh`。
4. 从 `Cargo.toml` 删除仅供 fixture runner 使用的 `serde` 与 `serde_json` 开发依赖；删除前以全仓搜索确认无其他使用者。
5. 将 `tools/coverage.sh` 改为只运行 Rust targets 和 LLVM coverage，不调用 shell fixture runner、Gradle 或 Kotlin checkout。

完成条件：Rust 日常测试、fixture golden 更新和 coverage 代码路径中不再出现 `TIQIAN_ROOT`、`TIQIAN_SHAPING_EVIDENCE`、`exportLayoutFixture` 或 `fixture-layout-dump`。

### Phase 4：更新当前文档

更新仍描述当前开发入口的文档：

1. `README.md`：开发环境只要求 Rust toolchain；说明本地 fixture golden 测试、更新命令和 coverage 命令。
2. `docs/tracking.md`：将 fixture 验证映射到 Rust 本地 fixture golden 测试；保留 Kotlin 在上游同步时作为参考来源的说明。
3. `docs/key-differences.md`：移除会将局部单测或 coverage 增量解释为 fixture 替代的文字；记录本地 fixture golden 的最终验证边界。
4. `docs/api-design-report.md`：将开发期验证适配器由 `examples/fixture-layout-dump.rs` 更新为 Rust 测试支持模块，避免描述已删除的 example。

`docs/archived/` 和既有 `docs/tracking/<date>-*.md` 是历史记录，本轮不编辑。

完成条件：当前文档不再把 Kotlin checkout、Gradle、recorded evidence 或 shell fixture runner 作为 Rust 日常验证入口。

### Phase 5：最终验证与审阅

1. 运行 `cargo test --test tiqian layout_fixture_golden_test`。
2. 运行 `cargo test --all-targets`。
3. 运行 `cargo llvm-cov --html --all-targets` 或更新后的 `tools/coverage.sh`，确认 coverage 不依赖 Kotlin checkout。
4. 运行 `git diff --check`。
5. 对本轮改动的中文文档运行文档风格检查。
6. 搜索活动代码、脚本、README 和当前维护文档，确认不再引用已删除的 runner、脚本、`TIQIAN_ROOT` 或 `TIQIAN_SHAPING_EVIDENCE`。
7. 审阅全部 52 个 golden 的初始迁移 diff，以及任何因 Rust 格式定义导致的差异。
8. 检查 `git status`，不处理并行工作的无关改动。

完成条件：项目可构建、全部测试通过、coverage 可在单独的 Rust checkout 中生成，且 fixture golden 的输入、输出、更新与执行均由 Rust 仓库拥有。

## 实施记录

2026-09-03 已完成本迭代：

1. 新增 `tests/fixture_layout/`，以原生 Rust 定义全部 52 项普通 deterministic stub fixture；每项运行 greedy、lookahead、paragraph-DP 三种 breaker。
2. 新增 52 个本地 golden，并在初始迁移时与 Kotlin 普通 `layout-dumps/` 逐字节比较，文件集合与内容均一致。
3. 新增 `layout_fixture_golden_test`；默认模式只读比较，`TIQIAN_UPDATE_LAYOUT_GOLDENS=1` 时重新生成本地 golden。
4. 删除 Kotlin JSON runner、recorded evidence replay 和跨仓库 shell 验证脚本；移除直接 `serde`、`serde_json` 开发依赖。
5. 更新 coverage、README 和当前维护文档，使日常验证只依赖 Rust checkout。

已运行：

```text
cargo test --test tiqian layout_fixture_golden_test
cargo test --all-targets
bash tools/coverage.sh
```

三项验证均通过；coverage 报告生成在 `target/llvm-cov/html/index.html`。`ParagraphDpLineBreaker::gap_boundaries` 的既有 dead-code warning 未随本迭代处理。

## 风险与处理

| 风险 | 处理 |
| --- | --- |
| Kotlin fixture 默认值未被完整改写 | 以 Kotlin exporter 的已解析输入逐项交叉核对 Rust fixture 定义。 |
| Rust formatter 与当前 stub golden 不兼容 | 初始迁移阶段逐字节对照；先修 formatter 或确认上游格式变动，再写入本地 golden。 |
| 更新变量被误用 | 仅指定测试入口响应 `TIQIAN_UPDATE_LAYOUT_GOLDENS=1`，更新后要求审阅所有 golden diff。 |
| golden 文件缺失、重复或过期 | 正常模式检查 fixture ID 与 golden 文件名集合相等。 |
| 删除 runner 后仍存在外部依赖 | Phase 5 搜索脚本、文档和代码中的 Kotlin fixture 入口及环境变量。 |

## 回滚

本轮文件变更均局限于测试资产、开发脚本、开发依赖和文档。若本地 fixture 测试无法达到 52 项逐字节匹配，保留现有跨仓库 runner，撤回尚未完成的本地 fixture 测试与删除操作；不得在未验证的状态下删除现有 Kotlin 对照路径。
