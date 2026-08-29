use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::link_address_display::displays_address;

#[test]
fn identical_display_and_target_is_an_address() {
    assert!(displays_address(
        &Text::from("https://example.com/a"),
        &Text::from("https://example.com/a"),
    ));
    assert!(displays_address(
        &Text::from("footnote-1"),
        &Text::from("footnote-1"),
    ));
}

#[test]
fn scheme_less_display_of_target_is_an_address() {
    assert!(displays_address(
        &Text::from("example.com/b"),
        &Text::from("https://example.com/b"),
    ));
    assert!(displays_address(
        &Text::from("example.com"),
        &Text::from("http://example.com"),
    ));
    assert!(displays_address(
        &Text::from("a@example.com"),
        &Text::from("mailto:a@example.com"),
    ));
}

#[test]
fn prose_display_text_is_not_an_address() {
    assert!(!displays_address(
        &Text::from("Example"),
        &Text::from("https://example.com"),
    ));
    assert!(!displays_address(
        &Text::from("示例站"),
        &Text::from("https://example.com"),
    ));
    assert!(!displays_address(
        &Text::from("action"),
        &Text::from("generic"),
    ));
    assert!(!displays_address(
        &Text::from(""),
        &Text::from("https://example.com"),
    ));
    assert!(!displays_address(&Text::from("Example"), &Text::from(""),));
}
