#[path = "fixture_layout/mod.rs"]
mod fixture_layout;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fixture_layout::{dump_fixture, fixtures};

const UPDATE_ENV: &str = "TIQIAN_UPDATE_LAYOUT_GOLDENS";

#[test]
fn layout_fixture_goldens_match() {
    let fixtures = fixtures();
    let fixture_ids = fixtures.iter().map(|fixture| fixture.id).collect::<BTreeSet<_>>();
    assert_eq!(fixtures.len(), fixture_ids.len(), "fixture IDs must be unique");

    let golden_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_layout/golden");
    let updating = std::env::var(UPDATE_ENV).as_deref() == Ok("1");
    if updating {
        fs::create_dir_all(&golden_directory).expect("create fixture golden directory");
        for fixture in &fixtures {
            fs::write(golden_directory.join(format!("{}.txt", fixture.id)), dump_fixture(fixture))
                .expect("write fixture golden");
        }
    }

    let golden_ids = fs::read_dir(&golden_directory)
        .expect("read fixture golden directory")
        .map(|entry| entry.expect("read fixture golden entry").file_name().into_string().expect("UTF-8 fixture golden name"))
        .map(|name| name.strip_suffix(".txt").expect("fixture golden must use .txt extension").to_owned())
        .collect::<BTreeSet<_>>();
    let expected_ids = fixture_ids.iter().map(|id| (*id).to_owned()).collect::<BTreeSet<_>>();
    assert_eq!(expected_ids, golden_ids, "fixture IDs and golden file names must match");

    for fixture in &fixtures {
        let golden_path = golden_directory.join(format!("{}.txt", fixture.id));
        let expected = fs::read_to_string(&golden_path).expect("read fixture golden");
        let actual = dump_fixture(fixture);
        assert_eq!(expected, actual, "{}", diff_message(fixture.id, &expected, &actual));
    }
}

fn diff_message(id: &str, expected: &str, actual: &str) -> String {
    let expected = expected.lines().collect::<Vec<_>>();
    let actual = actual.lines().collect::<Vec<_>>();
    let mut differences = Vec::new();
    for index in 0..expected.len().max(actual.len()) {
        if expected.get(index) != actual.get(index) {
            differences.push(format!(
                "line {} [{}]:\n  golden: {}\n  actual: {}",
                index + 1,
                breaker_at(&actual, index),
                expected.get(index).copied().unwrap_or("<missing>"),
                actual.get(index).copied().unwrap_or("<missing>")
            ));
            if differences.len() == 8 {
                break;
            }
        }
    }
    format!("fixture '{id}' golden mismatch:\n{}", differences.join("\n"))
}

fn breaker_at<'a>(lines: &'a [&'a str], index: usize) -> &'a str {
    lines[..index.min(lines.len())]
        .iter()
        .rev()
        .find_map(|line| line.strip_prefix("== ").and_then(|line| line.strip_suffix(" ==")))
        .unwrap_or("header")
}