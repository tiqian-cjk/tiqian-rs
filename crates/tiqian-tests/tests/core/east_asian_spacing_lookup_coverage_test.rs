use tiqian::core::east_asian_spacing::{EastAsianSpacingValue, unicode_east_asian_spacing};

#[test]
fn lookup_covers_every_generated_value_and_both_miss_directions() {
    for (character, expected) in [
        ('!', EastAsianSpacingValue::Conditional),
        ('A', EastAsianSpacingValue::Narrow),
        ('0', EastAsianSpacingValue::Narrow),
        ('中', EastAsianSpacingValue::Wide),
        ('\u{9FFF}', EastAsianSpacingValue::Wide),
        ('\u{02}', EastAsianSpacingValue::Other),
        ('\u{10FFFF}', EastAsianSpacingValue::Other),
        ('"', EastAsianSpacingValue::Other),
    ] {
        assert_eq!(expected, unicode_east_asian_spacing::property_of(character));
    }
}
