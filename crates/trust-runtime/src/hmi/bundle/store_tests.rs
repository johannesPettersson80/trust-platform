//! Specification 39, §16.2.3–5: atomic activation, the previous bundle kept, and rollback.

use std::sync::Arc;

use super::store::{BundleStore, Versions};
use super::HmiBundle;

fn bundle(title: &str) -> HmiBundle {
    HmiBundle {
        files: [
            ("hmi/screen.toml".to_string(), b"format = 1\n".to_vec()),
            (
                "hmi/pages/overview.toml".to_string(),
                format!("title = \"{title}\"\n").into_bytes(),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn project(name: &str, first: &HmiBundle) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-hmi-store-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("a folder");
    first.write(&root).expect("the first bundle");
    root
}

#[test]
fn activating_a_bundle_makes_it_the_hmi_folder_and_keeps_the_previous() {
    let (one, two) = (bundle("One"), bundle("Two"));
    let root = project("activate", &one);
    let store = BundleStore::new(root.clone());
    assert_eq!(
        store.versions(),
        Versions {
            active: Some(one.version()),
            previous: None
        }
    );
    let versions = store.activate(&two).expect("activated");
    assert_eq!(
        versions,
        Versions {
            active: Some(two.version()),
            previous: Some(one.version())
        }
    );
    assert_eq!(store.read(HmiBundle::read).expect("readable"), two);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn activating_the_active_version_again_keeps_the_previous_bundle() {
    let (one, two) = (bundle("One"), bundle("Two"));
    let root = project("again", &one);
    let store = BundleStore::new(root.clone());
    store.activate(&two).expect("activated");
    let versions = store.activate(&two).expect("activated again");
    assert_eq!(
        versions,
        Versions {
            active: Some(two.version()),
            previous: Some(one.version())
        }
    );
    store.rollback().expect("rolled back");
    assert_eq!(store.read(HmiBundle::read).expect("readable"), one);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_rollback_restores_the_previous_bundle() {
    let (one, two) = (bundle("One"), bundle("Two"));
    let root = project("rollback", &one);
    let store = BundleStore::new(root.clone());
    store.activate(&two).expect("activated");
    let versions = store.rollback().expect("rolled back");
    assert_eq!(
        versions,
        Versions {
            active: Some(one.version()),
            previous: Some(two.version())
        }
    );
    assert_eq!(store.read(HmiBundle::read).expect("readable"), one);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn without_a_previous_bundle_a_rollback_changes_nothing() {
    let one = bundle("One");
    let root = project("no-previous", &one);
    let store = BundleStore::new(root.clone());
    let refused = store.rollback().expect_err("nothing to roll back to");
    assert!(refused.contains("previous"), "{refused}");
    assert_eq!(store.read(HmiBundle::read).expect("readable"), one);
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_read_never_sees_a_bundle_half_switched() {
    let (one, two) = (bundle("One"), bundle("Two"));
    let root = project("concurrent", &one);
    let store = Arc::new(BundleStore::new(root.clone()));
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let store = store.clone();
            let (one, two) = (one.clone(), two.clone());
            std::thread::spawn(move || {
                for _ in 0..200 {
                    let read = store.read(HmiBundle::read).expect("readable");
                    assert!(
                        read == one || read == two,
                        "a half-switched read: {:?}",
                        read.files.keys().collect::<Vec<_>>()
                    );
                }
            })
        })
        .collect();
    for round in 0..40 {
        store
            .activate(if round % 2 == 0 { &two } else { &one })
            .expect("activated");
    }
    for reader in readers {
        reader.join().expect("a reader");
    }
    std::fs::remove_dir_all(root).ok();
}
