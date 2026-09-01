use tiqian::core::geometry::TextRange;
use tiqian::core::text::Text;
use tiqian::font::font_policy::{CjkFontRoleClassifier, FontRole};

#[test]
fn bmp_math_and_currency_symbols_resolve_to_symbol_role() {
    let classifier = CjkFontRoleClassifier;
    assert_eq!(
        FontRole::Symbol,
        classifier.classify_with_default_context(&Text::from("±"), TextRange::new(0, 1)),
    );
    assert_eq!(
        FontRole::Symbol,
        classifier.classify_with_default_context(&Text::from("€"), TextRange::new(0, 1)),
    );
}