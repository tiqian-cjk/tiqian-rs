use tiqian::core::geometry::TextRange;

#[test]
fn exposes_length() {
    assert_eq!(3, TextRange::new(2, 5).length());
}

#[test]
#[should_panic(expected = "TextRange start must be non-negative.")]
fn rejects_negative_start() {
    TextRange::new(-1, 1);
}
