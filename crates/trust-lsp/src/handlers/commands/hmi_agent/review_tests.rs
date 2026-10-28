//! Closed-form tests of the object-level review of an edit (specification 43 AT-05). New tests of
//! `review.rs`; the TypeScript suite checks the review through the service only.

use serde_json::json;
use trust_runtime::hmi::page_files::HmiPage;

use super::super::edits::page_from_json;
use super::describe_page_changes;

/// A page with the given objects, each `(id, x)` placed at height 0 with size 100 by 100.
fn page(title: &str, objects: &[(&str, i64)]) -> HmiPage {
    let objects: Vec<_> = objects
        .iter()
        .map(|(id, x)| {
            json!({"id": id, "type": "tank", "at": [x, 0], "size": [100, 100], "rotation": 0, "bind": {}})
        })
        .collect();
    page_from_json(&json!({"title": title, "object": objects, "pipe": []}))
        .expect("the page is valid")
}

#[test]
fn an_added_removed_and_moved_object_are_reported_against_their_identities() {
    let before = page("Process", &[("a", 0), ("b", 200)]);
    let after = page("Process", &[("a", 8), ("c", 400)]);
    let changes = describe_page_changes(Some(&before), &after);
    assert_eq!(changes.added, vec!["c".to_owned()]);
    assert_eq!(changes.removed, vec!["b".to_owned()]);
    assert_eq!(changes.modified.len(), 1);
    assert_eq!(changes.modified[0].id, "a");
    assert_eq!(changes.modified[0].properties, vec!["at".to_owned()]);
    assert!(
        changes.page_properties.is_empty(),
        "the title did not change"
    );
    assert!(!changes.order_changed);
}

#[test]
fn the_same_objects_in_another_order_change_the_order_and_nothing_else() {
    let before = page("Process", &[("a", 0), ("b", 200)]);
    let after = page("Process", &[("b", 200), ("a", 0)]);
    let changes = describe_page_changes(Some(&before), &after);
    assert!(changes.order_changed);
    assert!(changes.added.is_empty() && changes.removed.is_empty() && changes.modified.is_empty());
}

#[test]
fn a_changed_title_is_a_page_property() {
    let before = page("Process", &[("a", 0)]);
    let after = page("Renamed", &[("a", 0)]);
    let changes = describe_page_changes(Some(&before), &after);
    assert_eq!(changes.page_properties, vec!["title".to_owned()]);
    assert!(changes.modified.is_empty());
}

#[test]
fn without_a_before_page_every_object_is_added() {
    let after = page("Process", &[("a", 0), ("b", 200)]);
    let changes = describe_page_changes(None, &after);
    assert_eq!(changes.added, vec!["a".to_owned(), "b".to_owned()]);
    assert!(changes.removed.is_empty());
    assert!(changes.page_properties.contains(&"title".to_owned()));
}
