//! Specification 42, §5: the Comm library pairs `Comm.Link` with the link faceplate and the
//! `network-link` symbol; the faceplate shows the link's state, since when, the last data, the
//! latest error, its counters and the alarm, and lets an engineer switch and test the link.

use std::path::Path;

use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::binding_checks::{check_faceplate, check_pair};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{
    parse_faceplate, parse_pair, write_faceplate, write_pair, HmiBindingDirection, HmiFaceplate,
    HmiRole,
};

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");
const COMM: &str = include_str!("../../../libraries/comm/src/comm.st");

fn read(folder: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../libraries/comm/hmi/{folder}/{name}.toml"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("the library ships hmi/{folder}/{name}.toml"))
}

fn faceplate() -> (HmiFaceplate, String) {
    let text = read("faceplates", "link");
    let faceplate = parse_faceplate("hmi/panels/link.toml", &text).expect("the faceplate loads");
    (faceplate, text)
}

#[test]
fn the_link_block_is_paired_with_the_link_faceplate_and_symbol_and_checks_clean() {
    let text = read("pairs", "link");
    let pair = parse_pair("hmi/pairs/link.toml", &text).expect("the pair loads");
    assert_eq!(
        (
            pair.block.as_str(),
            pair.faceplate.as_str(),
            pair.symbol.as_str()
        ),
        ("Comm.Link", "link", "network-link")
    );
    assert_eq!(
        pair.bind.get("out-of-service").map(String::as_str),
        Some("Disabled"),
        "a link switched off shows on its symbol as out of service"
    );
    assert_eq!(write_pair(&pair), text, "the pair is written canonically");
    let (faceplate, faceplate_text) = faceplate();
    assert_eq!(
        write_faceplate(&faceplate),
        faceplate_text,
        "the faceplate is written canonically"
    );
    let harness = TestHarness::from_sources(&[
        ALARM,
        COMM,
        "PROGRAM Main\nVAR\n    Unused : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    ])
    .expect("the libraries compile");
    let metadata = harness.runtime().metadata_snapshot();
    assert_eq!(
        check_pair(&pair, &faceplate, &metadata),
        vec![],
        "the pair against the library"
    );
    let catalogue = Catalogue::shipped();
    assert_eq!(
        check_faceplate(
            &faceplate,
            catalogue,
            &metadata,
            harness.runtime().storage()
        ),
        vec![]
    );
    assert!(
        catalogue.component(&pair.symbol).is_some(),
        "the symbol is a component"
    );
}

/// Specification 42, §4 items 6–8: the faceplate shows the link, its counters and response time,
/// and lets an engineer switch it, test it and reset its counters; nothing else is written.
#[test]
fn the_link_faceplate_shows_the_link_and_lets_an_engineer_switch_and_test_it() {
    let (faceplate, _) = faceplate();
    assert!(
        faceplate.description.is_some(),
        "a description for its header"
    );
    let bindings: Vec<(&str, HmiBindingDirection)> = faceplate
        .popup
        .page
        .objects
        .iter()
        .flat_map(|object| object.bind.values())
        .filter_map(|binding| {
            binding
                .path
                .strip_prefix('$')
                .map(|field| (field, binding.direction))
        })
        .collect();
    for field in [
        "Name",
        "State",
        "StateTime",
        "Connected",
        "LastGoodAge",
        "LastError",
        "LinkFailed",
        "Exchanges",
        "Failures",
        "ResponseTime",
        "TestResult",
        "Detail",
    ] {
        assert!(
            bindings.iter().any(|(bound, _)| *bound == field),
            "the faceplate shows {field}"
        );
    }
    let mut written: Vec<&str> = bindings
        .iter()
        .filter(|(_, direction)| *direction != HmiBindingDirection::Read)
        .map(|(field, _)| *field)
        .collect();
    written.sort_unstable();
    assert_eq!(
        written,
        ["Enable", "Reset", "Test"],
        "only the link's switches are written; its state is the runtime's"
    );
    for object in faceplate.popup.page.objects.iter().filter(|object| {
        object
            .bind
            .values()
            .any(|binding| binding.direction != HmiBindingDirection::Read)
    }) {
        assert_eq!(
            object.role,
            Some(HmiRole::Engineer),
            "{} is for engineers",
            object.id
        );
    }
    let symbol = faceplate
        .popup
        .page
        .objects
        .iter()
        .find(|object| object.id == "symbol")
        .expect("the symbol");
    assert_eq!(symbol.kind, "network-link");
    assert_eq!(
        symbol
            .bind
            .get("connected")
            .map(|binding| binding.path.as_str()),
        Some("$Connected")
    );
    assert_eq!(
        symbol
            .bind
            .get("out-of-service")
            .map(|binding| binding.path.as_str()),
        Some("$Disabled"),
        "switched off on purpose, the symbol is out of service, not lost"
    );
}
