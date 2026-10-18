//! The golden views: the JSON of each scenario's view, which the webview's render tests read
//! (design §4.5). A run with `TRUST_WRITE_DEVICES_VIEW_FIXTURES` set writes them to
//! `crates/trust-devices/tests/fixtures/devices_view/`; every other run compares them byte for
//! byte. The files are never written by hand.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use super::scenarios;

/// Set to write the fixtures instead of comparing them.
const WRITE: &str = "TRUST_WRITE_DEVICES_VIEW_FIXTURES";

/// The command that writes them.
const WRITE_COMMAND: &str =
    "TRUST_WRITE_DEVICES_VIEW_FIXTURES=1 cargo test -p trust-devices --lib golden_views_match_the_fixtures";

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devices_view")
}

/// The fixture text of a view: pretty JSON with a final newline.
fn text(view: &crate::view::types::DevicesView) -> String {
    let json = serde_json::to_string_pretty(view).expect("a view serializes to JSON");
    format!("{json}\n")
}

#[test]
fn golden_views_match_the_fixtures() {
    let directory = directory();
    let scenarios = scenarios::all();
    if std::env::var_os(WRITE).is_some() {
        fs::create_dir_all(&directory).expect("create the fixture folder");
        for (name, view) in &scenarios {
            fs::write(directory.join(format!("{name}.json")), text(view)).expect("write a fixture");
        }
        return;
    }
    let mut problems = Vec::new();
    for (name, view) in &scenarios {
        let path = directory.join(format!("{name}.json"));
        match fs::read_to_string(&path) {
            Ok(fixture) if fixture == text(view) => {}
            Ok(_) => problems.push(format!("{name}.json differs from the view")),
            Err(error) => problems.push(format!("{name}.json cannot be read: {error}")),
        }
    }
    let names: BTreeSet<String> = scenarios
        .iter()
        .map(|(name, _)| format!("{name}.json"))
        .collect();
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file = entry.file_name().to_string_lossy().into_owned();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && !names.contains(&file)
            {
                problems.push(format!("{file} belongs to no scenario"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "the golden views in {} do not match:\n  {}\nWrite them with `{WRITE_COMMAND}` and review the diff.",
        directory.display(),
        problems.join("\n  ")
    );
}

#[test]
fn golden_views_are_deterministic_and_show_no_secret_or_socket_path() {
    for ((name, first), (_, second)) in scenarios::all().iter().zip(scenarios::all()) {
        let shown = text(first);
        assert_eq!(shown, text(&second), "{name} is the same view every time");
        assert!(
            !shown.contains("/tmp/trust-"),
            "{name} shows a local socket's path"
        );
        assert!(!shown.contains("secret"), "{name} shows a secret");
    }
}
