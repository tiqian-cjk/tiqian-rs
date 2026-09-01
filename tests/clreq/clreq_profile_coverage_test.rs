use tiqian::clreq::bopomofo_reading::{BopomofoReading, BopomofoTone, bopomofo_parser};
use tiqian::clreq::clreq_profile::{
    BuiltInClreqProfileResolver, ClreqProfile, ClreqProfileResolver, ClreqRegion,
    ClreqStrictness, CjkPunctuationGlyphPolicy, InteriorPunctuationStyle, KinsokuLevel,
    PunctuationClass, PunctuationGluePlacement, PunctuationWidthPolicy,
    clreq_punctuation_advance_policy, clreq_punctuation_policies,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutProfileId, built_in_layout_profiles};

#[test]
fn bopomofo_models_and_parser() {
    let reading = BopomofoReading { symbols: vec![Text::from("ㄅ"), Text::from("ㄚ")], tone: BopomofoTone::Yangping };
    assert_eq!(reading, reading.clone());
    assert!(format!("{reading:?}").contains("BopomofoReading"));
    assert_eq!(BopomofoTone::Yinping, bopomofo_parser::parse(&Text::new()).tone);
    for (text, tone) in [("˙ㄇㄚ", BopomofoTone::Neutral), ("ㄇㄚˊ", BopomofoTone::Yangping), ("ㄇㄚˇ", BopomofoTone::Shang), ("ㄇㄚˋ", BopomofoTone::Qu), ("ㄇㄚˉ", BopomofoTone::Yinping), ("ㄇㄚ", BopomofoTone::Yinping)] {
        assert_eq!(tone, bopomofo_parser::parse(&Text::from(text)).tone, "{text}");
    }
}

#[test]
fn clreq_profile_and_resolver() {
    assert!(ClreqProfile::mainland_horizontal().coalesce_repeatable_punctuation.contains(&0x2014));
    assert_eq!("clreq-mainland-horizontal", ClreqProfile::mainland_horizontal().id);
    assert_eq!("clreq-taiwan-horizontal", ClreqProfile::taiwan_horizontal().id);
    assert_eq!("clreq-hongkong-horizontal", ClreqProfile::hong_kong_horizontal().id);
    assert_eq!(PunctuationGluePlacement::MainlandSimplified, PunctuationGluePlacement::for_region(ClreqRegion::Mainland));
    assert_eq!(PunctuationGluePlacement::Traditional, PunctuationGluePlacement::for_region(ClreqRegion::Taiwan));
    assert_eq!(PunctuationGluePlacement::Traditional, PunctuationGluePlacement::for_region(ClreqRegion::HongKong));
    assert_eq!(PunctuationGluePlacement::MainlandSimplified, PunctuationGluePlacement::for_region(ClreqRegion::Custom));
    let resolver = BuiltInClreqProfileResolver;
    for id in [
        built_in_layout_profiles::clreq_horizontal(),
        LayoutProfileId { value: "clreq-mainland-horizontal".into() },
        LayoutProfileId { value: "other-profile".into() },
    ] {
        assert_eq!(ClreqProfile::mainland_horizontal(), resolver.resolve(&id));
    }
    assert_eq!(ClreqStrictness::Normal, ClreqProfile::mainland_horizontal().strictness);
    assert_eq!(CjkPunctuationGlyphPolicy::PreferClreqRecommendedCodepoints, ClreqProfile::mainland_horizontal().punctuation_glyph_policy);
}

