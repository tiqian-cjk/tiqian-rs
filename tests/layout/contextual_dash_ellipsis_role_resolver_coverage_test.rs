use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TextStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::font::font_policy::{FontRole, FontRoleContext};
use tiqian::layout::contextual_dash_ellipsis_role_resolver::ContextualDashEllipsisRoleResolver;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn resolve(text: &str, locale: &str) -> Vec<tiqian::layout::contextual_dash_ellipsis_role_resolver::DashEllipsisRoleDecision> {
    ContextualDashEllipsisRoleResolver.resolve(
        &Text::from(text),
        &FontRoleContext::with_locale(locale.to_owned()),
    )
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
    for (text, mark, role) in [
        ("English — next", '—', FontRole::LatinText),
        ("— English", '—', FontRole::LatinText),
        ("A——B", '—', FontRole::LatinText),
        ("Wait…what", '…', FontRole::LatinText),
        ("Wait……what", '…', FontRole::LatinText),
        ("中文—下句", '—', FontRole::CjkPunctuation),
        ("中文——下句", '—', FontRole::CjkPunctuation),
        ("中文—123", '—', FontRole::CjkPunctuation),
        ("123—English", '—', FontRole::LatinText),
        ("中文……", '…', FontRole::CjkPunctuation),
        ("等等…真的", '…', FontRole::CjkPunctuation),
        ("等等……真的", '…', FontRole::CjkPunctuation),
    ] {
        let decision = resolve(text, "zh-Hans").pop().unwrap();
        assert_eq!(role, decision.role, "{text}");
        assert_eq!(
            text.chars().position(|character| character == mark).unwrap() as i32,
            decision.range.start(),
            "{text}",
        );
        assert_eq!(
            text.chars()
                .enumerate()
                .filter_map(|(index, character)| (character == mark).then_some(index))
                .last()
                .unwrap() as i32
                + 1,
            decision.range.end(),
            "{text}",
        );
        assert_eq!("DashEllipsisSurroundingScriptContext", decision.source, "{text}");
    }
}

#[test]
fn conflicting_or_absent_script_falls_back_to_paragraph_language() {
    for (locale, role) in [("zh-Hans", FontRole::CjkPunctuation), ("en-US", FontRole::LatinText)] {
        for text in ["中文—English", "…"] {
            let decision = resolve(text, locale).pop().unwrap();
            assert_eq!(role, decision.role, "{locale}: {text}");
            assert_eq!("ParagraphLanguageDashEllipsisContext", decision.source, "{locale}: {text}");
        }
    }
}

#[test]
fn decision_reason_names_the_evidence_shape() {
    assert_eq!("matching-surrounding-script", resolve("A—B", "zh-Hans")[0].reason);
    assert_eq!("only-left-strong-script", resolve("中文……", "zh-Hans")[0].reason);
    assert_eq!("only-right-strong-script", resolve("— English", "zh-Hans")[0].reason);
}

#[test]
fn mandatory_break_stops_context_search() {
    let decision = resolve("—\nEnglish", "zh-Hans").pop().unwrap();
    assert_eq!(FontRole::CjkPunctuation, decision.role);
    assert_eq!("ParagraphLanguageDashEllipsisContext", decision.source);
    assert!(decision.reason.starts_with("no-strong-script-context"));
}

#[test]
fn linear_context_index_preserves_supplementary_script_evidence() {
    assert_eq!(FontRole::CjkPunctuation, resolve("𠀀—123", "zh-Hans")[0].role);
    assert_eq!(FontRole::LatinText, resolve("123—𐐀", "zh-Hans")[0].role);
}

#[test]
fn resolves_many_neutral_separated_runs_from_one_paragraph_index() {
    let text = format!("A{}B", " — ".repeat(2_048));
    let decisions = resolve(&text, "zh-Hans");
    assert_eq!(2_048, decisions.len());
    assert!(decisions.iter().all(|decision| decision.role == FontRole::LatinText));
}

#[test]
fn pairs_parenthetical_dashes_across_inserted_content() {
    let decisions = resolve("他彻夜想Jessica——Jessica是他的前女友——睡不着觉", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "ParagraphLanguageDashEllipsisContext"
            && decision.reason.starts_with("parenthetical-pair-conflicting-outer-script")
    }));
}

