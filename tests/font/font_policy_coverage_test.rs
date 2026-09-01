use tiqian::core::geometry::TextRange;
use tiqian::core::text::Text;
use tiqian::font::font_metrics::{
    BaselineClass, FontMetricSource, FontMetricsNormalizationInput, FontMetricsNormalizer,
    FontMetricsRequest, FontMetricsResolver, MetricBox, ScriptAwareFontMetricsNormalizer,
    StubFontMetricsResolver,
};
use tiqian::font::font_policy::{
    BaselinePolicy, FallbackResolver, FontCandidate, FontDecision, FontMetricsPolicy, FontRequest,
    FontRole, FontRoleContext, LayoutFontMetrics, PreferCjkForAmbiguousPunctuationResolver,
    RawFontMetrics, font_role_name_uses_latin_face,
};

#[test]
fn test_font_request_and_roles() {
    let request = FontRequest {
        preferred_families: vec!["Source Han Sans".to_owned()],
        locale: "zh-Hans".to_owned(),
        role: FontRole::CjkText,
    };
    assert_eq!(vec!["Source Han Sans"], request.preferred_families);
    assert_eq!("zh-Hans", request.locale);
    assert_eq!(FontRole::CjkText, request.role);

    assert!(FontRole::LatinText.uses_latin_face());
    for role in [
        FontRole::CjkText,
        FontRole::CjkPunctuation,
        FontRole::Symbol,
        FontRole::Emoji,
        FontRole::Unknown,
    ] {
        assert!(!role.uses_latin_face(), "{role:?}");
    }

    assert!(font_role_name_uses_latin_face(Some("LatinText")));
    for name in [Some("CjkText"), Some("Unknown"), None, Some("NotARole")] {
        assert!(!font_role_name_uses_latin_face(name));
    }

    let candidate = FontCandidate {
        key: "cjk-key".to_owned(),
        family: "Source Han Sans".to_owned(),
        role: FontRole::CjkText,
    };
    assert_eq!("cjk-key", candidate.key);
    assert_eq!("Source Han Sans", candidate.family);
    assert_eq!(FontRole::CjkText, candidate.role);

    let decision = FontDecision {
        range: TextRange::new(0, 1),
        candidate: candidate.clone(),
        role: FontRole::CjkText,
        reason: "reason".to_owned(),
    };
    assert_eq!(TextRange::new(0, 1), decision.range);
    assert_eq!(candidate, decision.candidate);
    assert_eq!(FontRole::CjkText, decision.role);
    assert_eq!("reason", decision.reason);

    let context = FontRoleContext::new("zh-TW".to_owned(), Some("TW".to_owned()));
    assert_eq!("zh-TW", context.locale);
    assert_eq!(Some("TW".to_owned()), context.region_hint);
}

#[test]
fn test_prefer_cjk_for_ambiguous_punctuation_resolver() {
    let resolver = PreferCjkForAmbiguousPunctuationResolver::new(
        "cjk-key".to_owned(),
        "latin-key".to_owned(),
        "symbol-key".to_owned(),
    );

    let cjk_decision = resolver.resolve(
        &Text::from("中"),
        TextRange::new(0, 1),
        &FontRequest {
            preferred_families: vec!["CustomCjk".to_owned()],
            locale: "zh-Hans".to_owned(),
            role: FontRole::CjkText,
        },
    );
    assert_eq!("cjk-key", cjk_decision.candidate.key);
    assert_eq!("CustomCjk", cjk_decision.candidate.family);

    let cjk_default_family = resolver.resolve(
        &Text::from("中"),
        TextRange::new(0, 1),
        &FontRequest {
            preferred_families: Vec::new(),
            locale: "zh-Hans".to_owned(),
            role: FontRole::CjkPunctuation,
        },
    );
    assert_eq!("cjk-key", cjk_default_family.candidate.family);

    for (text, range, role, expected_key) in [
        ("A", TextRange::new(0, 1), FontRole::LatinText, "latin-key"),
        ("©", TextRange::new(0, 1), FontRole::Symbol, "symbol-key"),
        ("😀", TextRange::new(0, 2), FontRole::Emoji, "symbol-key"),
        ("\u{0001}", TextRange::new(0, 1), FontRole::Unknown, "symbol-key"),
    ] {
        let decision = resolver.resolve(
            &Text::from(text),
            range,
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "en".to_owned(),
                role,
            },
        );
        assert_eq!(expected_key, decision.candidate.key, "{role:?}");
    }
}

#[test]
fn test_font_enums_and_models() {
    let raw_metrics = RawFontMetrics::builder(16.0, 4.0)
        .leading(2.0)
        .source(FontMetricSource::RawTables)
        .typo_ascent(Some(14.0))
        .typo_descent(Some(2.0))
        .build();
    assert_eq!(16.0, raw_metrics.ascent);
    assert_eq!(4.0, raw_metrics.descent);
    assert_eq!(2.0, raw_metrics.leading);
    assert_eq!(Some(14.0), raw_metrics.typo_ascent);
    assert_eq!(Some(2.0), raw_metrics.typo_descent);

    let layout_metrics = LayoutFontMetrics::builder(
        14.0,
        2.0,
        0.0,
        FontMetricsPolicy::IdeographicBox,
        BaselinePolicy::Ideographic,
    )
    .baseline_class(BaselineClass::IdeographicLow)
    .metric_box(MetricBox::IdeographicEmBox)
    .source(FontMetricSource::RawTables)
    .reason("test".to_owned())
    .build();
    assert_eq!(14.0, layout_metrics.ascent);
    assert_eq!(2.0, layout_metrics.descent);
    assert_eq!(0.0, layout_metrics.baseline_offset);
    assert_eq!(FontMetricsPolicy::IdeographicBox, layout_metrics.policy);
    assert_eq!(BaselinePolicy::Ideographic, layout_metrics.baseline_policy);
    assert_eq!(BaselineClass::IdeographicLow, layout_metrics.baseline_class);
    assert_eq!(MetricBox::IdeographicEmBox, layout_metrics.metric_box);
    assert_eq!(FontMetricSource::RawTables, layout_metrics.source);
    assert_eq!("test", layout_metrics.reason);
}

