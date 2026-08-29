use tiqian::core::east_asian_spacing::{
    EastAsianSpacingEdges, EastAsianSpacingValue, unicode_east_asian_spacing,
};
use tiqian::core::text::Text;

#[test]
fn chinese_language_context_uses_pinned_macrolanguage_registry() {
    assert!(unicode_east_asian_spacing::is_chinese_language_context(
        &Text::from("zh-Hans")
    ));
    assert!(unicode_east_asian_spacing::is_chinese_language_context(
        &Text::from("yue-Hant-HK")
    ));
    assert!(!unicode_east_asian_spacing::is_chinese_language_context(
        &Text::from("en")
    ));
}

#[test]
fn uses_pinned_unicode_draft_data_across_scripts() {
    assert_eq!(
        EastAsianSpacingValue::Wide,
        unicode_east_asian_spacing::property_of('提' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Wide,
        unicode_east_asian_spacing::property_of(0x17000)
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::property_of('A' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::property_of('α' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::property_of('я' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::property_of('9' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Conditional,
        unicode_east_asian_spacing::property_of('%' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::property_of('／' as i32)
    );
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::property_of(0x1F600)
    );
}

#[test]
fn resolves_conditional_values_from_chinese_language_context() {
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::resolved_for_grapheme_cluster(
            &Text::from("%"),
            &Text::from("zh-Hans"),
        )
    );
    assert_eq!(
        EastAsianSpacingValue::Narrow,
        unicode_east_asian_spacing::resolved_for_grapheme_cluster(
            &Text::from("%"),
            &Text::from("yue-Hant-HK"),
        )
    );
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::resolved_for_grapheme_cluster(
            &Text::from("%"),
            &Text::from("en"),
        )
    );
}

#[test]
fn enclosing_mark_makes_the_whole_grapheme_cluster_other() {
    assert_eq!(
        EastAsianSpacingValue::Other,
        unicode_east_asian_spacing::resolved_for_grapheme_cluster(
            &Text::from("A\u{20DD}"),
            &Text::from("zh-Hans"),
        )
    );
}

#[test]
fn resolves_actual_source_units_at_each_shaping_cluster_edge() {
    assert_eq!(
        EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Other,
            trailing: EastAsianSpacingValue::Narrow,
            contains_wide: false,
        },
        unicode_east_asian_spacing::resolved_edges(&Text::from("/Hi"), &Text::from("zh-Hans"),),
    );
    assert_eq!(
        EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Other,
            trailing: EastAsianSpacingValue::Other,
            contains_wide: false,
        },
        unicode_east_asian_spacing::resolved_edges(
            &Text::from("A\u{20DD}"),
            &Text::from("zh-Hans"),
        ),
    );
}
