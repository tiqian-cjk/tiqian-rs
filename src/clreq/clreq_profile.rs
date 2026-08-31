// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/clreq/ClreqProfile.kt

use crate::common::HashSet;

use super::super::core::text::Text;
use super::super::font::font_policy::FontRole;
use super::super::core::text_model::{LayoutProfileId, built_in_layout_profiles};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClreqStrictness {
    Loose,
    Normal,
    Strict,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClreqProfile {
    pub id: String,
    pub strictness: ClreqStrictness,
    pub region: ClreqRegion,
    pub punctuation_glyph_policy: CjkPunctuationGlyphPolicy,
    pub coalesce_repeatable_punctuation: HashSet<i32>,
    pub auto_space: AutoSpacePolicy,
    pub glue_placement: PunctuationGluePlacement,
    pub adjustment: AdjustmentStylePolicy,
    /**
     * 行首行尾禁则档与行尾悬挂的解析方式。默认为 [`KinsokuMode::MeasureAdaptive`]，
     * 按行长（字数）自适应；[`KinsokuMode::Fixed`] 固定一档。
     */
    pub kinsoku_mode: KinsokuMode,
    /// 标点宽度风格（全身式 / 开明式；GB 固定半宽连接号等）。
    pub punctuation_width: PunctuationWidthPolicy,
}

impl ClreqProfile {
    pub fn new(id: String, strictness: ClreqStrictness, region: ClreqRegion) -> Self {
        Self {
            id,
            strictness,
            region,
            punctuation_glyph_policy: CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints,
            coalesce_repeatable_punctuation: default_coalesce_repeatable_punctuation(),
            auto_space: AutoSpacePolicy::default_policy(),
            glue_placement: PunctuationGluePlacement::for_region(region),
            adjustment: AdjustmentStylePolicy::default(),
            kinsoku_mode: KinsokuMode::measure_adaptive(),
            punctuation_width: PunctuationWidthPolicy::default(),
        }
    }

    pub fn mainland_horizontal() -> Self {
        Self::new(
            "clreq-mainland-horizontal".to_owned(),
            ClreqStrictness::Normal,
            ClreqRegion::Mainland,
        )
    }

    pub fn taiwan_horizontal() -> Self {
        Self::new(
            "clreq-taiwan-horizontal".to_owned(),
            ClreqStrictness::Normal,
            ClreqRegion::Taiwan,
        )
    }

    pub fn hong_kong_horizontal() -> Self {
        Self::new(
            "clreq-hongkong-horizontal".to_owned(),
            ClreqStrictness::Normal,
            ClreqRegion::HongKong,
        )
    }

    pub fn builder(
        id: String,
        strictness: ClreqStrictness,
        region: ClreqRegion,
    ) -> ClreqProfileBuilder {
        ClreqProfileBuilder {
            profile: Self::new(id, strictness, region),
        }
    }
}

pub struct ClreqProfileBuilder {
    profile: ClreqProfile,
}

impl ClreqProfileBuilder {
    pub fn punctuation_glyph_policy(mut self, value: CjkPunctuationGlyphPolicy) -> Self {
        self.profile.punctuation_glyph_policy = value;
        self
    }

    pub fn coalesce_repeatable_punctuation(mut self, value: HashSet<i32>) -> Self {
        self.profile.coalesce_repeatable_punctuation = value;
        self
    }

    pub fn auto_space(mut self, value: AutoSpacePolicy) -> Self {
        self.profile.auto_space = value;
        self
    }

    pub fn glue_placement(mut self, value: PunctuationGluePlacement) -> Self {
        self.profile.glue_placement = value;
        self
    }

    pub fn adjustment(mut self, value: AdjustmentStylePolicy) -> Self {
        self.profile.adjustment = value;
        self
    }

    pub fn kinsoku_mode(mut self, value: KinsokuMode) -> Self {
        self.profile.kinsoku_mode = value;
        self
    }

    pub fn punctuation_width(mut self, value: PunctuationWidthPolicy) -> Self {
        self.profile.punctuation_width = value;
        self
    }

    pub fn build(self) -> ClreqProfile {
        self.profile
    }
}

// `CoalesceRepeatablePunctuation`：连续重复时形成单个语义标点 cluster 的码点
//（CLREQ 双字宽破折号与省略号）。将其列在 profile 中，使区域覆盖不需要修改引擎代码。
// 在 Kotlin 中必须先于 `MainlandHorizontal` 声明，才能让其构造函数默认值完成解析。
pub fn default_coalesce_repeatable_punctuation() -> HashSet<i32> {
    [0x2014, 0x2026, 0x22EF].into_iter().collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClreqRegion {
    Mainland,
    Taiwan,
    HongKong,
    Custom,
}

/**
 * 标点 atom 的半宽字面在 em box 中的位置，以及剩余空间作为 glue 落在哪一侧。
 * 根据 CLREQ 3.1.3（标点符号的位置），同一字符会随区域采用不同位置：
 *
 * - 简体中文（Mainland）：句号/逗号居于格内左下，body 锚定前侧，glue 全在后侧；
 * - 繁体中文（Taiwan / Hong Kong）：句号/逗号居中，body 居中，glue 分布于两侧；
 * - 开始标点（`「（《〈『`）与之镜像：大陆样式 body 锚定后侧，繁体样式居中；
 *   大陆样式的区域默认值为仅前侧 glue，即 [`GlueSide::LeadingOnly`]。
 *
 * 根据 ADR 0014，这是由区域/profile 驱动的排版决策，而不是由字体 glyph 位置决定。
 * 对所有标点都居中的低质量字体（早期 Microsoft YaHei、部分方正字体）由渲染层使用
 * ink bounds 平移 glyph，使其落在 profile 指定的位置；加法 glue 模型仍从此处推导方向。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PunctuationGluePlacement {
    /// 大陆/简体惯例。
    MainlandSimplified,
    /// 繁体中文惯例（台湾、香港）。
    Traditional,
}

impl PunctuationGluePlacement {
    pub fn for_region(region: ClreqRegion) -> Self {
        match region {
            ClreqRegion::Mainland | ClreqRegion::Custom => Self::MainlandSimplified,
            ClreqRegion::Taiwan | ClreqRegion::HongKong => Self::Traditional,
        }
    }

    /// 返回给定标点类别中 glue 相对于 body 所处的位置。
    pub fn glue_side_for(self, punctuation_class: PunctuationClass) -> GlueSide {
        match self {
            Self::MainlandSimplified => match punctuation_class {
                PunctuationClass::Opening => GlueSide::LeadingOnly,
                PunctuationClass::Closing | PunctuationClass::PauseOrStop => GlueSide::TrailingOnly,
                _ => GlueSide::BothSides,
            },
            // 根据 CLREQ 3.1.3，繁体样式将 。、，等居中，因此 Opening 与
            // Closing/PauseOrStop 都使用 BothSides；单侧锚定仅属于简体样式。
            Self::Traditional => GlueSide::BothSides,
        }
    }
}

/// 给定标点类别中 glue 相对于 body 所处的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlueSide {
    LeadingOnly,
    TrailingOnly,
    BothSides,
}

/**
 * `AutoSpacePolicy` 控制 Unicode `East_Asian_Spacing` Wide↔Narrow 边界如何实体化。
 * `cjk_latin` 是非十进制 Narrow 值的模式（包括 Greek、Cyrillic，以及在中文 locale
 * 中解析后的 Conditional 值）；`cjk_digit` 保留 CSS 风格的十进制数字覆盖。
 * 边界分类与字体角色无关，并固定在 core Unicode 表中。
 *
 * 参见 [ADR 0009](docs/adr/0009-autospace-policy.md)。
 *
 * 逐边界 [`AutoSpaceMode`] 决定：
 * - `Disabled`：引擎不插入间距；输入的 U+0020 按名义 1em advance 绘制，亦即经典 stub 行为。
 * - `Replace`（默认）：Wide↔Narrow 边界上输入的 U+0020 被吸收到 autospace gap 中。
 *   空格 cluster 的 advance 从 1em 缩至 `gap_em`，可见结果只有一个可配置 gap，
 *   而不是 1em 与 autospace 的重复累加。
 * - `Insert`：输入的 U+0020 保持完整 1em，同时额外加入 autospace gap。它供需要让 U+0020
 *   经过复制/粘贴往返的编辑流程使用；当前 slice 预留但尚未实现，因为需要注入虚拟 cluster。
 *
 * `gap_em` 与 `stretch_max_em` 是同一对值：gap 的基础宽度与 justify 拉伸后的最终上限。
 * 两者均以 em 为单位（ADR 0009 修订）。CLREQ 要求 1/4–1/2；主流实践（iOS、Chrome
 * `text-autospace`）收敛到 1/8 基准值，CLREQ 自身的注②也记载了将拉伸限制在 1/3 的样式，
 * 因此默认采用 1/8–1/3，CLREQ 字面值 1/4–1/2 由 [`Self::clreq`] 提供。引擎将这对值
 * 传入 Justifier，其他位置不再重复声明这些数值。
 */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoSpacePolicy {
    pub cjk_latin: AutoSpaceMode,
    pub cjk_digit: AutoSpaceMode,
    pub gap_em: f32,
    /// justify 拉伸上限（最终宽度，em）——间距对 `gap_em..stretch_max_em`。
    pub stretch_max_em: f32,
}

impl AutoSpacePolicy {
    /// 实践收敛值：1/8 基准、1/3 上限（iOS / Chrome `text-autospace`）。默认。
    pub fn default_policy() -> Self {
        Self {
            cjk_latin: AutoSpaceMode::Insert,
            cjk_digit: AutoSpaceMode::Insert,
            gap_em: 0.125,
            stretch_max_em: 1.0 / 3.0,
        }
    }

    /// CLREQ 字面：1/4 基准、1/2 上限。
    pub fn clreq() -> Self {
        Self {
            gap_em: 0.25,
            stretch_max_em: 0.5,
            ..Self::default_policy()
        }
    }

    pub fn disabled() -> Self {
        Self {
            cjk_latin: AutoSpaceMode::Disabled,
            cjk_digit: AutoSpaceMode::Disabled,
            ..Self::default_policy()
        }
    }

    pub fn builder() -> AutoSpacePolicyBuilder {
        AutoSpacePolicyBuilder {
            policy: Self::default_policy(),
        }
    }
}

pub struct AutoSpacePolicyBuilder {
    policy: AutoSpacePolicy,
}

impl AutoSpacePolicyBuilder {
    pub fn cjk_latin(mut self, value: AutoSpaceMode) -> Self {
        self.policy.cjk_latin = value;
        self
    }

    pub fn cjk_digit(mut self, value: AutoSpaceMode) -> Self {
        self.policy.cjk_digit = value;
        self
    }

    pub fn gap_em(mut self, value: f32) -> Self {
        self.policy.gap_em = value;
        self
    }

    pub fn stretch_max_em(mut self, value: f32) -> Self {
        self.policy.stretch_max_em = value;
        self
    }

    pub fn build(self) -> AutoSpacePolicy {
        self.policy
    }
}

impl Default for AutoSpacePolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/**
 * 行内调整（挤压/拉伸）的风格开关。CLREQ 的调整程序是默认侧；每个开关都
 * 对应原文点名的「部分排版风格」变体。
 */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdjustmentStylePolicy {
    /**
     * 严格风格（默认）：行尾标点无条件削成半宽（`LineEndHalfWidthPunctuation`）。
     * 宽松风格：行尾标点保留全宽，其空白只在需要挤压时按需消耗——字身网格
     * 在行尾保持完整，墨迹缘允许参差。
     */
    pub line_end_punctuation: LineEndPunctuationStyle,
    /**
     * CLREQ 挤压第④档：「位于行内的句号、问号、感叹号……最小挤为半个汉字字宽」。
     * 「有些排版风格禁止此项调整，而保持句号、问号、惊叹号固定一个字宽」；设为 false
     * 时这些标点不进入挤压容量。
     */
    pub allow_inline_stop_compression: bool,
    /**
     * 「在一些排版风格中，中西间距固定默认宽度……被排除在行内调整对象之外，
     * 不允许被挤压（/拉伸）」；设为 false 时 sino-western gap 既不进入挤压容量，
     * 也不参与 justify 的优先拉伸或最后的统一字距。
     */
    pub allow_sino_western_gap_adjustment: bool,
    /**
     * 行尾越界字的「推入/推出」取舍（CLREQ §6.2.2「先挤进、后推出」与行内
     * 「先挤压、后拉伸」）。默认 [`LineAdjustmentStrategy::PushInFirst`]，采用固定顺序；
     * 曾有「偏差最小化」的 Auto 档，但实际观感差，已删除（ADR 0031 修订）。
     */
    pub line_adjustment: LineAdjustmentStrategy,
}

impl Default for AdjustmentStylePolicy {
    fn default() -> Self {
        Self {
            line_end_punctuation: LineEndPunctuationStyle::ForceHalfWidth,
            allow_inline_stop_compression: true,
            allow_sino_western_gap_adjustment: true,
            line_adjustment: LineAdjustmentStrategy::PushInFirst,
        }
    }
}

impl AdjustmentStylePolicy {
    pub fn builder() -> AdjustmentStylePolicyBuilder {
        AdjustmentStylePolicyBuilder {
            policy: Self::default(),
        }
    }
}

pub struct AdjustmentStylePolicyBuilder {
    policy: AdjustmentStylePolicy,
}

impl AdjustmentStylePolicyBuilder {
    pub fn line_end_punctuation(mut self, value: LineEndPunctuationStyle) -> Self {
        self.policy.line_end_punctuation = value;
        self
    }

    pub fn allow_inline_stop_compression(mut self, value: bool) -> Self {
        self.policy.allow_inline_stop_compression = value;
        self
    }

    pub fn allow_sino_western_gap_adjustment(mut self, value: bool) -> Self {
        self.policy.allow_sino_western_gap_adjustment = value;
        self
    }

    pub fn line_adjustment(mut self, value: LineAdjustmentStrategy) -> Self {
        self.policy.line_adjustment = value;
        self
    }

    pub fn build(self) -> AdjustmentStylePolicy {
        self.policy
    }
}

/**
 * 行尾越界那一字落在本行还是下一行的取舍——压缩（推入）与拉伸（推出）的方向选择。
 * 压缩/拉伸的档内分配始终按 §6.2.2.3/§6.2.2.4 的 tier 顺序，本枚举只决定方向。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineAdjustmentStrategy {
    /// 先推入：压得动就把越界字挤进本行（CLREQ「先挤进」的字面顺序），压不动才推出。默认。
    PushInFirst,
    /// 先推出：能断就把越界字推到下一行并拉伸本行，只有推出会触发均排兜底时才回头推入。
    PushOutFirst,
    /// 仅推出：永不为容纳越界字而压缩，一律断行并拉伸（旧行为）。
    PushOutOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEndPunctuationStyle {
    ForceHalfWidth,
    AllowFullWidth,
}

/**
 * CLREQ 第六节「行首行尾禁则」四档（逐档收紧）。命名对齐 CLREQ 原文：
 *
 * - `None`（不处理）：完全不处理行首行尾禁则，常见于台港报刊。
 * - `Basic`（基本处理）：点号、结束引号/括号/书名号乙式、连接号、间隔号、分隔号
 *   不得居行首；开始引号/括号/书名号乙式不得居行尾。CLREQ 称其为「最推荐的方法」，
 *   也是本项目默认值。
 * - `GbStyle`（GB 法）：在基本处理上追加分隔号不得居行尾。
 * - `Strict`（严格处理）：在 GB 法上追加破折号、省略号不得居行首。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KinsokuLevel {
    None,
    Basic,
    GbStyle,
    Strict,
}

/**
 * 标点宽度风格——标点的默认占宽，落到加法模型上即「字面 + 空隙」中的空隙
 * 是否默认归零（半字、不可调）。
 *
 * - `interior` 选择全身式（默认）或开明式。开明式下，句中点号（逗号、顿号、分号、
 *   冒号）与夹注/括号/引号占半字，句末点号（句号、问号、感叹号）仍占一字。
 *   CLREQ issue #572：「句中点号、夹注号半字，句末点号（除行末外）一字」。
 * - `gb_fixed_separators` 启用 GB 式固定半宽：连接号、间隔号、分隔号固定半字且不可调整
 *   （CLREQ「不可调整的标点……固定半个字宽」）。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PunctuationWidthPolicy {
    pub interior: InteriorPunctuationStyle,
    pub gb_fixed_separators: bool,
}

impl Default for PunctuationWidthPolicy {
    fn default() -> Self {
        Self {
            interior: InteriorPunctuationStyle::FullWidth,
            gb_fixed_separators: false,
        }
    }
}

impl PunctuationWidthPolicy {
    pub fn new(interior: InteriorPunctuationStyle, gb_fixed_separators: bool) -> Self {
        Self {
            interior,
            gb_fixed_separators,
        }
    }

    pub fn with_interior(interior: InteriorPunctuationStyle) -> Self {
        Self::new(interior, false)
    }

    pub fn with_gb_fixed_separators(gb_fixed_separators: bool) -> Self {
        Self::new(InteriorPunctuationStyle::FullWidth, gb_fixed_separators)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteriorPunctuationStyle {
    FullWidth,
    Kaiming,
}

/// 为给定行长解析出的禁则档与悬挂风格。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedKinsoku {
    pub level: KinsokuLevel,
    pub hanging: HangingPunctuationStyle,
    pub reason: String,
}

/**
 * 禁则档与悬挂的选择方式。`Fixed` 固定两者；`MeasureAdaptive` 根据 wiki/文献语料实验，
 * 按行长（汉字数）选择：
 *
 * - 行长小于 14 字：启用行尾悬挂。窄行（手机正文）悬挂可消除无法修复的行，
 *   避免 CarryPrevious 将整字腰斩式推出，收益在此区间最明显。
 * - 行长大于 24 字：采用 GB 法，追加分隔号禁行尾。
 * - 行长大于 32 字：采用严格处理，追加破折号/省略号禁行首；宽行可负担更严禁则，
 *   实验中宽行采用更严档的代价趋近于零。
 * - 其余行长：采用 CLREQ 称为「最推荐」的基本处理。
 *
 * CLREQ 主张「一份文档内禁则级别应统一」；自适应是面向响应式/移动端重排
 * （measure 随容器变化）的现代扩展，决策写入 dump。
 */
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KinsokuMode {
    Fixed {
        level: KinsokuLevel,
        hanging: HangingPunctuationStyle,
    },
    MeasureAdaptive {
        hang_below_em: f32,
        gb_above_em: f32,
        strict_above_em: f32,
    },
}

impl KinsokuMode {
    pub fn fixed(level: KinsokuLevel) -> Self {
        Self::Fixed {
            level,
            hanging: HangingPunctuationStyle::Disabled,
        }
    }

    pub fn fixed_with_hanging(level: KinsokuLevel, hanging: HangingPunctuationStyle) -> Self {
        Self::Fixed { level, hanging }
    }

    pub fn measure_adaptive() -> Self {
        Self::MeasureAdaptive {
            hang_below_em: 14.0,
            gb_above_em: 24.0,
            strict_above_em: 32.0,
        }
    }

    pub fn measure_adaptive_with_thresholds(
        hang_below_em: f32,
        gb_above_em: f32,
        strict_above_em: f32,
    ) -> Self {
        Self::MeasureAdaptive {
            hang_below_em,
            gb_above_em,
            strict_above_em,
        }
    }

    pub fn resolve(self, measure_em: f32) -> ResolvedKinsoku {
        match self {
            Self::Fixed { level, hanging } => ResolvedKinsoku {
                level,
                hanging,
                reason: format!(
                    "Fixed:{level:?}{}",
                    if hanging != HangingPunctuationStyle::Disabled {
                        "+Hang"
                    } else {
                        ""
                    }
                ),
            },
            Self::MeasureAdaptive {
                hang_below_em,
                gb_above_em,
                strict_above_em,
            } => {
                let level = if measure_em > strict_above_em {
                    KinsokuLevel::Strict
                } else if measure_em > gb_above_em {
                    KinsokuLevel::GbStyle
                } else {
                    KinsokuLevel::Basic
                };
                let hanging = if measure_em < hang_below_em {
                    HangingPunctuationStyle::PauseStops
                } else {
                    HangingPunctuationStyle::Disabled
                };
                ResolvedKinsoku {
                    level,
                    hanging,
                    reason: format!(
                        "MeasureAdaptiveKinsoku:{}字→{level:?}{}",
                        measure_em as i32,
                        if hanging != HangingPunctuationStyle::Disabled {
                            "+Hang"
                        } else {
                            ""
                        }
                    ),
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HangingPunctuationStyle {
    /// 不悬挂（默认）：行尾点号走挤进/推出修复链。
    Disabled,
    /**
     * 悬挂顿号、逗号、句号（CLREQ「适合行尾悬挂的标点符号有顿号、逗号及句号」）。
     * 行尾只悬挂一个。
     */
    PauseStops,
}

/**
 * CLREQ：「原则上，汉字与西文字母、数字间使用不多于四分之一个汉字宽的字距或空白。」
 * 无论作者是否输入 U+0020，该 gap 都存在。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoSpaceMode {
    Disabled,
    /**
     * 仅将边界上输入的空格规范为 [`AutoSpacePolicy::gap_em`]（`TextAutoSpaceReplace`）；
     * 无输入空格的边界不添加内容。这是 Insert 之前的保守行为，供将缺少空格视为作者意图的样式保留。
     */
    Replace,
    /**
     * 完整 CLREQ 行为，是 Replace 的超集：无输入空格的边界也增加
     * [`AutoSpacePolicy::gap_em`]（`TextAutoSpaceInsert`）。默认。
     */
    Insert,
}

pub trait ClreqProfileResolver {
    fn resolve(&self, profile_id: &LayoutProfileId) -> ClreqProfile;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BuiltInClreqProfileResolver;

impl ClreqProfileResolver for BuiltInClreqProfileResolver {
    fn resolve(&self, profile_id: &LayoutProfileId) -> ClreqProfile {
        let clreq_horizontal = built_in_layout_profiles::clreq_horizontal();
        if profile_id.value == clreq_horizontal.value
            || profile_id.value == "clreq-mainland-horizontal"
        {
            ClreqProfile::mainland_horizontal()
        } else {
            ClreqProfile::mainland_horizontal()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CjkPunctuationGlyphPolicy {
    PreserveInput,
    PreferClreqRecommendedCodepoints,
    ForceClreqRecommendedCodepoints,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PunctuationClass {
    Opening,
    Closing,
    PauseOrStop,
    MiddleDot,
    Interpunct,
    Connector,
    Solidus,
    Ellipsis,
    Dash,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PunctuationPolicy {
    pub punctuation_class: PunctuationClass,
    pub allow_at_line_start: bool,
    pub allow_at_line_end: bool,
    pub default_body_em: f32,
    pub default_advance_em: f32,
}

impl PunctuationPolicy {
    pub fn with_default_advance(
        punctuation_class: PunctuationClass,
        allow_at_line_start: bool,
        allow_at_line_end: bool,
        default_body_em: f32,
    ) -> Self {
        Self {
            punctuation_class,
            allow_at_line_start,
            allow_at_line_end,
            default_body_em,
            default_advance_em: 1.0,
        }
    }

    pub fn new(
        punctuation_class: PunctuationClass,
        allow_at_line_start: bool,
        allow_at_line_end: bool,
        default_body_em: f32,
        default_advance_em: f32,
    ) -> Self {
        Self {
            punctuation_class,
            allow_at_line_start,
            allow_at_line_end,
            default_body_em,
            default_advance_em,
        }
    }
}

pub mod clreq_punctuation_policies {
    use super::*;

    /**
     * `AsciiPointMark`：方向明确的西文点号。它们保留 Latin 字体与自然 advance；
     * 此判断仅向断行暴露标点语义，包括它们在中文横排正文中的非典型用法。
     * 直引号被有意排除，因为 U+0022/U+0027 不能确定开始或结束方向。
     */
    pub fn is_ascii_point_mark(character: char) -> bool {
        ASCII_POINT_MARKS.contains(&character)
    }

    pub fn classify(character: char) -> PunctuationClass {
        match character {
            '“' | '‘' | '（' | '《' | '〈' | '「' | '『' | '【' | '〔' | '〖' | '〘' | '〚' => {
                PunctuationClass::Opening
            }
            '”' | '’' | '）' | '》' | '〉' | '」' | '』' | '】' | '〕' | '〗' | '〙' | '〛' => {
                PunctuationClass::Closing
            }
            '，' | '、' | '。' | '；' | '：' | '！' | '？' => PunctuationClass::PauseOrStop,
            '·' => PunctuationClass::MiddleDot,
            '・' | '‧' | '•' => PunctuationClass::Interpunct,
            '～' | '~' | '-' | '–' => PunctuationClass::Connector,
            '/' | '／' => PunctuationClass::Solidus,
            '…' | '⋯' => PunctuationClass::Ellipsis,
            '—' | '⸺' => PunctuationClass::Dash,
            _ => PunctuationClass::Other,
        }
    }

    /**
     * 判断字符是否因标点宽度 policy 被强制为固定半宽（仅 body，无可调整 glue）。
     * 此结果驱动 atom builder 覆盖 advance。
     */
    pub fn forced_half_width(character: char, policy: PunctuationWidthPolicy) -> bool {
        // 短横线占半个字位置（CLREQ 5.1.6，与风格无关；grid 占位覆盖字体 glyph advance）。
        if SHORT_HYPHEN_CONNECTORS.contains(&character) {
            return true;
        }
        let class = classify(character);
        if policy.gb_fixed_separators
            && matches!(
                class,
                PunctuationClass::Connector
                    | PunctuationClass::MiddleDot
                    | PunctuationClass::Interpunct
                    | PunctuationClass::Solidus
            )
        {
            return true;
        }
        if policy.interior == InteriorPunctuationStyle::Kaiming {
            // 句中点号（，、；：）与夹注/括号/引号半字；句末点号（。！？）全字。
            if matches!(class, PunctuationClass::Opening | PunctuationClass::Closing) {
                return true;
            }
            if class == PunctuationClass::PauseOrStop && !SENTENCE_END_STOPS.contains(&character) {
                return true;
            }
        }
        false
    }

    pub fn policy_for(character: char) -> PunctuationPolicy {
        let punctuation_class = classify(character);
        PunctuationPolicy {
            punctuation_class,
            // 布尔字段保存 CLREQ 基本处理档（最推荐的默认值）；KinsokuLevel 在其上追加差异。
            allow_at_line_start: !forbidden_at_line_start(character, KinsokuLevel::Basic),
            allow_at_line_end: !forbidden_at_line_end(character, KinsokuLevel::Basic),
            default_body_em: default_punctuation_body_em(character, punctuation_class),
            default_advance_em: default_punctuation_advance_em(character, punctuation_class),
        }
    }

    /**
     * 行首行尾禁则按 CLREQ 第六节四档逐档收紧。CLREQ：「行首行尾禁则规定属于排版风格，
     * 用户代理实现时可以根据自身实际情况，选择或者自定义……更宽松或者严格的禁则」。
     *
     * 破折号/省略号在基本处理与 GB 法下不禁于行首，只保护其不被拆行
     * （见 `clreq-punctuation-audit.md`）；对话破折号本就以行首开头。仅严格处理追加此禁则。
     */
    pub fn forbidden_at_line_start(character: char, level: KinsokuLevel) -> bool {
        if level == KinsokuLevel::None {
            return false;
        }
        match classify(character) {
            PunctuationClass::PauseOrStop
            | PunctuationClass::Closing
            | PunctuationClass::Connector
            | PunctuationClass::MiddleDot
            | PunctuationClass::Interpunct
            | PunctuationClass::Solidus => true,
            PunctuationClass::Dash | PunctuationClass::Ellipsis => level == KinsokuLevel::Strict,
            _ => false,
        }
    }

    pub fn forbidden_at_line_end(character: char, level: KinsokuLevel) -> bool {
        if level == KinsokuLevel::None {
            return false;
        }
        match classify(character) {
            PunctuationClass::Opening => true,
            PunctuationClass::Solidus => level != KinsokuLevel::Basic,
            _ => false,
        }
    }

    fn default_punctuation_body_em(character: char, punctuation_class: PunctuationClass) -> f32 {
        if character == '⸺' {
            2.0
        } else if SHORT_HYPHEN_CONNECTORS.contains(&character)
            || matches!(
                punctuation_class,
                PunctuationClass::PauseOrStop
                    | PunctuationClass::Closing
                    | PunctuationClass::Opening
            )
        {
            0.5
        } else {
            1.0
        }
    }

    fn default_punctuation_advance_em(character: char, punctuation_class: PunctuationClass) -> f32 {
        if character == '⸺' {
            2.0
        } else if SHORT_HYPHEN_CONNECTORS.contains(&character) {
            0.5
        } else if punctuation_class == PunctuationClass::Other {
            1.0
        } else {
            1.0
        }
    }

    /// 句末点号（句号、问号、感叹号）在开明式下仍占一字。
    const SENTENCE_END_STOPS: [char; 4] = ['。', '！', '？', '．'];
    /// 短横线是连接号的一种，占半个字位置（CLREQ / GB/T 15834 5.1.6），与一字宽浪纹线 ～ 区别。
    const SHORT_HYPHEN_CONNECTORS: [char; 2] = ['-', '–'];
    const ASCII_POINT_MARKS: [char; 6] = [',', '.', ':', ';', '!', '?'];
}

pub mod clreq_punctuation_advance_policy {
    use super::Text;

    pub fn advance_em(source_text: &Text, display_text: &Text) -> f32 {
        if display_text == "⸺" || source_text == "⸺" {
            2.0
        } else {
            source_text.chars().count() as f32
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CjkPunctuationGlyphSubstitution {
    pub source_text: Text,
    pub display_text: Text,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClreqPunctuationGlyphSubstitutor {
    policy: CjkPunctuationGlyphPolicy,
}

impl Default for ClreqPunctuationGlyphSubstitutor {
    fn default() -> Self {
        Self::new(CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints)
    }
}

impl ClreqPunctuationGlyphSubstitutor {
    pub fn new(policy: CjkPunctuationGlyphPolicy) -> Self {
        Self { policy }
    }

    pub fn substitute(&self, source_text: &Text) -> CjkPunctuationGlyphSubstitution {
        let display_text = match self.policy {
            CjkPunctuationGlyphPolicy::PreserveInput => source_text.clone(),
            CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints
            | CjkPunctuationGlyphPolicy::ForceClreqRecommendedCodepoints => {
                to_clreq_recommended_display_text(source_text)
            }
        };
        let reason = if display_text == *source_text {
            format!("CjkPunctuationGlyphPolicy:{:?}:preserve", self.policy)
        } else {
            format!(
                "CjkPunctuationGlyphPolicy:{:?}:{}->{}",
                self.policy,
                to_code_point_labels(source_text),
                to_code_point_labels(&display_text)
            )
        };
        CjkPunctuationGlyphSubstitution {
            source_text: source_text.clone(),
            display_text,
            reason,
        }
    }

    /**
     * `CjkRoleGatedDisplaySubstitution` keeps CLREQ display-codepoint
     * replacement downstream of contextual font-role resolution.
     */
    pub fn substitute_for_role(
        &self,
        source_text: &Text,
        role: FontRole,
    ) -> CjkPunctuationGlyphSubstitution {
        let candidate = self.substitute(source_text);
        if role == FontRole::CjkPunctuation || candidate.display_text == *source_text {
            candidate
        } else {
            CjkPunctuationGlyphSubstitution {
                source_text: source_text.clone(),
                display_text: source_text.clone(),
                reason: format!("CjkRoleGatedDisplaySubstitution:preserve-role-{role:?}"),
            }
        }
    }
}

fn to_clreq_recommended_display_text(source_text: &Text) -> Text {
    if (0..source_text.utf16_len())
        .all(|offset| source_text.utf16_code_unit_at(offset) == '…' as i32)
    {
        Text::from("⋯".repeat(source_text.utf16_len() as usize))
    } else if source_text == "——" {
        Text::from("⸺")
    } else if matches!(source_text.as_str(), "・" | "‧" | "•") {
        Text::from("·")
    } else {
        source_text.clone()
    }
}

fn to_code_point_labels(text: &Text) -> String {
    text.encode_utf16()
        .map(|unit| format!("U+{unit:04X}"))
        .collect::<Vec<_>>()
        .join("+")
}
