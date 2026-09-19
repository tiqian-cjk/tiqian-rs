use tiqian::core::unicode_script_evidence::{
    UnicodeScriptEvidence, unicode_script_evidence_classifier,
};

#[test]
fn common_and_inherited_scalars_do_not_vote() {
    for character in [' ', '0', '\u{201C}', '？', '\u{0301}', '😀'] {
        assert_eq!(
            UnicodeScriptEvidence::Neutral,
            unicode_script_evidence_classifier::classify(character),
            "U+{:04X}", character as u32,
        );
    }
}

#[test]
fn east_asian_scripts_are_distinct_from_other_strong_scripts() {
    for character in ['中', '\u{3105}', 'あ', 'ア', '가', '\u{20000}'] {
        assert_eq!(
            UnicodeScriptEvidence::EastAsian,
            unicode_script_evidence_classifier::classify(character),
            "U+{:04X}", character as u32,
        );
    }
    for character in ['A', 'π', 'Ж', 'ا'] {
        assert_eq!(
            UnicodeScriptEvidence::Other,
            unicode_script_evidence_classifier::classify(character),
            "U+{:04X}", character as u32,
        );
    }
}
