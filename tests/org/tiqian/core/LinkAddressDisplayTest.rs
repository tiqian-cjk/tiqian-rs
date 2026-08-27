use tiqian::org::tiqian::core::TextModel::link_address_display::displays_address;

#[test]
fn identical_display_and_target_is_an_address() {
    assert!(displays_address("https://example.com/a", "https://example.com/a"));
    assert!(displays_address("footnote-1", "footnote-1"));
}

#[test]
fn scheme_less_display_of_target_is_an_address() {
    assert!(displays_address("example.com/b", "https://example.com/b"));
    assert!(displays_address("example.com", "http://example.com"));
    assert!(displays_address("a@example.com", "mailto:a@example.com"));
}

#[test]
fn prose_display_text_is_not_an_address() {
    assert!(!displays_address("Example", "https://example.com"));
    assert!(!displays_address("示例站", "https://example.com"));
    assert!(!displays_address("action", "generic"));
    assert!(!displays_address("", "https://example.com"));
    assert!(!displays_address("Example", ""));
}
