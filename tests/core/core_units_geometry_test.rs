use tiqian::core::geometry::{text_range, LayoutConstraints, Rect};
use tiqian::core::layout_model::{LayoutDebugInfo, MaxLinesDecisionInfo};
use tiqian::core::units::{Ic, IcLiteral};

#[test]
fn ic_plus_returns_sum() {
    assert_eq!(Ic { count: 5.0 }, Ic { count: 2.0 } + Ic { count: 3.0 });
}

#[test]
fn ic_unary_minus_returns_negated() {
    assert_eq!(Ic { count: -3.0 }, -Ic { count: 3.0 });
}

#[test]
fn float_ic_extension_creates_ic() {
    assert_eq!(Ic { count: 2.0 }, 2.0_f32.ic());
}

#[test]
fn int_ic_extension_creates_ic() {
    assert_eq!(Ic { count: 5.0 }, 5_i32.ic());
}

#[test]
fn ic_to_px_multiplies_by_em_size() {
    assert_eq!(24.0, Ic { count: 3.0 }.to_px(8.0));
}

#[test]
fn rect_height_returns_difference() {
    assert_eq!(20.0, Rect { left: 0.0, top: 0.0, right: 10.0, bottom: 20.0 }.height());
}

#[test]
fn rect_width_returns_difference() {
    assert_eq!(10.0, Rect { left: 0.0, top: 0.0, right: 10.0, bottom: 20.0 }.width());
}

#[test]
#[should_panic(expected = "TextRange start must not be greater than end.")]
fn text_range_rejects_start_greater_than_end() {
    text_range(5, 2);
}

#[test]
#[should_panic(expected = "maxWidth must be positive.")]
fn layout_constraints_rejects_non_positive_max_width() {
    LayoutConstraints::with_defaults(-1.0);
}

#[test]
#[should_panic(expected = "maxHeight must be positive.")]
fn layout_constraints_rejects_non_positive_max_height() {
    LayoutConstraints::with_max_height(100.0, -1.0);
}

#[test]
#[should_panic(expected = "maxLines must be positive.")]
fn layout_constraints_rejects_non_positive_max_lines() {
    LayoutConstraints::with_max_lines(100.0, 0);
}

#[test]
fn max_lines_decision_info_records_truncation_details() {
    let info = MaxLinesDecisionInfo::with_reason(5, 3, "MaxLinesLineTruncation".to_owned());
    assert_eq!(5, info.laid_out_lines);
    assert_eq!(3, info.visible_lines);
    assert_eq!("MaxLinesLineTruncation", info.reason);
}

#[test]
fn layout_debug_info_accepts_max_lines_decision() {
    let debug = LayoutDebugInfo::builder()
        .max_lines_decision(Some(MaxLinesDecisionInfo::new(5, 3)))
        .build();
    assert_eq!(Some(5), debug.max_lines_decision.as_ref().map(|info| info.laid_out_lines));
    assert_eq!(Some(3), debug.max_lines_decision.as_ref().map(|info| info.visible_lines));
}