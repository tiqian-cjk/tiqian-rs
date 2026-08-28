use tiqian::org::tiqian::clreq::ClreqProfile::{
    CjkPunctuationGlyphPolicy, ClreqPunctuationGlyphSubstitutor, clreq_punctuation_advance_policy,
    clreq_punctuation_policies,
};

#[test]
fn prefer_policy_uses_clreq_recommended_display_codepoints() {
    let substitutor = ClreqPunctuationGlyphSubstitutor::new(
        CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints,
    );
    assert_eq!("⋯⋯", substitutor.substitute("……").display_text);
    assert_eq!("⸺", substitutor.substitute("——").display_text);
    assert_eq!("·", substitutor.substitute("・").display_text);
    assert_eq!("·", substitutor.substitute("‧").display_text);
    assert_eq!("·", substitutor.substitute("•").display_text);
}

#[test]
fn preserve_policy_keeps_input_display_codepoints() {
    let substitutor =
        ClreqPunctuationGlyphSubstitutor::new(CjkPunctuationGlyphPolicy::PreserveInput);
    assert_eq!("……", substitutor.substitute("……").display_text);
    assert_eq!("——", substitutor.substitute("——").display_text);
    assert_eq!("・", substitutor.substitute("・").display_text);
}

#[test]
fn prefer_policy_does_not_rewrite_ambiguous_connector_or_solidus_forms() {
    let substitutor = ClreqPunctuationGlyphSubstitutor::new(
        CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints,
    );
    for text in ["～", "-", "/", "／", "．"] {
        assert_eq!(text, substitutor.substitute(text).display_text);
    }
}

#[test]
fn recommended_dash_codepoint_occupies_two_em() {
    let policy = clreq_punctuation_policies::policy_for('⸺');
    assert_eq!(2.0, policy.default_body_em);
    assert_eq!(2.0, policy.default_advance_em);
    assert_eq!(2.0, clreq_punctuation_advance_policy::advance_em("⸺", "⸺"));
    assert_eq!(2.0, clreq_punctuation_advance_policy::advance_em("——", "⸺"));
}
