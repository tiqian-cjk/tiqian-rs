use tiqian::core::east_asian_spacing::{EastAsianSpacingValue, unicode_east_asian_spacing};
use tiqian::core::unicode_script_evidence::{
    UnicodeScriptEvidence, unicode_script_evidence_classifier,
};
use tiqian::core::unicode_word_character::unicode_word_character;

#[test]
fn test_unicode_word_character() {
    assert_eq!("17.0.0", unicode_word_character::DATA_REVISION);
    assert!(!unicode_word_character::DATA_SOURCE.is_empty());
    assert!(!unicode_word_character::DATA_SHA256.is_empty());
    assert!(unicode_word_character::contains('A'));
    assert!(unicode_word_character::contains('中'));
    assert!(!unicode_word_character::contains(' '));
    assert!(!unicode_word_character::contains('!'));
}

#[test]
fn test_unicode_script_evidence() {
    assert_eq!("17.0.0", unicode_script_evidence_classifier::DATA_REVISION);
    assert!(!unicode_script_evidence_classifier::DATA_SOURCE.is_empty());
    assert!(!unicode_script_evidence_classifier::DATA_SHA256.is_empty());
    assert_eq!(
        UnicodeScriptEvidence::EastAsian,
        unicode_script_evidence_classifier::classify('中')
    );
    assert_eq!(
        UnicodeScriptEvidence::Other,
        unicode_script_evidence_classifier::classify('A')
    );
    assert_eq!(
        UnicodeScriptEvidence::Neutral,
        unicode_script_evidence_classifier::classify(' ')
    );
}

#[test]
fn test_east_asian_spacing_data_and_values() {
    assert_eq!(
        EastAsianSpacingValue::Wide,
        unicode_east_asian_spacing::property_of('\u{02C7}')
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::property_of('0')
    );
    assert_eq!(
        EastAsianSpacingValue::Conditional,
        unicode_east_asian_spacing::property_of('!')
    );
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::property_of('\0')
    );
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::property_of('\u{10FFFF}')
    );
}
