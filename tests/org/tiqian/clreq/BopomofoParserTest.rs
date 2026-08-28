use tiqian::org::tiqian::clreq::BopomofoReading::{BopomofoReading, BopomofoTone, bopomofo_parser};

#[test]
fn yinping_has_no_mark() {
    let reading = bopomofo_parser::parse("ㄓㄨㄥ");
    assert_eq!(vec!["ㄓ", "ㄨ", "ㄥ"], reading.symbols);
    assert_eq!(BopomofoTone::Yinping, reading.tone);
}

#[test]
fn suffix_marks_are_tone_and_stripped() {
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄔ".into(), "ㄤ".into()],
            tone: BopomofoTone::Yangping
        },
        bopomofo_parser::parse("ㄔㄤˊ")
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄋ".into(), "ㄧ".into()],
            tone: BopomofoTone::Shang
        },
        bopomofo_parser::parse("ㄋㄧˇ")
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄑ".into(), "ㄩ".into()],
            tone: BopomofoTone::Qu
        },
        bopomofo_parser::parse("ㄑㄩˋ")
    );
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄇ".into(), "ㄚ".into()],
            tone: BopomofoTone::Yinping
        },
        bopomofo_parser::parse("ㄇㄚˉ")
    );
}

#[test]
fn neutral_tone_is_prefixed() {
    let reading = bopomofo_parser::parse("˙ㄉㄜ");
    assert_eq!(vec!["ㄉ", "ㄜ"], reading.symbols);
    assert_eq!(BopomofoTone::Neutral, reading.tone);
}

#[test]
fn single_symbol() {
    assert_eq!(
        BopomofoReading {
            symbols: vec!["ㄦ".into()],
            tone: BopomofoTone::Yangping
        },
        bopomofo_parser::parse("ㄦˊ")
    );
}