#[test]
fn matching_outer_script_resolves_the_parenthetical_pair_directly() {
    let decisions = resolve("word——and stuff——word", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::LatinText && decision.source == "ParentheticalDashPairContext"
    }));
}

#[test]
fn punctuation_between_runs_keeps_them_independent() {
    let decisions = resolve("地点——北京，时间——明天", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.role == FontRole::CjkPunctuation
            && decision.source == "DashEllipsisSurroundingScriptContext"
    }));
}

#[test]
fn symbol_between_runs_keeps_them_independent() {
    let decisions = resolve("时价——$100——很贵", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| {
        decision.source == "DashEllipsisSurroundingScriptContext"
    }));
}

#[test]
fn unequal_run_lengths_do_not_pair() {
    let decisions = resolve("想Jessica——Jessica是前女友—睡不着", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert_eq!(FontRole::LatinText, decisions[0].role);
    assert_eq!(FontRole::CjkPunctuation, decisions[1].role);
}

#[test]
fn ellipsis_runs_never_pair() {
    let decisions = resolve("想Jessica……Jessica是他的前女友……睡不着", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert_eq!(FontRole::LatinText, decisions[0].role);
    assert_eq!(FontRole::CjkPunctuation, decisions[1].role);
}

#[test]
fn mandatory_break_between_runs_keeps_them_independent() {
    let decisions = resolve("想Jessica——Jessica\n是前女友——睡不着", "zh-Hans");
    assert_eq!(2, decisions.len());
    assert_eq!(FontRole::LatinText, decisions[0].role);
    assert_eq!(FontRole::CjkPunctuation, decisions[1].role);
}

#[test]
fn western_context_keeps_dash_and_ellipsis_on_latin_face_and_preserves_source_display() {
    let text = "English — next; ellipsis… / slash. A——B; Wait……what?";
    let result = layout(text, "zh-Hans");
    for decision in result.debug.font_decisions.iter().filter(|decision| {
        decision.source_text.contains('—') || decision.source_text.contains('…')
    }) {
        assert_eq!("LatinText", decision.role);
        assert_eq!(decision.source_text, decision.display_text);
    }
    assert_eq!("——", result.debug.font_decisions.iter().find(|d| d.source_text == "——").unwrap().source_text);
    assert_eq!("……", result.debug.font_decisions.iter().find(|d| d.source_text == "……").unwrap().source_text);
    assert!(result.debug.punctuation_decisions.iter().all(|decision| !matches!(decision.ch, '—' | '…')));
    assert!(result.debug.role_overrides.iter().filter(|decision| {
        decision.source_text.contains('—') || decision.source_text.contains('…')
    }).all(|decision| decision.source == "DashEllipsisSurroundingScriptContext"));
}

#[test]
fn cjk_context_keeps_clreq_display_substitution_independent_of_mark_count() {
    let result = layout("中—文，等…真；中文——下句，省略号……。", "zh-Hans");
    for (source, display) in [("—", "—"), ("…", "⋯"), ("——", "⸺"), ("……", "⋯⋯")] {
        let decision = result.debug.font_decisions.iter().find(|decision| decision.source_text == source).unwrap();
        assert_eq!(display, decision.display_text);
    }
    assert!(result.debug.font_decisions.iter().filter(|decision| {
        decision.source_text.contains('—') || decision.source_text.contains('…')
    }).all(|decision| decision.role == "CjkPunctuation"));
}

#[test]
fn parenthetical_pair_shares_one_face_and_substitution() {
    let result = layout("他彻夜想Jessica——Jessica是他的前女友——睡不着觉", "zh-Hans");
    let decisions: Vec<_> = result.debug.font_decisions.iter().filter(|decision| decision.source_text == "——").collect();
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| decision.role == "CjkPunctuation" && decision.display_text == "⸺"));
}

#[test]
fn standalone_western_ellipsis_cannot_be_rewritten_by_the_substitutor() {
    let result = layout("…", "en-US");
    let decision = result.debug.font_decisions.first().unwrap();
    assert_eq!("LatinText", decision.role);
    assert_eq!("…", decision.source_text);
    assert_eq!("…", decision.display_text);
    assert_eq!("CjkRoleGatedDisplaySubstitution:preserve-role-LatinText", decision.substitution_reason);
    assert_eq!("…", result.clusters.first().unwrap().display_text);
}
