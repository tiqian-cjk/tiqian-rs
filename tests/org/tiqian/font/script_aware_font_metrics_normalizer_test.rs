use tiqian::font::font_metrics::FontMetricsRequest;
use tiqian::font::font_metrics::{
    BaselineClass, FontMetricSource, FontMetricsNormalizationInput, FontMetricsNormalizer,
    FontMetricsResolver, MetricBox, ScriptAwareFontMetricsNormalizer, StubFontMetricsResolver,
};
use tiqian::font::font_policy::{FontMetricsPolicy, FontRole, RawFontMetrics};

fn request(font_key: &str, role: FontRole, locale: &str) -> FontMetricsRequest {
    FontMetricsRequest::new(font_key.to_owned(), 16.0, role, locale.to_owned())
}

#[test]
fn cjk_text_uses_font_declared_typo_box_instead_of_synthesized_square() {
    let request = request("cjk-primary", FontRole::CjkText, "zh-Hans");
    let raw = StubFontMetricsResolver.resolve(&request);
    let layout = ScriptAwareFontMetricsNormalizer.normalize(&FontMetricsNormalizationInput {
        request,
        raw_metrics: raw.clone(),
    });

    assert_eq!(Some(14.08), raw.typo_ascent);
    assert_eq!(Some(1.92), raw.typo_descent);
    assert_eq!(14.08, layout.ascent);
    assert_eq!(1.92, layout.descent);
    assert_eq!(BaselineClass::IdeographicLow, layout.baseline_class);
    assert_eq!(MetricBox::IdeographicEmBox, layout.metric_box);
    assert_eq!(FontMetricSource::RawTables, layout.source);
}

#[test]
fn cjk_text_falls_back_to_hhea_when_font_has_no_typo_metrics() {
    let request = request("cjk-bad", FontRole::CjkText, "zh-Hans");
    let raw = RawFontMetrics::new(18.4, 4.0);
    let layout = ScriptAwareFontMetricsNormalizer.normalize(&FontMetricsNormalizationInput {
        request,
        raw_metrics: raw,
    });

    assert_eq!(18.4, layout.ascent);
    assert_eq!(4.0, layout.descent);
    assert_eq!(FontMetricsPolicy::Raw, layout.policy);
}

#[test]
fn latin_text_keeps_roman_raw_metrics() {
    let request = request("latin-primary", FontRole::LatinText, "en");
    let raw = StubFontMetricsResolver.resolve(&request);
    let layout = ScriptAwareFontMetricsNormalizer.normalize(&FontMetricsNormalizationInput {
        request,
        raw_metrics: raw.clone(),
    });

    assert_eq!(raw.ascent, layout.ascent);
    assert_eq!(raw.descent, layout.descent);
    assert_eq!(BaselineClass::Roman, layout.baseline_class);
    assert_eq!(MetricBox::RawFontBox, layout.metric_box);
}
