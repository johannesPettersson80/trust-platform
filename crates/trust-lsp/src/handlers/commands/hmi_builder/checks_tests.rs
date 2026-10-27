//! The small checks of the HMI Builder (row H13): a page made from the program, the PLC cycle,
//! the languages and Keep in project's comparison (specification 39, §4 item 1, §9 item 3,
//! §10.3 item 3, §14.5 item 2, §21.4 item 6).

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::parse_screen;

use super::*;

fn ask(request: Value) -> Value {
    answer(request).expect("the request is answered")
}

fn new_page(pages: &[&str], has_screen: bool) -> Value {
    ask(json!({ "action": "newPage", "name": "overview", "pages": pages, "hasScreen": has_screen }))
}

fn same(project: Value, running: Value) -> bool {
    ask(json!({ "action": "sameSetting", "project": project, "running": running }))["same"]
        .as_bool()
        .expect("same or not")
}

#[test]
fn a_page_made_from_the_program_takes_its_name_or_the_first_free_number() {
    // §9 item 3: an existing page is never overwritten. Catches numbering from 1, a taken
    // number used again, and the plain name passed over while it is free.
    assert_eq!(
        new_page(&[], true),
        json!({ "ok": true, "file": "hmi/pages/overview.toml" })
    );
    assert_eq!(
        new_page(&["overview"], true)["file"],
        "hmi/pages/overview-2.toml"
    );
    assert_eq!(
        new_page(&["overview", "overview-2", "settings"], true)["file"],
        "hmi/pages/overview-3.toml"
    );
    assert_eq!(
        new_page(&["overview-2"], true)["file"],
        "hmi/pages/overview.toml"
    );
}

#[test]
fn a_page_made_in_a_project_without_a_screen_layout_brings_one_that_loads() {
    // §4 item 1. Catches the `format = 1` alone written before, which the loader refuses for
    // lacking the size, theme and symbols.
    let answered = new_page(&[], false);
    let screen = answered["screen"].as_str().expect("a screen layout");
    assert_eq!(screen, super::super::screens::default_screen());
    assert_eq!(
        parse_screen("hmi/screen.toml", screen)
            .expect("the layout loads")
            .size,
        [1920, 1080]
    );
}

#[test]
fn a_page_name_that_cannot_name_a_file_is_refused() {
    // Catches a page file created under a name the runtime cannot address.
    let refused = answer(json!({
        "action": "newPage", "name": "my page", "pages": [], "hasScreen": true,
    }));
    assert_eq!(refused, Err("\"my page\" cannot name a page.".to_string()));
}

#[test]
fn the_control_cycle_is_a_whole_number_of_milliseconds_from_1_to_60000() {
    // §10.3 item 3. Catches 0 and 60 001 accepted, a fraction, sign, unit or space accepted, a
    // number too long for 32 bits read as another number, and digits of other scripts taken as
    // digits.
    for (text, milliseconds) in [("1", 1), ("250", 250), ("60000", 60_000), ("007", 7)] {
        assert_eq!(
            ask(json!({ "action": "cycle", "text": text })),
            json!({ "ok": true, "milliseconds": milliseconds }),
            "{text}"
        );
    }
    for text in [
        "",
        "0",
        "60001",
        "1.5",
        "-5",
        "+5",
        "5ms",
        " 5",
        "5 ",
        "4294967297",
        "١٢",
    ] {
        assert_eq!(
            ask(json!({ "action": "cycle", "text": text })),
            json!({ "ok": true, "problem": "Enter a whole number from 1 to 60000 milliseconds." }),
            "{text:?}"
        );
    }
}

#[test]
fn a_project_has_at_least_one_language() {
    // §14.5 item 2. Catches an empty language list written to the screen layout.
    assert_eq!(
        ask(json!({ "action": "languages", "languages": [] })),
        json!({ "ok": true, "problem": "A project has at least one language." })
    );
    for languages in [json!(["en"]), json!(["sv", "en", "de"])] {
        assert_eq!(
            ask(json!({ "action": "languages", "languages": languages })),
            json!({ "ok": true })
        );
    }
}

#[test]
fn numbers_are_the_same_setting_within_a_billionth_of_the_larger_magnitude() {
    // §21.4 item 6: a rounding difference is no change to keep. Around 10⁹ the tolerance is about
    // 1, so 1 apart is the same and 2 apart is not; below 1 it stays 10⁻⁹. Catches an absolute
    // tolerance (10⁹ and 10⁹ + 1 kept as a change), a tolerance ten times too large, and exact
    // comparison.
    assert!(same(json!(1e9), json!(1e9 + 1.0)));
    assert!(!same(json!(1e9), json!(1e9 + 2.0)));
    assert!(same(json!(1.0), json!(1.0 + 1e-10)));
    assert!(!same(json!(1.0), json!(1.0 + 2e-9)));
    assert!(same(json!(0.0), json!(1e-10)));
    assert!(!same(json!(0.0), json!(2e-9)));
    assert!(same(json!(0.1 + 0.2), json!(0.3)));
    assert!(!same(json!(21.5), json!(22.0)));
}

#[test]
fn an_integer_and_a_real_of_the_same_value_are_the_same_setting() {
    // Catches comparing the JSON numbers as written, where 3 and 3.0 differ.
    assert!(same(json!(3), json!(3.0)));
    assert!(same(json!(-40), json!(-40.0)));
}

#[test]
fn other_values_are_the_same_setting_only_when_equal() {
    // Catches a text compared as a number, and a value the project lacks taken for one.
    assert!(same(json!(true), json!(true)));
    assert!(!same(json!(true), json!(false)));
    assert!(same(json!("Auto"), json!("Auto")));
    assert!(!same(json!("Auto"), json!("auto")));
    assert!(!same(json!("3"), json!(3)));
    assert!(!same(json!(null), json!(0)));
    assert!(!same(json!(null), json!(false)));
    // A project value that is missing is no value.
    let answered = ask(json!({ "action": "sameSetting", "running": 5 }));
    assert_eq!(answered, json!({ "ok": true, "same": false }));
}

#[test]
fn an_unknown_check_is_refused() {
    // Catches a request answered as if a check had passed.
    assert!(answer(json!({ "action": "anything" })).is_err());
}
