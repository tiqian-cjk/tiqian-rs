use tiqian::core::geometry::scalar_offset;
use tiqian::core::text::Text;
use tiqian::linebreak::line_break::{
    BreakKind, LineBreakAnalyzer, SimpleCharacterLineBreakAnalyzer, is_mandatory_break_code_point,
    is_zero_width_space_code_point,
};

#[test]
fn recognizes_mandatory_break_code_points() {
    for code_point in [0x000A, 0x000B, 0x000C, 0x000D, 0x0085, 0x2028, 0x2029] {
        assert!(
            is_mandatory_break_code_point(code_point),
            "U+{code_point:04X}"
        );
    }
    for code_point in ['a' as i32, '中' as i32, ' ' as i32, '\t' as i32, 0x3000] {
        assert!(
            !is_mandatory_break_code_point(code_point),
            "U+{code_point:04X}"
        );
    }
}

#[test]
fn recognizes_zero_width_space_without_conflating_no_break_controls() {
    assert!(is_zero_width_space_code_point(0x200B));
    for code_point in [0x200C, 0x200D, 0x2060, 0xFEFF] {
        assert!(
            !is_zero_width_space_code_point(code_point),
            "U+{code_point:04X}"
        );
    }
}

#[test]
fn marks_required_after_line_feed() {
    let opportunities = SimpleCharacterLineBreakAnalyzer.analyze(&Text::from("a\nb"));
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(2))
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Allowed,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(1))
            .unwrap()
            .kind
    );
}

#[test]
fn collapses_crlf_to_a_single_break_after_lf() {
    let opportunities = SimpleCharacterLineBreakAnalyzer.analyze(&Text::from("a\r\nb"));
    assert_eq!(
        BreakKind::Allowed,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(2))
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(3))
            .unwrap()
            .kind
    );
}

#[test]
fn preserves_each_blank_line_break() {
    let opportunities = SimpleCharacterLineBreakAnalyzer.analyze(&Text::from("a\n\nb"));
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(2))
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == scalar_offset(3))
            .unwrap()
            .kind
    );
}

#[test]
fn test_simple_character_line_break_analyzer() {
    let analyzer = SimpleCharacterLineBreakAnalyzer;

    assert!(analyzer.analyze(&Text::new()).is_empty());

    let single = analyzer.analyze(&Text::from("A"));
    assert_eq!(1, single.len());
    assert_eq!(scalar_offset(1), single[0].index);
    assert_eq!(BreakKind::Required, single[0].kind);
    assert_eq!("SimpleCharacterLineBreakAnalyzer", single[0].reason);

    let multiple = analyzer.analyze(&Text::from("abc"));
    assert_eq!(3, multiple.len());
    assert_eq!(BreakKind::Allowed, multiple[0].kind);
    assert_eq!(BreakKind::Allowed, multiple[1].kind);
    assert_eq!(BreakKind::Required, multiple[2].kind);

    let with_lf = analyzer.analyze(&Text::from("a\nb"));
    assert_eq!(BreakKind::Allowed, with_lf[0].kind);
    assert_eq!(BreakKind::Required, with_lf[1].kind);
    assert_eq!("MandatoryBreak", with_lf[1].reason);
    assert_eq!(BreakKind::Required, with_lf[2].kind);

    let with_crlf = analyzer.analyze(&Text::from("a\r\nb"));
    assert_eq!(BreakKind::Allowed, with_crlf[0].kind);
    assert_eq!(BreakKind::Allowed, with_crlf[1].kind);
    assert_eq!("SimpleCharacterLineBreakAnalyzer", with_crlf[1].reason);
    assert_eq!(BreakKind::Required, with_crlf[2].kind);
    assert_eq!("MandatoryBreak", with_crlf[2].reason);
    assert_eq!(BreakKind::Required, with_crlf[3].kind);

    let with_cr_before_other = analyzer.analyze(&Text::from("a\rb"));
    assert_eq!(BreakKind::Required, with_cr_before_other[1].kind);
    assert_eq!("MandatoryBreak", with_cr_before_other[1].reason);

    let with_final_cr = analyzer.analyze(&Text::from("a\r"));
    assert_eq!(BreakKind::Required, with_final_cr[1].kind);
    assert_eq!("MandatoryBreak", with_final_cr[1].reason);
}
