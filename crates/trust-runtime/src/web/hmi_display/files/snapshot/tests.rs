//! Specification 39, §21.5 item 4: the active bundle is read and hashed once when it is activated
//! and again only when its files change; requests use the snapshot.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::current;
use crate::hmi::bundle::store::BundleStore;
use crate::hmi::bundle::HmiBundle;

fn temp(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-snapshot-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("a folder");
    root
}

fn write(project: &Path, path: &str, text: &str) {
    let file = project.join(path);
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
    std::fs::write(file, text).expect("write");
}

fn bundle(title: &str) -> HmiBundle {
    HmiBundle {
        files: [
            ("hmi/screen.toml".to_string(), b"format = 1\n".to_vec()),
            (
                "hmi/pages/overview.toml".to_string(),
                format!("format = 1\ntitle = \"{title}\"\n").into_bytes(),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

/// The version the files on disk have now, read and hashed in full.
fn on_disk(project: &Path) -> String {
    HmiBundle::read(project).expect("readable").short_version()
}

/// Gives a file a modification time of its own, whatever the clock's resolution.
fn touch(project: &Path, path: &str, seconds_ago: u64) {
    let file = std::fs::File::options()
        .write(true)
        .open(project.join(path))
        .expect("the file");
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds_ago))
        .expect("a modification time");
}

#[test]
fn repeated_requests_use_the_snapshot_without_reading_the_bundle_again() {
    let project = temp("repeated");
    bundle("One").write(&project).expect("the bundle");
    let first = current(&project);
    assert_eq!(first.version, on_disk(&project));
    for _ in 0..10 {
        let again = current(&project);
        assert!(
            std::sync::Arc::ptr_eq(&first, &again),
            "the same snapshot, not a new read"
        );
    }
    assert_eq!(current(&project).reads, 1, "read once");
    std::fs::remove_dir_all(project).ok();
}

#[test]
fn a_deploy_and_a_rollback_refresh_the_snapshot_once_each() {
    let project = temp("activation");
    let (one, two) = (bundle("One"), bundle("Two"));
    one.write(&project).expect("the first bundle");
    let store = BundleStore::new(project.clone());
    let before = current(&project);
    assert_eq!(before.version, one.short_version());
    store.activate(&two).expect("deployed");
    let deployed = current(&project);
    assert_eq!(deployed.version, two.short_version());
    assert_eq!(deployed.reads, before.reads + 1);
    assert_eq!(current(&project).reads, deployed.reads, "and then kept");
    store.rollback().expect("rolled back");
    let rolled_back = current(&project);
    assert_eq!(rolled_back.version, one.short_version());
    assert_eq!(rolled_back.reads, deployed.reads + 1);
    assert_eq!(current(&project).reads, rolled_back.reads, "and then kept");
    std::fs::remove_dir_all(project).ok();
}

#[test]
fn a_file_edited_added_or_removed_by_hand_refreshes_the_snapshot() {
    let project = temp("hand-edits");
    bundle("One").write(&project).expect("the bundle");
    let mut reads = current(&project).reads;
    let mut changed = |project: &Path, what: &str| {
        let snapshot = current(project);
        assert_eq!(snapshot.reads, reads + 1, "{what} is read once");
        assert_eq!(snapshot.version, on_disk(project), "{what}");
        assert_eq!(current(project).reads, snapshot.reads, "{what} then kept");
        reads = snapshot.reads;
    };
    write(
        &project,
        "hmi/pages/overview.toml",
        "format = 1\ntitle = \"Edited by hand\"\n",
    );
    changed(&project, "an edit");
    write(
        &project,
        "hmi/pages/trends.toml",
        "format = 1\ntitle = \"Trends\"\n",
    );
    changed(&project, "a new page");
    std::fs::remove_file(project.join("hmi/pages/trends.toml")).expect("removed");
    changed(&project, "a removed page");
    // Same size, other content: the modification time tells it.
    write(
        &project,
        "hmi/pages/overview.toml",
        "format = 1\ntitle = \"Edited by HAND\"\n",
    );
    touch(&project, "hmi/pages/overview.toml", 60);
    changed(&project, "an edit of the same size");
    std::fs::remove_dir_all(project).ok();
}

#[test]
fn a_library_panel_is_part_of_the_snapshot_and_its_edit_refreshes_it() {
    let project = temp("libraries");
    write(
        &project,
        "trust-lsp.toml",
        "[dependencies]\nequipment = { path = \"lib/equipment\" }\n",
    );
    write(
        &project,
        "lib/equipment/trust-lsp.toml",
        "[package]\nversion = \"0.1.0\"\n",
    );
    // A library has sources; the build's resolver, which the bundle uses, needs its `src/`.
    write(
        &project,
        "lib/equipment/src/equipment.st",
        "(* sources *)\n",
    );
    write(
        &project,
        "lib/equipment/hmi/panels/pump.toml",
        "shipped pump",
    );
    write(&project, "hmi/screen.toml", "format = 1\n");
    let first = current(&project);
    assert_eq!(first.version, on_disk(&project));
    write(
        &project,
        "lib/equipment/hmi/panels/pump.toml",
        "shipped pump, edited",
    );
    let edited = current(&project);
    assert_eq!(edited.reads, first.reads + 1);
    assert_eq!(edited.version, on_disk(&project));
    assert_ne!(edited.version, first.version);
    std::fs::remove_dir_all(project).ok();
}

#[test]
fn a_project_without_an_hmi_folder_has_the_empty_bundles_version() {
    let project = temp("empty");
    let snapshot = current(&project);
    assert_eq!(snapshot.version, HmiBundle::default().short_version());
    assert_eq!(current(&project).reads, snapshot.reads);
    std::fs::remove_dir_all(project).ok();
}
