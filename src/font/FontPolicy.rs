// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/font/FontPolicy.kt

use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{EmojiPresentation, GeneralCategory, Script},
};

use super::super::core::Geometry::TextRange;
use super::super::core::Text::Text;
use super::FontMetrics::{BaselineClass, FontMetricSource, MetricBox};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRequest {
    pub preferred_families: Vec<String>,
    pub locale: String,
    pub role: FontRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontRole {
    CjkText,
    CjkPunctuation,
    LatinText,
    Symbol,
    Emoji,
    Unknown,
}

/**
 * `LatinVsCjkFaceSelection`——shaping、metrics 与 rendering 选择 Latin/CJK face 时必须共享的
 * 唯一规则。若三者各自推导，结果会漂移：在 Latin face 中测量的缺字 `.notdef` 约为 0.65em box，
 * 却在 CJK face 中按全 em box 绘制，便会溢出槽位并与下一个 cluster 相撞。
 *
 * 只有真正的 Latin text 使用 Latin face。Symbol、Emoji、Unknown 均回退至 CJK face，例如缺字会
 * 绘制为与 CJK 正文一致的全 em 字身框豆腐，而不是 Latin 半宽框；两种情况下 measure 都必须等于 draw。
 */
impl FontRole {
    pub fn uses_latin_face(self) -> bool {
        self == Self::LatinText
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "CjkText" => Some(Self::CjkText),
            "CjkPunctuation" => Some(Self::CjkPunctuation),
            "LatinText" => Some(Self::LatinText),
            "Symbol" => Some(Self::Symbol),
            "Emoji" => Some(Self::Emoji),
            "Unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// 供仅持有序列化 role name（LayoutResult dump）的调用方使用。
pub fn font_role_name_uses_latin_face(role_name: Option<&str>) -> bool {
    role_name
        .and_then(FontRole::from_name)
        .is_some_and(FontRole::uses_latin_face)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontCandidate {
    pub key: String,
    pub family: String,
    pub role: FontRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontDecision {
    pub range: TextRange,
    pub candidate: FontCandidate,
    pub role: FontRole,
    pub reason: String,
}

pub trait FallbackResolver {
    fn resolve(&self, text: &Text, range: TextRange, request: &FontRequest) -> FontDecision;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontRoleContext {
    pub locale: String,
    pub region_hint: Option<String>,
}

impl Default for FontRoleContext {
    fn default() -> Self {
        Self {
            locale: "zh-Hans".to_owned(),
            region_hint: None,
        }
    }
}

impl FontRoleContext {
    pub fn new(locale: String, region_hint: Option<String>) -> Self {
        Self {
            locale,
            region_hint,
        }
    }

    pub fn with_locale(locale: String) -> Self {
        Self::new(locale, None)
    }
}

pub trait FontRoleClassifier {
    fn classify(&self, text: &Text, range: TextRange, context: &FontRoleContext) -> FontRole;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CjkFontRoleClassifier;

impl CjkFontRoleClassifier {
    pub fn classify_with_default_context(&self, text: &Text, range: TextRange) -> FontRole {
        <Self as FontRoleClassifier>::classify(self, text, range, &FontRoleContext::default())
    }
}

impl FontRoleClassifier for CjkFontRoleClassifier {
    fn classify(&self, text: &Text, range: TextRange, _context: &FontRoleContext) -> FontRole {
        let first_code_point = text.code_point_at_compat(range.start(), text.utf16_len());
        // Printable ASCII can never reach the CJK or curly-quote branches below,
        // so resolve it without touching the Unicode property tries.
        if is_typed_ascii_latin(first_code_point) {
            FontRole::LatinText
        } else if is_cjk_code_point(first_code_point) {
            FontRole::CjkText
        // 仅弯引号是 CJK/Western 共享码点，需由上下文决定。其他字符均为原生归属：
        // 输入的 ASCII 属于 Latin，CJK 码点属于 CJK。
        } else if is_latin_curly_quote(first_code_point, text, range) {
            FontRole::LatinText
        } else if is_cjk_punctuation_code_point(first_code_point) {
            FontRole::CjkPunctuation
        } else if is_latin_code_point(first_code_point) {
            FontRole::LatinText
        } else if is_emoji_code_point(first_code_point) {
            FontRole::Emoji
        } else if is_symbol_code_point(first_code_point) {
            FontRole::Symbol
        } else {
            FontRole::Unknown
        }
    }
}

fn is_cjk_code_point(code_point: i32) -> bool {
    matches!(
        CodePointMapData::<Script>::new().get32(code_point as u32),
        Script::Bopomofo | Script::Han
    )
}

fn is_cjk_punctuation_code_point(code_point: i32) -> bool {
    (0x3000..=0x303F).contains(&code_point)
        || code_point == 0x2014
        || is_ambiguous_curly_quote(code_point)
        || code_point == 0x2013
        || code_point == 0x203C
        || code_point == 0x2047
        || code_point == 0x2026
        || code_point == 0x2027
        || code_point == 0x22EF
        || code_point == 0x30FB
        || code_point == 0x2E3A
        || code_point == 0x00B7
        || code_point == 0x2022
        || code_point == 0xFF01
        || code_point == 0xFF1F
        || code_point == 0xFF0C
        || code_point == 0xFF0E
        || code_point == 0xFF0F
        || code_point == 0xFF1A
        || code_point == 0xFF1B
        || code_point == 0xFF08
        || code_point == 0xFF09
        || code_point == 0xFF5E
}

fn is_latin_curly_quote(code_point: i32, text: &Text, range: TextRange) -> bool {
    is_ambiguous_curly_quote(code_point)
        && text
            .code_point_before(range.start())
            .is_some_and(is_latin_run_code_point)
        && text
            .code_point_at_or_none(range.end())
            .is_some_and(is_latin_run_code_point)
}

fn is_ambiguous_curly_quote(code_point: i32) -> bool {
    matches!(code_point, 0x2018 | 0x2019 | 0x201C | 0x201D)
}

fn is_latin_run_code_point(code_point: i32) -> bool {
    is_typed_ascii_latin(code_point)
        || is_ambiguous_curly_quote(code_point)
        || is_latin_code_point(code_point)
}

fn is_latin_code_point(code_point: i32) -> bool {
    CodePointMapData::<Script>::new().get32(code_point as u32) == Script::Latin
}

/**
 * 所有可打印 ASCII（U+0020..U+007E）均表示输入的 Latin 意图，归入 [`FontRole::LatinText`]。
 * 唯一共享的 CJK/Western 码点是弯引号（U+2018–201D），由 `is_latin_curly_quote` 按上下文解析；
 * 真正的 CJK 码点（—、…、、和全宽 FF**）会先被 `is_cjk_punctuation_code_point` 捕获。因此 ASCII
 * `%`、`.`、`-`、`/` 使用 Western 渲染并与相邻 Latin 聚合，而不会落到 CJK face（ADR 0029：ASCII
 * `-`、`/` 是英文连字符，不是 CJK 连接号）。U+0020 SPACE 按 ADR 0009 加入 Latin run，其 advance
 * 稍后由边界上的 `ClreqProfile.autoSpace` 调整。
 */
fn is_typed_ascii_latin(code_point: i32) -> bool {
    (0x0020..=0x007E).contains(&code_point)
}

pub(crate) fn is_emoji_code_point(code_point: i32) -> bool {
    CodePointSetData::new::<EmojiPresentation>().contains32(code_point as u32)
}

fn is_symbol_code_point(code_point: i32) -> bool {
    matches!(
        CodePointMapData::<GeneralCategory>::new().get32(code_point as u32),
        GeneralCategory::MathSymbol
            | GeneralCategory::CurrencySymbol
            | GeneralCategory::ModifierSymbol
            | GeneralCategory::OtherSymbol
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferCjkForAmbiguousPunctuationResolver {
    cjk_font_key: String,
    latin_font_key: String,
    symbol_font_key: String,
}

impl Default for PreferCjkForAmbiguousPunctuationResolver {
    fn default() -> Self {
        Self::new(
            "cjk-primary".to_owned(),
            "latin-primary".to_owned(),
            "symbol-fallback".to_owned(),
        )
    }
}

impl PreferCjkForAmbiguousPunctuationResolver {
    pub fn new(cjk_font_key: String, latin_font_key: String, symbol_font_key: String) -> Self {
        Self {
            cjk_font_key,
            latin_font_key,
            symbol_font_key,
        }
    }

    pub fn builder() -> PreferCjkForAmbiguousPunctuationResolverBuilder {
        PreferCjkForAmbiguousPunctuationResolverBuilder {
            resolver: Self::default(),
        }
    }
}

pub struct PreferCjkForAmbiguousPunctuationResolverBuilder {
    resolver: PreferCjkForAmbiguousPunctuationResolver,
}

impl PreferCjkForAmbiguousPunctuationResolverBuilder {
    pub fn cjk_font_key(mut self, value: String) -> Self {
        self.resolver.cjk_font_key = value;
        self
    }

    pub fn latin_font_key(mut self, value: String) -> Self {
        self.resolver.latin_font_key = value;
        self
    }

    pub fn symbol_font_key(mut self, value: String) -> Self {
        self.resolver.symbol_font_key = value;
        self
    }

    pub fn build(self) -> PreferCjkForAmbiguousPunctuationResolver {
        self.resolver
    }
}

impl FallbackResolver for PreferCjkForAmbiguousPunctuationResolver {
    fn resolve(&self, _text: &Text, range: TextRange, request: &FontRequest) -> FontDecision {
        let role = request.role;
        let candidate = match role {
            FontRole::CjkText | FontRole::CjkPunctuation => FontCandidate {
                key: self.cjk_font_key.clone(),
                family: request
                    .preferred_families
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.cjk_font_key.clone()),
                role,
            },
            FontRole::LatinText => FontCandidate {
                key: self.latin_font_key.clone(),
                family: self.latin_font_key.clone(),
                role,
            },
            FontRole::Symbol | FontRole::Emoji | FontRole::Unknown => FontCandidate {
                key: self.symbol_font_key.clone(),
                family: self.symbol_font_key.clone(),
                role,
            },
        };
        FontDecision {
            range,
            candidate,
            role,
            reason: format!("PreferCjkForAmbiguousPunctuationResolver:{role:?}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontMetricsPolicy {
    Raw,
    IdeographicBox,
    GlyphBoundsSampled,
    ManualOverride,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselinePolicy {
    Alphabetic,
    Ideographic,
    CenteredCjkVisual,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub source: FontMetricSource,
    /**
     * 字体声明的 typographic box（OpenType `OS/2` sTypoAscender/Descender），以 baseline
     * 上下方的正数长度表示。对 CJK 字体，它是干净的 ideographic em（例如 Source Han
     * 0.88 / 0.12 = 1.000em），不同于由 hhea 推导并被放大的 `ascent` / `descent`。
     * 字体没有 `OS/2` 表时为 `None`，normalizer 回退至 `ascent` / `descent`。参见 ADR 0002 修订。
     */
    pub typo_ascent: Option<f32>,
    pub typo_descent: Option<f32>,
}

impl RawFontMetrics {
    pub fn new(ascent: f32, descent: f32) -> Self {
        Self {
            ascent,
            descent,
            leading: 0.0,
            source: FontMetricSource::RawTables,
            typo_ascent: None,
            typo_descent: None,
        }
    }

    pub fn builder(ascent: f32, descent: f32) -> RawFontMetricsBuilder {
        RawFontMetricsBuilder {
            metrics: Self::new(ascent, descent),
        }
    }
}

pub struct RawFontMetricsBuilder {
    metrics: RawFontMetrics,
}

impl RawFontMetricsBuilder {
    pub fn leading(mut self, value: f32) -> Self {
        self.metrics.leading = value;
        self
    }

    pub fn source(mut self, value: FontMetricSource) -> Self {
        self.metrics.source = value;
        self
    }

    pub fn typo_ascent(mut self, value: Option<f32>) -> Self {
        self.metrics.typo_ascent = value;
        self
    }

    pub fn typo_descent(mut self, value: Option<f32>) -> Self {
        self.metrics.typo_descent = value;
        self
    }

    pub fn build(self) -> RawFontMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutFontMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub baseline_offset: f32,
    pub policy: FontMetricsPolicy,
    pub baseline_policy: BaselinePolicy,
    pub baseline_class: BaselineClass,
    pub metric_box: MetricBox,
    pub source: FontMetricSource,
    pub reason: String,
}

impl LayoutFontMetrics {
    pub fn new(
        ascent: f32,
        descent: f32,
        baseline_offset: f32,
        policy: FontMetricsPolicy,
        baseline_policy: BaselinePolicy,
    ) -> Self {
        Self {
            ascent,
            descent,
            baseline_offset,
            policy,
            baseline_policy,
            baseline_class: BaselineClass::Roman,
            metric_box: MetricBox::RawFontBox,
            source: FontMetricSource::RawTables,
            reason: String::new(),
        }
    }

    pub fn builder(
        ascent: f32,
        descent: f32,
        baseline_offset: f32,
        policy: FontMetricsPolicy,
        baseline_policy: BaselinePolicy,
    ) -> LayoutFontMetricsBuilder {
        LayoutFontMetricsBuilder {
            metrics: Self::new(ascent, descent, baseline_offset, policy, baseline_policy),
        }
    }
}

pub struct LayoutFontMetricsBuilder {
    metrics: LayoutFontMetrics,
}

impl LayoutFontMetricsBuilder {
    pub fn baseline_class(mut self, value: BaselineClass) -> Self {
        self.metrics.baseline_class = value;
        self
    }

    pub fn metric_box(mut self, value: MetricBox) -> Self {
        self.metrics.metric_box = value;
        self
    }

    pub fn source(mut self, value: FontMetricSource) -> Self {
        self.metrics.source = value;
        self
    }

    pub fn reason(mut self, value: String) -> Self {
        self.metrics.reason = value;
        self
    }

    pub fn build(self) -> LayoutFontMetrics {
        self.metrics
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PunctuationFontPolicy {
    PreferCjkForAmbiguousPunctuation,
    PreferLatinForAscii,
    PreserveRunFont,
    CustomMap,
}
