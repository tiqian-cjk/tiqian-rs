use tiqian::core::geometry::{scalar_offset, text_range, TextRange};

#[test]
fn exposes_length() {
    assert_eq!(3, text_range(2, 5).length());
}

#[test]
#[should_panic(expected = "ScalarOffset must be non-negative.")]
fn rejects_negative_start() {
    TextRange::new(scalar_offset(-1), scalar_offset(1));
}
