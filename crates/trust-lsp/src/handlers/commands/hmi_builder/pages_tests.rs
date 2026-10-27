//! The Pages panel's rules (specification 39, §3.3 item 8, §4 item 6). Each expected file is the
//! canonical text of §15.2 written out by hand: `format`, `title`, `order`, then `hidden`.

use serde_json::{json, Value};

use super::*;

/// A page file with its title and, when it has one, its order.
fn page(title: &str, order: Option<u32>) -> String {
    match order {
        Some(order) => format!("format = 1\ntitle = \"{title}\"\norder = {order}\n"),
        None => format!("format = 1\ntitle = \"{title}\"\n"),
    }
}

/// Overview first and Trends second by their order; Alarms without one.
fn project() -> Value {
    json!({
        "overview": page("Overview", Some(0)),
        "trends": page("Trends", Some(1)),
        "alarms": page("Alarms", None),
    })
}

fn ask(request: Value) -> Value {
    answer(request).expect("the request is answered")
}

fn listed(pages: Value) -> Vec<String> {
    ask(json!({ "action": "list", "pages": pages }))["pages"]
        .as_array()
        .expect("a page list")
        .iter()
        .map(|entry| entry["name"].as_str().expect("a name").to_string())
        .collect()
}

#[test]
fn the_pages_are_listed_by_order_then_by_name_and_pages_without_an_order_last() {
    // Catches a page without an order listed first (an absent order read as 0), and pages of the
    // same order left in the order the files came.
    assert_eq!(listed(project()), ["overview", "trends", "alarms"]);
    let tied = json!({
        "b": page("B", Some(1)),
        "a": page("A", Some(1)),
        "c": page("C", None),
        "d": page("D", Some(0)),
        "aa": page("AA", None),
    });
    assert_eq!(listed(tied), ["d", "a", "b", "aa", "c"]);
}

#[test]
fn each_entry_has_the_title_and_whether_the_navigation_hides_the_page() {
    // Catches the file name shown instead of the title, and a hidden page left out of the
    // editor's list (only the runtime's navigation leaves it out, §4 item 6).
    let pages = json!({
        "overview": page("Overview", Some(0)),
        "service": "format = 1\ntitle = \"Service\"\nhidden = true\n",
    });
    assert_eq!(
        ask(json!({ "action": "list", "pages": pages }))["pages"],
        json!([
            { "name": "overview", "title": "Overview", "hidden": false },
            { "name": "service", "title": "Service", "hidden": true },
        ])
    );
}

#[test]
fn a_page_that_does_not_load_is_listed_by_its_name_after_the_ordered_pages() {
    // Catches a broken page dropped from the list, so the user could not reach its errors.
    let pages = json!({ "broken": "format = 1\ntitle =\n", "overview": page("Overview", Some(0)) });
    assert_eq!(
        ask(json!({ "action": "list", "pages": pages }))["pages"],
        json!([
            { "name": "overview", "title": "Overview", "hidden": false },
            { "name": "broken", "title": "broken", "hidden": false },
        ])
    );
}

#[test]
fn a_new_page_takes_the_first_free_number_and_goes_last() {
    // Three pages: the new one is fourth, order 3, and Alarms, without an order, gets 2. Catches
    // the new page placed first or given no order, and Alarms left without one.
    let added = ask(json!({ "action": "add", "pages": project() }));
    assert_eq!(added["name"], "page-1");
    assert_eq!(
        added["write"],
        json!([{ "name": "alarms", "text": page("Alarms", Some(2)) }])
    );
    assert_eq!(
        added["create"],
        json!([{ "name": "page-1", "text": page("Page 1", Some(3)) }])
    );
    // Catches a number counted from the pages (page-3, taken) instead of the first free one.
    let numbered = json!({ "page-1": page("Page 1", Some(0)), "page-3": page("Page 3", Some(1)) });
    let added = ask(json!({ "action": "add", "pages": numbered }));
    assert_eq!(added["name"], "page-2");
    assert_eq!(added["write"], json!([]));
    assert_eq!(
        added["create"],
        json!([{ "name": "page-2", "text": page("Page 2", Some(2)) }])
    );
}

#[test]
fn a_new_title_is_trimmed_and_an_empty_one_changes_nothing() {
    // Catches the spaces kept in the title, and a page given an empty title.
    let renamed = ask(json!({
        "action": "rename", "pages": project(), "name": "trends", "title": "  Trends 24 h  ",
    }));
    assert_eq!(
        renamed["write"],
        json!([{ "name": "trends", "text": page("Trends 24 h", Some(1)) }])
    );
    let blank =
        ask(json!({ "action": "rename", "pages": project(), "name": "trends", "title": "  " }));
    assert_eq!(blank["write"], json!([]));
    // The same title writes nothing, so the file keeps every byte.
    let same =
        ask(json!({ "action": "rename", "pages": project(), "name": "trends", "title": "Trends" }));
    assert_eq!(same["write"], json!([]));
    // Catches a rename that creates a page the project does not have.
    assert!(answer(
        json!({ "action": "rename", "pages": project(), "name": "nothing", "title": "X" })
    )
    .is_err());
}

