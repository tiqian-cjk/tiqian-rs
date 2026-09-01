use tiqian::clreq::bopomofo_reading::{BopomofoTone, bopomofo_parser};
use tiqian::clreq::clreq_profile::{KinsokuLevel, clreq_punctuation_policies};
use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::kinsoku_rule::{ClreqKinsokuRule, KinsokuRule};

#[test]
fn forbidden_at_line_start_covers_every_punctuation_class() {
    for level in [KinsokuLevel::Basic, KinsokuLevel::GbStyle, KinsokuLevel::Strict] {
        for character in ['，', '”', '·', '・', '～', '/'] {
            assert!(clreq_punctuation_policies::forbidden_at_line_start(character, level));
        }
    }
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('—', KinsokuLevel::Basic));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('—', KinsokuLevel::GbStyle));
    assert!(clreq_punctuation_policies::forbidden_at_line_start('—', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('…', KinsokuLevel::Basic));
    assert!(clreq_punctuation_policies::forbidden_at_line_start('…', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('文', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_start('，', KinsokuLevel::None));
}

#[test]
fn forbidden_at_line_end_covers_opening_solidus_and_other() {
    assert!(clreq_punctuation_policies::forbidden_at_line_end('“', KinsokuLevel::Basic));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('/', KinsokuLevel::Basic));
    assert!(clreq_punctuation_policies::forbidden_at_line_end('/', KinsokuLevel::GbStyle));
    assert!(clreq_punctuation_policies::forbidden_at_line_end('/', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('，', KinsokuLevel::Strict));
    assert!(!clreq_punctuation_policies::forbidden_at_line_end('“', KinsokuLevel::None));
}

#[test]
fn kinsoku_rule_allows_clusters_without_display_text() {
    let empty = Cluster::new(TextRange::new(0, 0), Text::new(), "stub".into(), 0.0);
    let rule = ClreqKinsokuRule::default();
    assert!(!rule.forbidden_at_line_start(&empty));
    assert!(!rule.forbidden_at_line_end(&empty));
}

#[test]
fn bopomofo_parser_covers_every_tone_arm() {
    let plain = bopomofo_parser::parse(&Text::from("ㄅㄚ"));
    assert_eq!(BopomofoTone::Yinping, plain.tone);
    assert_eq!(vec![Text::from("ㄅ"), Text::from("ㄚ")], plain.symbols);
    assert_eq!(BopomofoTone::Yangping, bopomofo_parser::parse(&Text::from("ㄅㄚˊ")).tone);
    assert_eq!(BopomofoTone::Shang, bopomofo_parser::parse(&Text::from("ㄅㄚˇ")).tone);
    assert_eq!(BopomofoTone::Qu, bopomofo_parser::parse(&Text::from("ㄅㄚˋ")).tone);
    assert_eq!(BopomofoTone::Yinping, bopomofo_parser::parse(&Text::from("ㄅㄚˉ")).tone);
    let neutral = bopomofo_parser::parse(&Text::from("˙ㄅㄚ"));
    assert_eq!(BopomofoTone::Neutral, neutral.tone);
    assert_eq!(vec![Text::from("ㄅ"), Text::from("ㄚ")], neutral.symbols);
    assert_eq!(BopomofoTone::Yinping, bopomofo_parser::parse(&Text::new()).tone);
    let in_range_default = bopomofo_parser::parse(&Text::from("ㄅㄚˈ"));
    assert_eq!(BopomofoTone::Yinping, in_range_default.tone);
    assert_eq!(vec![Text::from("ㄅ"), Text::from("ㄚ"), Text::from("ˈ")], in_range_default.symbols);
    assert_eq!(BopomofoTone::Yinping, bopomofo_parser::parse(&Text::from("ㄅㄚˌ")).tone);
}