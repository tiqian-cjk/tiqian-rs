use tiqian::clreq::NumberSymbolCohesion::number_symbol_cohesion;
use tiqian::core::Text::Text;

fn groups(text: &str) -> Vec<String> {
    number_symbol_cohesion::unbreakable_ranges(&Text::from(text))
        .into_iter()
        .map(|range| {
            let units: Vec<u16> = text.encode_utf16().collect();
            String::from_utf16(&units[range.first() as usize..=range.last() as usize])
                .expect("Kotlin source range must be valid UTF-16")
        })
        .collect()
}

#[test]
fn binds_digits_with_suffix_unit_prefix_sign_and_currency() {
    assert_eq!(vec!["50%"], groups("增长50%了"));
    assert_eq!(vec!["37℃"], groups("温37℃高"));
    assert_eq!(vec!["90°"], groups("转90°角"));
    assert_eq!(vec!["+5"], groups("是+5度"));
    assert_eq!(vec!["±2"], groups("误差±2毫米"));
    assert_eq!(vec!["¥100"], groups("价¥100元"));
    assert_eq!(vec!["100₫"], groups("约100₫的"));
}

#[test]
fn keeps_interior_decimal_and_thousands_separators() {
    assert_eq!(vec!["3.14"], groups("π≈3.14啦"));
    assert_eq!(vec!["1,000"], groups("共1,000人"));
    assert_eq!(vec!["100"], groups("有100。"));
}

#[test]
fn bare_number_is_its_own_group() {
    assert_eq!(vec!["2024"], groups("在2024年"));
    assert!(groups("纯中文没有数字").is_empty());
}
