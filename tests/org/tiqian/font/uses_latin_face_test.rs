use tiqian::core::text::Text;
use tiqian::font::font_policy::{FontRole, font_role_name_uses_latin_face};

#[test]
fn only_latin_text_uses_latin_face() {
    assert!(FontRole::LatinText.uses_latin_face());
    for role in [
        FontRole::CjkText,
        FontRole::CjkPunctuation,
        FontRole::Symbol,
        FontRole::Emoji,
        FontRole::Unknown,
    ] {
        assert!(
            !role.uses_latin_face(),
            "{role:?} must fall back to the CJK face"
        );
    }
}

#[test]
fn name_overload_agrees_with_enum() {
    for (role, name) in [
        (FontRole::CjkText, "CjkText"),
        (FontRole::CjkPunctuation, "CjkPunctuation"),
        (FontRole::LatinText, "LatinText"),
        (FontRole::Symbol, "Symbol"),
        (FontRole::Emoji, "Emoji"),
        (FontRole::Unknown, "Unknown"),
    ] {
        assert_eq!(
            role.uses_latin_face(),
            font_role_name_uses_latin_face(Some(&Text::from(name)))
        );
    }
    assert!(!font_role_name_uses_latin_face(None));
    assert!(!font_role_name_uses_latin_face(Some(&Text::from(
        "NotARole"
    ))));
}
