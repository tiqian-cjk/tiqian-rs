use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::linebreak::LineBreak::{
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
            .find(|opportunity| opportunity.index == 2)
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Allowed,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == 1)
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
            .find(|opportunity| opportunity.index == 2)
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == 3)
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
            .find(|opportunity| opportunity.index == 2)
            .unwrap()
            .kind
    );
    assert_eq!(
        BreakKind::Required,
        opportunities
            .iter()
            .find(|opportunity| opportunity.index == 3)
            .unwrap()
            .kind
    );
}