#[test]
fn clreq_punctuation_policies_and_classification() {
    for character in [',', '.', ':', ';', '!', '?'] { assert!(clreq_punctuation_policies::is_ascii_point_mark(character)); }
    for character in ['a', '，'] { assert!(!clreq_punctuation_policies::is_ascii_point_mark(character)); }
    for character in ['“', '‘', '（', '《', '〈', '「', '『', '【', '〔', '〖', '〘', '〚'] { assert_eq!(PunctuationClass::Opening, clreq_punctuation_policies::classify(character)); }
    for character in ['”', '’', '）', '》', '〉', '」', '』', '】', '〕', '〗', '〙', '〛'] { assert_eq!(PunctuationClass::Closing, clreq_punctuation_policies::classify(character)); }
    for character in ['，', '、', '。', '；', '：', '！', '？'] { assert_eq!(PunctuationClass::PauseOrStop, clreq_punctuation_policies::classify(character)); }
    assert_eq!(PunctuationClass::MiddleDot, clreq_punctuation_policies::classify('·'));
    for character in ['・', '‧', '•'] { assert_eq!(PunctuationClass::Interpunct, clreq_punctuation_policies::classify(character)); }
    for character in ['～', '~', '-', '–'] { assert_eq!(PunctuationClass::Connector, clreq_punctuation_policies::classify(character)); }
    for character in ['/', '／'] { assert_eq!(PunctuationClass::Solidus, clreq_punctuation_policies::classify(character)); }
    for character in ['…', '⋯'] { assert_eq!(PunctuationClass::Ellipsis, clreq_punctuation_policies::classify(character)); }
    for character in ['—', '⸺'] { assert_eq!(PunctuationClass::Dash, clreq_punctuation_policies::classify(character)); }
    assert_eq!(PunctuationClass::Other, clreq_punctuation_policies::classify('中'));
}

#[test]
fn forced_half_width_and_policy_for() {
    assert!(clreq_punctuation_policies::forced_half_width('-', PunctuationWidthPolicy::default()));
    assert!(clreq_punctuation_policies::forced_half_width('–', PunctuationWidthPolicy::default()));
    let gb = PunctuationWidthPolicy::with_gb_fixed_separators(true);
    for character in ['～', '·', '•', '/'] { assert!(clreq_punctuation_policies::forced_half_width(character, gb)); }
    assert!(!clreq_punctuation_policies::forced_half_width('，', gb));
    let kaiming = PunctuationWidthPolicy::with_interior(InteriorPunctuationStyle::Kaiming);
    for character in ['（', '）', '，', '；'] { assert!(clreq_punctuation_policies::forced_half_width(character, kaiming)); }
    for character in ['。', '！', '？', '．', '中'] { assert!(!clreq_punctuation_policies::forced_half_width(character, kaiming)); }
    for (character, body, advance) in [('⸺', 2.0, 2.0), ('-', 0.5, 0.5), ('，', 0.5, 1.0), ('（', 0.5, 1.0), ('）', 0.5, 1.0), ('字', 1.0, 1.0)] {
        let policy = clreq_punctuation_policies::policy_for(character);
        assert_eq!(body, policy.default_body_em, "{character}");
        assert_eq!(advance, policy.default_advance_em, "{character}");
    }
}

#[test]
fn forbidden_at_line_start_and_end() {
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('，', KinsokuLevel::None));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('（', KinsokuLevel::None));
    for character in ['，', '）', '～', '·', '•', '/'] { assert!(clreq_punctuation_policies::forbidden_at_line_start(character, KinsokuLevel::Basic)); }
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('—', KinsokuLevel::Basic));
    assert!(clreq_punctuation_policies::forbidden_at_line_start('—', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('…', KinsokuLevel::Basic));
    assert!(clreq_punctuation_policies::forbidden_at_line_start('…', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('（', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('字', KinsokuLevel::Strict));
    assert!(clreq_punctuation_policies::forbidden_at_line_end('（', KinsokuLevel::Basic));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('/', KinsokuLevel::Basic));
    assert!(clreq_punctuation_policies::forbidden_at_line_end('/', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('）', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('字', KinsokuLevel::Strict));
}

#[test]
fn punctuation_advance_and_substitutor() {
    assert_eq!(2.0, clreq_punctuation_advance_policy::advance_em(&Text::from("⸺"), &Text::from("⸺")));
    assert_eq!(2.0, clreq_punctuation_advance_policy::advance_em(&Text::from("—"), &Text::from("⸺")));
    assert_eq!(2.0, clreq_punctuation_advance_policy::advance_em(&Text::from("⸺"), &Text::from("——")));
    assert_eq!(3.0, clreq_punctuation_advance_policy::advance_em(&Text::from("abc"), &Text::from("abc")));
    assert_eq!(1.0, clreq_punctuation_advance_policy::advance_em(&Text::from("😀"), &Text::from("dummy")));
    let force = tiqian::clreq::clreq_profile::ClreqPunctuationGlyphSubstitutor::new(CjkPunctuationGlyphPolicy::ForceClreqRecommendedCodepoints);
    let unchanged = force.substitute(&Text::from("abc"));
    assert_eq!("abc", unchanged.display_text.as_str());
    assert!(unchanged.reason.contains("preserve"));
}