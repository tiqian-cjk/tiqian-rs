use tiqian::clreq::BopomofoReading::{BopomofoReading, BopomofoTone, bopomofo_parser};
use tiqian::core::Text::Text;

#[test]
fn yinping_has_no_mark() {
    let reading = bopomofo_parser::parse(&Text::from("ㄓㄨㄥ"));
    assert_eq!(
        vec![Text::from("ㄓ"), Text::from("ㄨ"), Text::from("ㄥ"),],
        reading.symbols
    );
    assert_eq!(BopomofoTone::Yinping, reading.tone);
}

#[test]
fn suffix_marks_are_tone_and_stripped() {
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄔ".into(), "ㄤ".into()],
            tone: BopomofoTone::Yangping
        },
        bopomofo_parser::parse(&Text::from("ㄔㄤˊ"))
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄋ".into(), "ㄧ".into()],
            tone: BopomofoTone::Shang
        },
        bopomofo_parser::parse(&Text::from("ㄋㄧˇ"))
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄑ".into(), "ㄩ".into()],
            tone: BopomofoTone::Qu
        },
        bopomofo_parser::parse(&Text::from("ㄑㄩˋ"))
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄇ".into(), "ㄚ".into()],
            tone: BopomofoTone::Yinping
        },
        bopomofo_parser::parse(&Text::from("ㄇㄚˉ"))
    );
}

#[test]
fn neutral_tone_is_prefixed() {
    let reading = bopomofo_parser::parse(&Text::from("˙ㄉㄜ"));
    assert_eq!(vec![Text::from("ㄉ"), Text::from("ㄜ")], reading.symbols);
    assert_eq!(BopomofoTone::Neutral, reading.tone);
}

#[test]
fn single_symbol() {
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄦ".into()],
            tone: BopomofoTone::Yangping
        },
        bopomofo_parser::parse(&Text::from("ㄦˊ"))
    );
}
