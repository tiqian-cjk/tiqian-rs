// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/font/FontMetrics.kt

use super::super::core::text::Text;
use super::font_policy::{
    BaselinePolicy, FontMetricsPolicy, FontRole, LayoutFontMetrics, RawFontMetrics,
};

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetricsRequest {
    pub font_key: String,
    pub font_size: f32,
    pub role: FontRole,
    pub locale: String,
    /**
     * 每个 span 的字体家族偏好（rich text）。resolver 测量该家族自身的字身框，使 serif/mono run
     * 按自己的 ideographic box 对齐，而不是按基础字体对齐。空列表表示使用该 role 的默认值。
     */
    pub font_families: Vec<String>,
    /// 请求其声明 metrics 的确切 OpenType weight instance。
    pub font_weight: i32,
    /// 是否请求 italic/oblique face instance，而不是 upright face。
    pub italic: bool,
    /**
     * source text 仅用于解析 `font_families` 中哪个具体 face 拥有此次 metric decision。
     * metrics 仍属于 face level；text 是 fallback selection evidence 的一部分，而不是 glyph bounds sample。
     */
    pub face_selection_text: Text,
}

impl FontMetricsRequest {
    pub fn new(font_key: String, font_size: f32, role: FontRole, locale: String) -> Self {
        Self {
            font_key,
            font_size,
            role,
            locale,
            font_families: Vec::new(),
            font_weight: 400,
            italic: false,
            face_selection_text: Text::new(),
        }
    }

    pub fn builder(
        font_key: String,
        font_size: f32,
        role: FontRole,
        locale: String,
    ) -> FontMetricsRequestBuilder {
        FontMetricsRequestBuilder {
            request: Self::new(font_key, font_size, role, locale),
        }
    }
}

pub struct FontMetricsRequestBuilder {
    request: FontMetricsRequest,
}

impl FontMetricsRequestBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.request.font_families = value;
        self
    }

    pub fn font_weight(mut self, value: i32) -> Self {
        self.request.font_weight = value;
        self
    }

    pub fn italic(mut self, value: bool) -> Self {
        self.request.italic = value;
        self
    }

    pub fn face_selection_text(mut self, value: Text) -> Self {
        self.request.face_selection_text = value;
        self
    }

    pub fn build(self) -> FontMetricsRequest {
        self.request
    }
}

pub trait FontMetricsResolver {
    fn resolve(&self, request: &FontMetricsRequest) -> RawFontMetrics;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StubFontMetricsResolver;

impl FontMetricsResolver for StubFontMetricsResolver {
    fn resolve(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        match request.role {
            FontRole::CjkText | FontRole::CjkPunctuation => RawFontMetrics {
                // hhea 风格的放大 box（为没有 OS/2 的 fallback 路径保留）；typo 字段是 layout 使用的、
                // 由字体声明的 ideographic em。数值镜像 Source Han Sans CN，参见 FontProvidedMetricsProbe。
                ascent: request.font_size * 1.16,
                descent: request.font_size * 0.288,
                leading: 0.0,
                source: FontMetricSource::RawTables,
                typo_ascent: Some(request.font_size * 0.88),
                typo_descent: Some(request.font_size * 0.12),
            },
            FontRole::LatinText => RawFontMetrics {
                ascent: request.font_size * 0.8,
                descent: request.font_size * 0.2,
                leading: 0.0,
                source: FontMetricSource::RawTables,
                typo_ascent: None,
                typo_descent: None,
            },
            FontRole::Symbol | FontRole::Emoji | FontRole::Unknown => RawFontMetrics {
                ascent: request.font_size * 0.9,
                descent: request.font_size * 0.25,
                leading: 0.0,
                source: FontMetricSource::RawTables,
                typo_ascent: None,
                typo_descent: None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetricsNormalizationInput {
    pub request: FontMetricsRequest,
    pub raw_metrics: RawFontMetrics,
}

pub trait FontMetricsNormalizer {
    fn normalize(&self, input: &FontMetricsNormalizationInput) -> LayoutFontMetrics;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptAwareFontMetricsNormalizer;

impl FontMetricsNormalizer for ScriptAwareFontMetricsNormalizer {
    fn normalize(&self, input: &FontMetricsNormalizationInput) -> LayoutFontMetrics {
        let request = &input.request;
        match request.role {
            FontRole::CjkText | FontRole::CjkPunctuation => {
                // ADR 0002 修订：CJK line box 是字体在真实 baseline 上声明的 ideographic em
                //（OS/2 sTypo），而不是围绕伪 baseline 居中的合成对称正方形。字体缺少 OS/2 typo
                // metrics 时回退至放大的 hhea box，而不是凭空发明；ink sampling 是另一条面向坏字体的
                // fallback，不属于此路径。
                let typo = input.raw_metrics.typo_ascent.is_some()
                    && input.raw_metrics.typo_descent.is_some();
                LayoutFontMetrics {
                    ascent: input
                        .raw_metrics
                        .typo_ascent
                        .unwrap_or(input.raw_metrics.ascent),
                    descent: input
                        .raw_metrics
                        .typo_descent
                        .unwrap_or(input.raw_metrics.descent),
                    baseline_offset: 0.0,
                    policy: if typo {
                        FontMetricsPolicy::IdeographicBox
                    } else {
                        FontMetricsPolicy::Raw
                    },
                    baseline_policy: BaselinePolicy::Ideographic,
                    baseline_class: BaselineClass::IdeographicLow,
                    metric_box: MetricBox::IdeographicEmBox,
                    source: input.raw_metrics.source,
                    reason: format!(
                        "ScriptAwareFontMetricsNormalizer:{:?}:{}",
                        request.role,
                        if typo {
                            "font-typo-box"
                        } else {
                            "hhea-fallback-no-os2"
                        }
                    ),
                }
            }
            FontRole::LatinText => LayoutFontMetrics {
                ascent: input.raw_metrics.ascent,
                descent: input.raw_metrics.descent,
                baseline_offset: 0.0,
                policy: FontMetricsPolicy::Raw,
                baseline_policy: BaselinePolicy::Alphabetic,
                baseline_class: BaselineClass::Roman,
                metric_box: MetricBox::RawFontBox,
                source: input.raw_metrics.source,
                reason: format!(
                    "ScriptAwareFontMetricsNormalizer:{:?}:roman-raw",
                    request.role
                ),
            },
            FontRole::Symbol | FontRole::Emoji | FontRole::Unknown => LayoutFontMetrics {
                ascent: input.raw_metrics.ascent,
                descent: input.raw_metrics.descent,
                baseline_offset: 0.0,
                policy: FontMetricsPolicy::Raw,
                baseline_policy: BaselinePolicy::Alphabetic,
                baseline_class: BaselineClass::Roman,
                metric_box: MetricBox::RawFontBox,
                source: input.raw_metrics.source,
                reason: format!(
                    "ScriptAwareFontMetricsNormalizer:{:?}:fallback-raw",
                    request.role
                ),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselineClass {
    Roman,
    IdeographicCentered,
    IdeographicLow,
    Math,
    Hanging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricBox {
    RawFontBox,
    IdeographicEmBox,
    IdeographicCharacterFace,
    SampledInkBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontMetricSource {
    RawTables,
    OpenTypeBase,
    GlyphSampling,
    ManualOverride,
    SynthesizedIdeographicBox,
}
