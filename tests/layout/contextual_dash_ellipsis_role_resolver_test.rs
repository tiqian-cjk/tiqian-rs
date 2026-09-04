use tiqian::common::HashSet;
use tiqian::clreq::clreq_profile::{CjkPunctuationGlyphPolicy, ClreqProfile, ClreqProfileResolver};
use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::font::font_policy::{CjkFontRoleClassifier, FontRole, FontRoleClassifier, FontRoleContext};
use tiqian::layout::contextual_dash_ellipsis_role_resolver::{
    with_contextual_dash_ellipsis_roles, ContextualDashEllipsisFontRoleClassifier,
    ContextualDashEllipsisRoleResolver,
};
use tiqian::layout::quote_pair_analyzer::{
    with_contextual_quote_roles, ContextualQuoteFontRoleClassifier,
};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct SplitDashProfile;

impl ClreqProfileResolver for SplitDashProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile.coalesce_repeatable_punctuation = HashSet::new();
        profile
    }
}

fn layout(text: &str, locale: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(1_000.0),
        )
        .text_style(TextStyle::builder().locale(locale.to_owned()).build())
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

#[test]
fn resolves_by_surrounding_script_rather_than_mark_count() {
    let resolver = ContextualDashEllipsisRoleResolver;
    for (text, role) in [
        ("English — next", FontRole::LatinText),
        ("— English", FontRole::LatinText),
        ("A——B", FontRole::LatinText),
        ("Wait……what", FontRole::LatinText),
        ("中文—下句", FontRole::CjkPunctuation),
        ("中文——下句", FontRole::CjkPunctuation),
        ("中文……", FontRole::CjkPunctuation),
    ] {
        let decisions = resolver.resolve(&Text::from(text), &FontRoleContext::default());
        assert_eq!(1, decisions.len(), "{text}");
        assert_eq!(role, decisions[0].role, "{text}");
        assert_eq!("DashEllipsisSurroundingScriptContext", decisions[0].source, "{text}");
    }
}

#[test]
fn conflicting_or_absent_script_falls_back_to_paragraph_language() {
    let resolver = ContextualDashEllipsisRoleResolver;
    for (locale, role) in [("zh-Hans", FontRole::CjkPunctuation), ("en-US", FontRole::LatinText)] {
        for text in ["中文—English", "…"] {
            let decisions = resolver.resolve(
                &Text::from(text),
                &FontRoleContext::with_locale(locale.to_owned()),
            );
            assert_eq!(role, decisions[0].role, "{locale}: {text}");
            assert_eq!("ParagraphLanguageDashEllipsisContext", decisions[0].source);
        }
    }
}

#[test]
fn mandatory_break_stops_context_search_and_preserves_supplementary_evidence() {
    let resolver = ContextualDashEllipsisRoleResolver;
    let break_decision = resolver.resolve(
        &Text::from("—\nEnglish"),
        &FontRoleContext::with_locale("zh-Hans".to_owned()),
    );
    assert_eq!(FontRole::CjkPunctuation, break_decision[0].role);
    assert_eq!("ParagraphLanguageDashEllipsisContext", break_decision[0].source);
    assert!(break_decision[0].reason.starts_with("no-strong-script-context"));

    assert_eq!(
        FontRole::CjkPunctuation,
        resolver
            .resolve(&Text::from("𠀀—123"), &FontRoleContext::default())[0]
            .role,
    );
    assert_eq!(
        FontRole::LatinText,
        resolver
            .resolve(&Text::from("123—𐐀"), &FontRoleContext::default())[0]
            .role,
    );
}

#[test]
fn parenthetical_dash_pairs_resolve_from_outer_script_only() {
    let resolver = ContextualDashEllipsisRoleResolver;
    let cjk = resolver.resolve(
        &Text::from("他彻夜想Jessica——Jessica是他的前女友——睡不着觉"),
        &FontRoleContext::default(),
    );
    assert_eq!(2, cjk.len());
    assert!(cjk.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "ParagraphLanguageDashEllipsisContext"
            && decision
                .reason
                .starts_with("parenthetical-pair-conflicting-outer-script")
    }));

    let latin = resolver.resolve(&Text::from("word——and stuff——word"), &FontRoleContext::default());
    assert!(latin.iter().all(|decision| {
        decision.role == FontRole::LatinText
            && decision.source == "ParentheticalDashPairContext"
            && decision.reason == "matching-outer-script"
    }));

    let independent = resolver.resolve(
        &Text::from("地点——北京，时间——明天"),
        &FontRoleContext::default(),
    );
    assert!(independent
        .iter()
        .all(|decision| decision.source == "DashEllipsisSurroundingScriptContext"));
}

#[test]
fn parenthetical_pair_with_only_left_outer_script_takes_the_left_role() {
    let decisions = ContextualDashEllipsisRoleResolver.resolve(
        &Text::from("中文——word——"),
        &FontRoleContext::with_locale("zh-Hans".to_owned()),
    );
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "ParentheticalDashPairContext"
            && decision.reason.starts_with("only-left-outer-script")
    }));
}

#[test]
fn parenthetical_pair_with_only_right_outer_script_takes_the_right_role() {
    let decisions = ContextualDashEllipsisRoleResolver.resolve(
        &Text::from("——word——中文"),
        &FontRoleContext::with_locale("zh-Hans".to_owned()),
    );
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "ParentheticalDashPairContext"
            && decision.reason.starts_with("only-right-outer-script")
    }));
}

