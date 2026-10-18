use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(0);

fn project(label: &str) -> PathBuf {
    let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-ads-runtime-enabled-{}-{label}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create ADS runtime project");
    root
}

fn write_runtime_toml(root: &Path, contents: &str) {
    fs::write(root.join("runtime.toml"), contents).expect("write runtime.toml");
}

fn read_runtime_toml(root: &Path) -> String {
    fs::read_to_string(root.join("runtime.toml")).expect("read runtime.toml")
}

#[test]
fn ads_runtime_enabled_in_an_existing_section() {
    // spec 25 §5.8: set enabled, config_path, and worker_tick_interval_ms in [runtime.ads].
    let root = project("existing-section");
    write_runtime_toml(
        &root,
        concat!(
            "[bundle]\n",
            "version = 1\n",
            "\n",
            "[resource]\n",
            "name = \"Simulator\"\n",
            "cycle_interval_ms = 50\n",
            "\n",
            "[runtime.ads]\n",
            "enabled = false\n",
            "config_path = \"old-ads.toml\"\n",
        ),
    );

    let changed =
        ensure_ads_runtime_enabled(root.as_path(), "ads.toml").expect("enable ADS runtime");

    // A wrong implementation that rebuilt the file with toml_edit would reorder or
    // drop the bundle/resource sections; that must fail this exact comparison.
    assert!(changed, "a changed runtime.toml must report changed = true");
    assert_eq!(
        read_runtime_toml(&root),
        concat!(
            "[bundle]\n",
            "version = 1\n",
            "\n",
            "[resource]\n",
            "name = \"Simulator\"\n",
            "cycle_interval_ms = 50\n",
            "\n",
            "[runtime.ads]\n",
            "enabled = true\n",
            "config_path = \"ads.toml\"\n",
            "worker_tick_interval_ms = 20\n",
        )
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ads_runtime_section_added_when_missing() {
    // spec 25 §5.8: the section is added at the end when it is missing.
    let root = project("missing-section");
    write_runtime_toml(&root, "[bundle]\nversion = 1\n");

    let changed =
        ensure_ads_runtime_enabled(root.as_path(), "ads.toml").expect("enable ADS runtime");

    // The ported `ensureAdsRuntimeEnabled` adds a blank line only when the file
    // does not already end with a newline ("\n\n"); here it ends with "\n", so the
    // section header follows the last line directly. A wrong implementation that
    // always inserted a blank line (or none when the file lacked a final newline)
    // fails this exact comparison.
    assert!(changed, "an added section must report changed = true");
    assert_eq!(
        read_runtime_toml(&root),
        concat!(
            "[bundle]\n",
            "version = 1\n",
            "[runtime.ads]\n",
            "enabled = true\n",
            "config_path = \"ads.toml\"\n",
            "worker_tick_interval_ms = 20\n",
        )
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ads_runtime_unchanged_file_is_not_written() {
    // spec 25 §5.8: an already-enabled section keeps its existing worker tick.
    let root = project("unchanged");
    let contents = concat!(
        "[runtime.ads]\n",
        "enabled = true\n",
        "config_path = \"ads.toml\"\n",
        "worker_tick_interval_ms = 20\n",
    );
    write_runtime_toml(&root, contents);

    let changed =
        ensure_ads_runtime_enabled(root.as_path(), "ads.toml").expect("enable ADS runtime");

    // A wrong implementation that always set worker_tick_interval_ms = 20 or rewrote
    // the file would report changed = true or change a byte here.
    assert!(
        !changed,
        "an unchanged runtime.toml must report changed = false"
    );
    assert_eq!(read_runtime_toml(&root), contents);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ads_runtime_missing_runtime_toml_is_refused() {
    // spec 25 §5.8 and `NC/offlineComm.ts:134`: refuse with the ported text.
    let root = project("missing-file");

    let error = ensure_ads_runtime_enabled(root.as_path(), "ads.toml")
        .expect_err("a missing runtime.toml must be refused");

    // A wrong implementation that created runtime.toml or used another message would
    // fail this exact comparison.
    assert_eq!(
        error,
        "ADS tag import wrote the selected tags, but runtime.toml is missing so ADS cannot be enabled automatically."
    );
    assert!(
        !root.join("runtime.toml").exists(),
        "a refusal must not create runtime.toml"
    );
    fs::remove_dir_all(&root).ok();
}
