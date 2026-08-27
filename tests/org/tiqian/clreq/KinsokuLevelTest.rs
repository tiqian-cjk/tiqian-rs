use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, HangingPunctuationStyle, KinsokuLevel, KinsokuMode, PunctuationClass,
    clreq_punctuation_policies,
};

fn start(character: char, level: KinsokuLevel) -> bool {
    clreq_punctuation_policies::forbidden_at_line_start(character, level)
}

fn end(character: char, level: KinsokuLevel) -> bool {
    clreq_punctuation_policies::forbidden_at_line_end(character, level)
}

#[test]
fn none_forbids_nothing() {
    for character in ['。', '，', '、', '”', '）', '·', '／', '—', '…', '“', '（'] {
        assert!(!start(character, KinsokuLevel::None), "{character} start@None");
        assert!(!end(character, KinsokuLevel::None), "{character} end@None");
    }
}

#[test]
fn basic_forbids_pause_stops_closing_connectors_at_start_and_opening_at_end() {
    for character in ['。', '，', '、', '：', '；', '！', '？', '”', '）', '】', '·', '～', '／'] {
        assert!(start(character, KinsokuLevel::Basic), "{character} start@Basic");
    }
    for character in ['“', '（', '《', '「', '【'] {
        assert!(end(character, KinsokuLevel::Basic), "{character} end@Basic");
    }
    assert!(!start('—', KinsokuLevel::Basic));
    assert!(!start('…', KinsokuLevel::Basic));
    assert!(!end('／', KinsokuLevel::Basic));
}

#[test]
fn gb_style_adds_separator_at_line_end() {
    assert!(!end('／', KinsokuLevel::Basic));
    assert!(end('／', KinsokuLevel::GbStyle));
    assert!(!start('—', KinsokuLevel::GbStyle));
    assert!(!start('…', KinsokuLevel::GbStyle));
}

#[test]
fn strict_adds_dash_and_ellipsis_at_line_start() {
    assert!(!start('—', KinsokuLevel::GbStyle));
    assert!(start('—', KinsokuLevel::Strict));
    assert!(start('…', KinsokuLevel::Strict));
    assert!(start('⋯', KinsokuLevel::Strict));
    assert!(end('／', KinsokuLevel::Strict));
}

#[test]
fn profile_defaults_to_measure_adaptive() {
    assert!(matches!(ClreqProfile::mainland_horizontal().kinsoku_mode, KinsokuMode::MeasureAdaptive { .. }));
}

#[test]
fn cjk_bracket_variants_classify_as_opening_and_closing() {
    for character in ['【', '〔', '〖', '〘', '〚'] {
        assert_eq!(PunctuationClass::Opening, clreq_punctuation_policies::classify(character), "{character}");
    }
    for character in ['】', '〕', '〗', '〙', '〛'] {
        assert_eq!(PunctuationClass::Closing, clreq_punctuation_policies::classify(character), "{character}");
    }
}

#[test]
fn exposes_unambiguous_ascii_point_marks_without_guessing_quotes_or_connectors() {
    for character in [',', '.', ':', ';', '!', '?'] {
        assert!(clreq_punctuation_policies::is_ascii_point_mark(character), "{character} point mark");
    }
    for character in ['"', '\'', '-', '/', '~', '%'] {
        assert!(!clreq_punctuation_policies::is_ascii_point_mark(character), "{character} excluded");
    }
}

#[test]
fn measure_adaptive_resolves_per_line_width() {
    let mode = KinsokuMode::measure_adaptive();
    let narrow = mode.resolve(10.0);
    assert_eq!(KinsokuLevel::Basic, narrow.level);
    assert_eq!(HangingPunctuationStyle::PauseStops, narrow.hanging);
    let normal = mode.resolve(20.0);
    assert_eq!(KinsokuLevel::Basic, normal.level);
    assert_eq!(HangingPunctuationStyle::Disabled, normal.hanging);
    assert_eq!(KinsokuLevel::GbStyle, mode.resolve(28.0).level);
    assert_eq!(KinsokuLevel::Strict, mode.resolve(40.0).level);
}
