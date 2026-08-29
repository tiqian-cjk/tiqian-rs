use tiqian::core::UnicodeScriptEvidence::{
    UnicodeScriptEvidence, unicode_script_evidence_classifier,
};

#[test]
fn common_and_inherited_scalars_do_not_vote() {
    for code_point in [0x20, 0x30, 0x201C, 0xFF1F, 0x0301, 0x1F600] {
        assert_eq!(
            UnicodeScriptEvidence::Neutral,
            unicode_script_evidence_classifier::classify(code_point),
            "U+{code_point:04X}",
        );
    }
}

#[test]
fn east_asian_scripts_are_distinct_from_other_strong_scripts() {
    for code_point in ['中' as i32, 0x3105, 0x3042, 0x30A2, 0xAC00, 0x20000] {
        assert_eq!(
            UnicodeScriptEvidence::EastAsian,
            unicode_script_evidence_classifier::classify(code_point),
            "U+{code_point:04X}",
        );
    }
    for code_point in ['A' as i32, 0x03C0, 0x0416, 0x0627] {
        assert_eq!(
            UnicodeScriptEvidence::Other,
            unicode_script_evidence_classifier::classify(code_point),
            "U+{code_point:04X}",
        );
    }
}