#[test]
fn test_font_metrics_request_and_resolvers() {
    let request = FontMetricsRequest::builder(
        "key1".to_owned(),
        16.0,
        FontRole::CjkText,
        "zh-Hans".to_owned(),
    )
    .font_families(vec!["FontA".to_owned()])
    .font_weight(700)
    .italic(true)
    .face_selection_text(Text::from("测试"))
    .build();
    assert_eq!("key1", request.font_key);
    assert_eq!(16.0, request.font_size);
    assert_eq!(FontRole::CjkText, request.role);
    assert_eq!("zh-Hans", request.locale);
    assert_eq!(vec!["FontA"], request.font_families);
    assert_eq!(700, request.font_weight);
    assert!(request.italic);
    assert_eq!(Text::from("测试"), request.face_selection_text);

    let resolver = StubFontMetricsResolver;
    let cjk_raw = resolver.resolve(&request);
    assert_eq!(16.0 * 1.16, cjk_raw.ascent);
    assert_eq!(Some(16.0 * 0.88), cjk_raw.typo_ascent);

    for (role, expected_ascent) in [
        (FontRole::CjkPunctuation, 16.0 * 1.16),
        (FontRole::LatinText, 16.0 * 0.8),
        (FontRole::Symbol, 16.0 * 0.9),
        (FontRole::Emoji, 16.0 * 0.9),
        (FontRole::Unknown, 16.0 * 0.9),
    ] {
        let raw = resolver.resolve(&FontMetricsRequest {
            role,
            ..request.clone()
        });
        assert_eq!(expected_ascent, raw.ascent, "{role:?}");
    }
}

#[test]
fn test_script_aware_font_metrics_normalizer_branches() {
    let normalizer = ScriptAwareFontMetricsNormalizer;
    let base_request =
        FontMetricsRequest::new("key".to_owned(), 16.0, FontRole::CjkText, "zh-Hans".to_owned());

    let with_typo = normalizer.normalize(&FontMetricsNormalizationInput {
        request: base_request.clone(),
        raw_metrics: RawFontMetrics::builder(18.0, 5.0)
            .typo_ascent(Some(14.0))
            .typo_descent(Some(2.0))
            .build(),
    });
    assert_eq!(14.0, with_typo.ascent);
    assert_eq!(2.0, with_typo.descent);
    assert_eq!(FontMetricsPolicy::IdeographicBox, with_typo.policy);
    assert!(with_typo.reason.contains("font-typo-box"));

    let only_typo_ascent = normalizer.normalize(&FontMetricsNormalizationInput {
        request: base_request.clone(),
        raw_metrics: RawFontMetrics::builder(18.0, 5.0)
            .typo_ascent(Some(14.0))
            .build(),
    });
    assert_eq!(14.0, only_typo_ascent.ascent);
    assert_eq!(5.0, only_typo_ascent.descent);
    assert_eq!(FontMetricsPolicy::Raw, only_typo_ascent.policy);
    assert!(only_typo_ascent.reason.contains("hhea-fallback-no-os2"));

    let only_typo_descent = normalizer.normalize(&FontMetricsNormalizationInput {
        request: base_request.clone(),
        raw_metrics: RawFontMetrics::builder(18.0, 5.0)
            .typo_descent(Some(2.0))
            .build(),
    });
    assert_eq!(18.0, only_typo_descent.ascent);
    assert_eq!(2.0, only_typo_descent.descent);
    assert_eq!(FontMetricsPolicy::Raw, only_typo_descent.policy);

    let no_typo = normalizer.normalize(&FontMetricsNormalizationInput {
        request: base_request.clone(),
        raw_metrics: RawFontMetrics::new(18.0, 5.0),
    });
    assert_eq!(18.0, no_typo.ascent);
    assert_eq!(5.0, no_typo.descent);
    assert_eq!(FontMetricsPolicy::Raw, no_typo.policy);

    let latin = normalizer.normalize(&FontMetricsNormalizationInput {
        request: FontMetricsRequest {
            role: FontRole::LatinText,
            ..base_request.clone()
        },
        raw_metrics: RawFontMetrics::new(13.0, 3.0),
    });
    assert_eq!(13.0, latin.ascent);
    assert_eq!(3.0, latin.descent);
    assert_eq!(FontMetricsPolicy::Raw, latin.policy);
    assert_eq!(BaselinePolicy::Alphabetic, latin.baseline_policy);

    let symbol = normalizer.normalize(&FontMetricsNormalizationInput {
        request: FontMetricsRequest {
            role: FontRole::Symbol,
            ..base_request
        },
        raw_metrics: RawFontMetrics::new(14.0, 4.0),
    });
    assert_eq!(14.0, symbol.ascent);
    assert_eq!(FontMetricsPolicy::Raw, symbol.policy);
}