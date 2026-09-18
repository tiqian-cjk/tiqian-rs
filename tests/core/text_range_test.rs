use tiqian::core::geometry::{scalar_offset, text_range, TextRange};

#[test]
fn exposes_length() {
    assert_eq!(3, text_range(2, 5).length());
}

#[test]
fn negative_offsets_normalize_before_range_construction() {
    assert_eq!(text_range(0, 1), TextRange::new(scalar_offset(-1), scalar_offset(1)));
}