#[test]
fn a_copy_goes_right_after_its_page_and_the_later_pages_move_on() {
    // Overview (0) is copied: the copy is 1, Trends moves from 1 to 2 and Alarms gets 3. Catches
    // the copy placed last, and the later pages left where the copy now is.
    let copied = ask(json!({ "action": "duplicate", "pages": project(), "name": "overview" }));
    assert_eq!(copied["name"], "overview-copy");
    assert_eq!(
        copied["create"],
        json!([{ "name": "overview-copy", "text": page("Overview (copy)", Some(1)) }])
    );
    assert_eq!(
        copied["write"],
        json!([
            { "name": "trends", "text": page("Trends", Some(2)) },
            { "name": "alarms", "text": page("Alarms", Some(3)) },
        ])
    );
}

#[test]
fn a_copy_takes_the_next_free_copy_name_and_keeps_everything_else() {
    // Catches a copy written over an earlier copy, and a copy that loses the page's icon, group,
    // hidden flag or objects.
    let original = "format = 1\ntitle = \"Pumps\"\norder = 0\nicon = \"pump\"\ngroup = \"Process\"\nhidden = true\n\n[[object]]\nid = \"value-1\"\ntype = \"value\"\nat = [40, 40]\nsize = [160, 48]\n";
    let pages = json!({
        "pumps": original,
        "pumps-copy": page("Pumps (copy)", Some(1)),
    });
    let copied = ask(json!({ "action": "duplicate", "pages": pages, "name": "pumps" }));
    assert_eq!(copied["name"], "pumps-copy-2");
    let expected = original
        .replace("title = \"Pumps\"", "title = \"Pumps (copy)\"")
        .replace("order = 0", "order = 1");
    assert_eq!(
        copied["create"],
        json!([{ "name": "pumps-copy-2", "text": expected }])
    );
    // The earlier copy moves from 1 to 2.
    assert_eq!(
        copied["write"],
        json!([{ "name": "pumps-copy", "text": page("Pumps (copy)", Some(2)) }])
    );
}

#[test]
fn a_moved_page_goes_before_the_other_page_or_last() {
    // Alarms before Overview: Alarms 0, Overview 1, Trends 2. Catches the page placed after the
    // other one, and the pages it passes left with their old order.
    let moved = ask(json!({
        "action": "move", "pages": project(), "name": "alarms", "before": "overview",
    }));
    assert_eq!(
        moved["write"],
        json!([
            { "name": "alarms", "text": page("Alarms", Some(0)) },
            { "name": "overview", "text": page("Overview", Some(1)) },
            { "name": "trends", "text": page("Trends", Some(2)) },
        ])
    );
    // Trends last: Overview keeps 0 and is not written; Alarms 1, Trends 2.
    let last = json!([
        { "name": "alarms", "text": page("Alarms", Some(1)) },
        { "name": "trends", "text": page("Trends", Some(2)) },
    ]);
    let moved =
        ask(json!({ "action": "move", "pages": project(), "name": "trends", "before": null }));
    assert_eq!(moved["write"], last);
    // A page that is not there counts as the end.
    let moved = ask(json!({
        "action": "move", "pages": project(), "name": "trends", "before": "nothing",
    }));
    assert_eq!(moved["write"], last);
    // Before itself, or a page the project lacks, changes nothing.
    for (name, before) in [("trends", "trends"), ("nothing", "overview")] {
        let moved =
            ask(json!({ "action": "move", "pages": project(), "name": name, "before": before }));
        assert_eq!(moved["write"], json!([]), "{name} before {before}");
    }
}

#[test]
fn hiding_writes_hidden_and_showing_removes_it() {
    // Catches `hidden = false` written into the file, and a hidden page shown again keeping it.
    let hidden = "format = 1\ntitle = \"Alarms\"\nhidden = true\n";
    let hid =
        ask(json!({ "action": "hide", "pages": project(), "name": "alarms", "hidden": true }));
    assert_eq!(hid["write"], json!([{ "name": "alarms", "text": hidden }]));
    let pages = json!({ "alarms": hidden });
    let shown = ask(json!({ "action": "hide", "pages": pages, "name": "alarms", "hidden": false }));
    assert_eq!(
        shown["write"],
        json!([{ "name": "alarms", "text": page("Alarms", None) }])
    );
    let again = ask(json!({ "action": "hide", "pages": pages, "name": "alarms", "hidden": true }));
    assert_eq!(again["write"], json!([]));
}

#[test]
fn a_page_that_does_not_load_is_left_as_it_is() {
    // Catches a broken page overwritten while the others are numbered, or copied.
    let pages = json!({ "broken": "format = 1\ntitle =\n", "overview": page("Overview", Some(0)) });
    let added = ask(json!({ "action": "add", "pages": pages }));
    assert_eq!(added["write"], json!([]));
    assert_eq!(
        added["create"],
        json!([{ "name": "page-1", "text": page("Page 1", Some(2)) }])
    );
    let copied = ask(json!({ "action": "duplicate", "pages": pages, "name": "broken" }));
    assert_eq!(
        (&copied["write"], &copied["create"]),
        (&json!([]), &json!([]))
    );
    let hidden = ask(json!({ "action": "hide", "pages": pages, "name": "broken", "hidden": true }));
    assert_eq!(hidden["write"], json!([]));
}

/// Whether a name is one of `names`.
fn taken(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |name| names.contains(&name)
}

#[test]
fn the_first_free_name_is_the_stem_then_numbered_from_2() {
    // Catches numbering from 1 (`overview-1`) and a taken name used again.
    assert_eq!(free_name("overview", taken(&[])), "overview");
    assert_eq!(free_name("overview", taken(&["overview"])), "overview-2");
    assert_eq!(
        free_name("overview", taken(&["overview", "overview-2"])),
        "overview-3"
    );
    assert_eq!(free_name("overview", taken(&["overview-2"])), "overview");
}
