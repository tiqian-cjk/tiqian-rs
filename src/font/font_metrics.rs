// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/font/FontMetrics.kt

use super::super::core::font_face::FontFaceId;
use super::font_policy::{
    BaselinePolicy, FontMetricsPolicy, FontRole, LayoutFontMetrics, RawFontMetrics,
};

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetricsRequest {
    pub face: FontFaceId,
    pub font_size: f32,
    pub role: FontRole,
    pub locale: String,
}

impl FontMetricsRequest {
    /// 创建一个已经由完整 shaping 选定字体面的 metrics 请求。
    pub fn new(face: FontFaceId, font_size: f32, role: FontRole, locale: String) -> Self {
        Self {
            face,
            font_size,
            role,
            locale,
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
