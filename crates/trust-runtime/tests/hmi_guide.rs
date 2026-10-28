//! The HMI library's manual (`docs/guides/HMI_LIBRARY_GUIDE.md`, specification 39, §18 phase 5):
//! every Structured Text example in it compiles with the library and serves the screens, and it
//! names the block and its output.

use std::path::Path;
use std::sync::Arc;

use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::exchange::HmiExchange;
use trust_runtime::value::Duration;

const HMI: &str = include_str!("../../../libraries/hmi/src/hmi.st");

fn guide() -> String {
    std::fs::read_to_string(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/guides/HMI_LIBRARY_GUIDE.md"
    )))
    .expect("the manual exists")
}

#[test]
fn every_example_of_the_manual_compiles_and_serves_the_screens() {
    let guide = guide();
    let examples: Vec<&str> = guide
        .split("```iecst")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .collect();
    assert!(!examples.is_empty(), "the manual shows the block at work");
    for (index, example) in examples.iter().enumerate() {
        let mut harness = TestHarness::from_sources(&[HMI, example])
            .unwrap_or_else(|error| panic!("example {} compiles: {error:?}\n{example}", index + 1));
        let exchange = Arc::new(HmiExchange::new());
        harness
            .runtime_mut()
            .set_hmi_exchange(Arc::clone(&exchange));
        for _ in 0..30 {
            harness.advance_time(Duration::from_millis(10));
            let result = harness.cycle();
            assert!(
                result.errors.is_empty(),
                "example {} runs: {:?}",
                index + 1,
                result.errors
            );
        }
        assert!(
            exchange.calls() > 0,
            "example {} serves the screens",
            index + 1
        );
    }
}

#[test]
fn the_manual_names_the_block_and_its_output() {
    let guide = guide();
    for name in ["Hmi.Display", "Calls"] {
        assert!(guide.contains(name), "the manual names {name}");
    }
}