#[test]
fn parenthetical_pair_without_outer_script_falls_back_to_paragraph_language() {
    let decisions = ContextualDashEllipsisRoleResolver.resolve(
        &Text::from("——word——"),
        &FontRoleContext::with_locale("zh-Hans".to_owned()),
    );
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "ParagraphLanguageDashEllipsisContext"
            && decision
                .reason
                .starts_with("parenthetical-pair-no-outer-context")
    }));
}

#[test]
fn contextual_role_extensions_wrap_outside_the_pipeline() {
    let base = CjkFontRoleClassifier;
    let context = FontRoleContext::with_locale("zh-Hans".to_owned());
    let plain = Text::from("中文");

    assert!(matches!(
        with_contextual_dash_ellipsis_roles(&base, &plain, &context),
        ContextualDashEllipsisFontRoleClassifier::Passthrough(_)
    ));
    assert!(matches!(
        with_contextual_quote_roles(&base, &plain, &context),
        ContextualQuoteFontRoleClassifier::Passthrough(_)
    ));
    assert!(matches!(
        with_contextual_dash_ellipsis_roles(&base, &plain, &FontRoleContext::default()),
        ContextualDashEllipsisFontRoleClassifier::Passthrough(_)
    ));
    assert!(matches!(
        with_contextual_quote_roles(&base, &plain, &FontRoleContext::default()),
        ContextualQuoteFontRoleClassifier::Passthrough(_)
    ));

    let dash_text = Text::from("中文—English");
    let dash_aware = with_contextual_dash_ellipsis_roles(&base, &dash_text, &context);
    assert_eq!(
        FontRole::CjkPunctuation,
        dash_aware.classify(&dash_text, text_range(2, 3), &context)
    );
    assert_eq!(
        base.classify(&dash_text, text_range(0, 1), &context),
        dash_aware.classify(&dash_text, text_range(0, 1), &context)
    );

    let quote_text = Text::from("中a“b”c文");
    let quote_aware = with_contextual_quote_roles(&base, &quote_text, &context);
    assert_eq!(
        FontRole::LatinText,
        quote_aware.classify(&quote_text, text_range(2, 3), &context)
    );
}

#[test]
fn layout_uses_final_role_for_cluster_display_and_debug() {
    let western = layout("English — next; A——B; Wait……what?", "zh-Hans");
    let western_marks: Vec<_> = western
        .debug
        .font_decisions
        .iter()
        .filter(|decision| decision.source_text.as_str().contains('—') || decision.source_text.as_str().contains('…'))
        .collect();
    assert_eq!(3, western_marks.len());
    assert!(western_marks.iter().all(|decision| {
        decision.role == "LatinText"
            && decision.source_text == decision.display_text
    }));
    assert_eq!(
        vec!["—", "——", "……"],
        western_marks
            .iter()
            .map(|decision| decision.source_text.as_str())
            .collect::<Vec<_>>(),
    );
    assert!(western_marks
        .iter()
        .filter(|decision| matches!(decision.source_text.as_str(), "——" | "……"))
        .all(|decision| {
            decision.substitution_reason
                == "CjkRoleGatedDisplaySubstitution:preserve-role-LatinText"
        }));
    assert!(western
        .debug
        .punctuation_decisions
        .iter()
        .all(|decision| !matches!(decision.ch, '—' | '…')));
    assert!(western
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| {
            override_info.source_text.as_str().contains('—') || override_info.source_text.as_str().contains('…')
        })
        .all(|override_info| override_info.source == "DashEllipsisSurroundingScriptContext"));

    let cjk = layout("中—文，等…真；中文——下句，省略号……。", "zh-Hans");
    let display_at = |source: &str| {
        cjk.debug
            .font_decisions
            .iter()
            .find(|decision| decision.source_text == source)
            .unwrap()
            .display_text
            .as_str()
    };
    assert_eq!("—", display_at("—"));
    assert_eq!("⋯", display_at("…"));
    assert_eq!("⸺", display_at("——"));
    assert_eq!("⋯⋯", display_at("……"));
}

#[test]
fn latin_dash_run_at_paragraph_end_stays_one_cluster() {
    let result = layout("End——", "zh-Hans");
    let decision = result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text.as_str().contains('—'))
        .unwrap();
    assert_eq!("——", decision.source_text);
    assert_eq!("LatinText", decision.role);
}

#[test]
fn style_span_inside_latin_dash_run_splits_the_cluster() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("A——B"))
                .spans(vec![TextSpan {
                    range: text_range(2, 3),
                    style: TextStyle::builder().font_weight(700).build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(1_000.0),
        )
        .text_style(TextStyle::builder().locale("zh-Hans".to_owned()).build())
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let dash_decisions: Vec<_> = result
        .debug
        .font_decisions
        .iter()
        .filter(|decision| decision.source_text == "—")
        .collect();
    assert_eq!(2, dash_decisions.len());
    assert!(dash_decisions
        .iter()
        .all(|decision| decision.role == "LatinText"));
}

#[test]
fn latin_dash_run_honors_profile_repeat_coalescing_and_style_boundaries() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(SplitDashProfile);
    let split = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("A——B")),
            LayoutConstraints::with_defaults(1_000.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    assert_eq!(
        vec!["A", "—", "—", "B"],
        split
            .clusters
            .iter()
            .map(|cluster| cluster.text.as_str())
            .collect::<Vec<_>>(),
    );

    let style_split = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("A——B"))
                .spans(vec![TextSpan {
                    range: text_range(2, 3),
                    style: TextStyle::builder().font_weight(700).build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(1_000.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    assert_eq!(
        2,
        style_split
            .debug
            .font_decisions
            .iter()
            .filter(|decision| decision.source_text == "—" && decision.role == "LatinText")
            .count(),
    );
}