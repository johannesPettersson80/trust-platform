//! The project's simulation speed (specification 46 §6.1 items 2 and 5). The text cases are the
//! extension's former `simulation-speed.test.ts` assertions, moved here with the rule.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-simulation-speed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_missing_or_blank_file_becomes_just_the_simulation_table() {
    assert_eq!(with_time_scale(None, 10), "[simulation]\ntime_scale = 10\n");
    assert_eq!(
        with_time_scale(Some("  \n"), 5),
        "[simulation]\ntime_scale = 5\n"
    );
}

#[test]
fn a_file_without_the_table_gets_it_before_the_first_table_after_the_leading_comments() {
    let before = [
        "# Water heater plant",
        "# Edited by the project",
        "",
        "[[plants]]",
        "id = \"tank\"",
        "kind = \"gravity_tank\"",
        "",
        "[[couplings]]",
        "from = \"pump\"",
        "to = \"tank\"",
        "",
    ]
    .join("\n");
    let after = [
        "# Water heater plant",
        "# Edited by the project",
        "",
        "[simulation]",
        "time_scale = 10",
        "",
        "[[plants]]",
        "id = \"tank\"",
        "kind = \"gravity_tank\"",
        "",
        "[[couplings]]",
        "from = \"pump\"",
        "to = \"tank\"",
        "",
    ]
    .join("\n");
    assert_eq!(with_time_scale(Some(&before), 10), after);
}

#[test]
fn an_existing_time_scale_changes_only_its_value() {
    // Its comment, `enabled` and `[[plants]]` stay. Catches: a rewrite that drops comments.
    let text = "[simulation]\nenabled = true\ntime_scale = 8   # faster than real time\n\n[[plants]]\nid = \"tank\"\n";
    assert_eq!(
        with_time_scale(Some(text), 20),
        "[simulation]\nenabled = true\ntime_scale = 20   # faster than real time\n\n[[plants]]\nid = \"tank\"\n"
    );
    assert_eq!(
        replace_time_scale("  time_scale=3  ", 7),
        "  time_scale=7  ",
        "the indentation, the spacing and blanks after the value stay"
    );
}

#[test]
fn a_table_without_time_scale_gets_the_key_directly_after_its_header() {
    assert_eq!(
        with_time_scale(
            Some("[simulation]\nenabled = true\n\n[[plants]]\nid = \"tank\"\n"),
            5
        ),
        "[simulation]\ntime_scale = 5\nenabled = true\n\n[[plants]]\nid = \"tank\"\n"
    );
}

#[test]
fn real_time_writes_one_and_keeps_the_table() {
    assert_eq!(
        with_time_scale(Some("[simulation]\ntime_scale = 10\n"), 1),
        "[simulation]\ntime_scale = 1\n"
    );
}

#[test]
fn crlf_line_endings_are_kept() {
    assert_eq!(
        with_time_scale(
            Some("[simulation]\r\ntime_scale = 2\r\n[[plants]]\r\nid = 1\r\n"),
            10
        ),
        "[simulation]\r\ntime_scale = 10\r\n[[plants]]\r\nid = 1\r\n"
    );
}

#[test]
fn a_file_without_any_table_gets_the_table_at_its_end() {
    // A file of comments only: the table follows after one blank line.
    assert_eq!(
        with_time_scale(Some("# plants come later"), 2),
        "# plants come later\n\n[simulation]\ntime_scale = 2\n"
    );
}

#[test]
fn the_saved_speed_falls_back_to_real_time() {
    assert_eq!(saved_speed(Some("[simulation]\ntime_scale = 20\n")), 20);
    assert_eq!(saved_speed(Some("[[plants]]\nid = \"tank\"\n")), 1);
    assert_eq!(saved_speed(Some("[simulation]\ntime_scale = 500\n")), 1);
    assert_eq!(saved_speed(Some("[simulation]\ntime_scale = 2.5\n")), 1);
    assert_eq!(saved_speed(None), 1);
    assert_eq!(
        saved_speed(Some(
            "[simulation]\nenabled = true\n[[plants]]\ntime_scale = 9\n"
        )),
        1,
        "a time_scale outside the [simulation] table is not the speed"
    );
}

#[test]
fn the_chooser_offers_seven_speeds() {
    assert_eq!(SIMULATION_SPEEDS, [1, 2, 5, 10, 20, 50, 100]);
}

#[test]
fn a_save_reaches_the_runtime_only_for_a_running_selected_simulator() {
    // Specification 35 A2.4: the selected target decides where a speed goes. Catches: config.set sent
    // to a stopped simulator, to a remote runtime or to a simulator without an endpoint.
    let project = Scratch::new();
    std::fs::write(project.0.join("runtime.toml"), "").expect("runtime.toml");
    let save = |target: Value| {
        answer(json!({ "action": "save", "folder": project.0, "speed": 10, "target": target }))
            .expect("answer")
    };
    let applied =
        save(json!({ "kind": "simulator", "status": "running", "endpoint": "tcp://127.0.0.1:9" }));
    assert_eq!(
        applied["live"],
        json!({ "request": "config.set", "params": { "simulation.time_scale": 10 } })
    );
    for target in [
        json!({ "kind": "simulator", "status": "stopped", "endpoint": "tcp://127.0.0.1:9" }),
        json!({ "kind": "remote", "status": "running", "endpoint": "tcp://10.0.0.5:7000" }),
        json!({ "kind": "simulator", "status": "running", "endpoint": " " }),
    ] {
        assert_eq!(save(target.clone())["live"], Value::Null, "{target}");
    }
    let read = answer(json!({ "action": "read", "folder": project.0 })).expect("read");
    assert_eq!(
        read["speed"],
        json!(10),
        "the save went to the project's simulation.toml"
    );
    assert_eq!(
        std::fs::read_to_string(project.0.join("simulation.toml")).expect("file"),
        "[simulation]\ntime_scale = 10\n"
    );
}

#[test]
fn simulation_toml_lies_beside_runtime_toml_in_a_bundle_folder() {
    // The workspace folder has no runtime.toml; `bundle/` has one. Catches: the speed saved where the
    // runtime never reads it.
    let workspace = Scratch::new();
    std::fs::create_dir_all(workspace.0.join("bundle")).expect("bundle");
    std::fs::write(workspace.0.join("bundle/runtime.toml"), "").expect("runtime.toml");
    let saved =
        answer(json!({ "action": "save", "folder": workspace.0, "speed": 5 })).expect("save");
    assert_eq!(
        saved["file"],
        json!(workspace
            .0
            .join("bundle")
            .join("simulation.toml")
            .display()
            .to_string())
    );
    assert!(workspace.0.join("bundle/simulation.toml").is_file());
}

#[test]
fn a_speed_outside_one_to_one_hundred_is_refused_and_nothing_is_written() {
    let project = Scratch::new();
    for speed in [0, 101, -3] {
        let error = answer(json!({ "action": "save", "folder": project.0, "speed": speed }))
            .expect_err("refused");
        assert!(error.contains("must be >= 1 and <= 100"), "{error}");
    }
    assert!(!project.0.join("simulation.toml").exists());
}
