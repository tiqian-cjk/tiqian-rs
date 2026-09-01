use tiqian::core::east_asian_spacing::{EastAsianSpacingValue, unicode_east_asian_spacing};

#[test]
fn lookup_covers_every_generated_value_and_both_miss_directions() {
    for (code_point, expected) in [
        ('!' as i32, EastAsianSpacingValue::Conditional),
        ('A' as i32, EastAsianSpacingValue::Narrow),
        ('0' as i32, EastAsianSpacingValue::Narrow),
        (0x4E00, EastAsianSpacingValue::Wide),
        (0x9FFF, EastAsianSpacingValue::Wide),
        (0x02, EastAsianSpacingValue::Other),
        (0x10FFFF, EastAsianSpacingValue::Other),
        (0x22, EastAsianSpacingValue::Other),
    ] {
        assert_eq!(expected, unicode_east_asian_spacing::property_of(code_point));
    }
}