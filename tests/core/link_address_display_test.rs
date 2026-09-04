use tiqian::core::text::Text;
use tiqian::core::text_model::link_address_display::displays_address;

#[test]
fn identical_display_and_target_is_an_address() {
    assert!(displays_address(
        &Text::from("https://example.com/a"),
        "https://example.com/a",
    ));
    assert!(displays_address(
        &Text::from("footnote-1"),
        "footnote-1",
    ));
}

#[test]
fn scheme_less_display_of_target_is_an_address() {
    assert!(displays_address(
        &Text::from("example.com/b"),
        "https://example.com/b",
    ));
    assert!(displays_address(
        &Text::from("example.com"),
        "http://example.com",
    ));
    assert!(displays_address(
        &Text::from("a@example.com"),
        "mailto:a@example.com",
    ));
}

#[test]
fn prose_display_text_is_not_an_address() {
    assert!(!displays_address(
        &Text::from("Example"),
        "https://example.com",
    ));
    assert!(!displays_address(
        &Text::from("示例站"),
        "https://example.com",
    ));
    assert!(!displays_address(
        &Text::from("action"),
        "generic",
    ));
    assert!(!displays_address(
        &Text::from(""),
        "https://example.com",
    ));
    assert!(!displays_address(&Text::from("Example"), ""));
}
