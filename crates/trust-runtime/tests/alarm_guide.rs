//! The alarm library's manual (`docs/guides/ALARM_LIBRARY_GUIDE.md`, specification 40): every
//! Structured Text example in it compiles with the library and runs, and it names every block
//! and type the library declares.

use std::path::Path;

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Duration;

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");

fn guide() -> String {
    std::fs::read_to_string(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/guides/ALARM_LIBRARY_GUIDE.md"
    )))
    .expect("the manual exists")
}

#[test]
fn every_example_of_the_manual_compiles_and_runs_with_the_library() {
    let guide = guide();
    let examples: Vec<&str> = guide
        .split("```iecst")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .collect();
    assert!(
        examples.len() >= 4,
        "the manual shows the library at work: {} examples",
        examples.len()
    );
    for (index, example) in examples.iter().enumerate() {
        let mut harness = TestHarness::from_sources(&[ALARM, example])
            .unwrap_or_else(|error| panic!("example {} compiles: {error:?}\n{example}", index + 1));
        for _ in 0..3 {
            harness.advance_time(Duration::from_millis(100));
            let result = harness.cycle();
            assert!(
                result.errors.is_empty(),
                "example {} runs: {:?}",
                index + 1,
                result.errors
            );
        }
    }
}

#[test]
fn the_manual_names_every_block_and_type_the_library_declares() {
    let guide = guide();
    let declared: Vec<&str> = ALARM
        .lines()
        .filter_map(|line| {
            line.strip_prefix("FUNCTION_BLOCK ")
                .or_else(|| line.strip_prefix("TYPE "))
        })
        .filter_map(|rest| rest.split([' ', ':']).next())
        .collect();
    assert!(
        declared.len() >= 9,
        "the library's declarations: {declared:?}"
    );
    let missing: Vec<&str> = declared
        .into_iter()
        .filter(|name| !guide.contains(&format!("`{name}`")))
        .collect();
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "blocks and types the manual does not name"
    );
}
