use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::linebreak::EnglishHyphenation::english_hyphenation;

fn hyphenated(word: &str) -> String {
    let offsets = english_hyphenation::en_us().hyphenate(&Text::from(word));
    word.chars()
        .enumerate()
        .flat_map(|(index, character)| {
            let offset = index as i32;
            offsets
                .contains(&offset)
                .then_some('-')
                .into_iter()
                .chain(std::iter::once(character))
        })
        .collect()
}

#[test]
fn hyphenates_common_words_at_syllable_points() {
    assert_eq!("hy-phen-ation", hyphenated("hyphenation"));
    assert_eq!("com-puter", hyphenated("computer"));
    assert!(hyphenated("international").starts_with("in-ter"));
}

#[test]
fn respects_margins_and_short_words() {
    assert!(
        english_hyphenation::en_us()
            .hyphenate(&Text::from("the"))
            .is_empty()
    );
    assert!(
        english_hyphenation::en_us()
            .hyphenate(&Text::from("a"))
            .is_empty()
    );
    let word = "supercalifragilistic";
    let offsets = english_hyphenation::en_us().hyphenate(&Text::from(word));
    assert!(
        offsets
            .iter()
            .all(|offset| (2..=word.chars().count() as i32 - 3).contains(offset)),
        "offsets={offsets:?}"
    );
}

#[test]
fn honours_the_exception_list() {
    assert!(
        english_hyphenation::en_us()
            .hyphenate(&Text::from("project"))
            .is_empty()
    );
    assert!(
        english_hyphenation::en_us()
            .hyphenate(&Text::from("present"))
            .is_empty()
    );
}
