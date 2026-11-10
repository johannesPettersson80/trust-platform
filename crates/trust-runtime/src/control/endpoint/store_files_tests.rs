//! Unit tests of a program store's runtime files (spec 11, section 6.8.0a; spec 34 §11.2): a runtime
//! started on a store's `current` link keeps them in the store folder, so that they stay with it
//! when an install or a rollback moves `current` to another bundle.

use std::path::{Path, PathBuf};

use crate::deployment::program_store::{replace_symlink, BUNDLES_DIR, CURRENT_LINK};

use super::defaults::platform_default_endpoint;
use super::file::{endpoint_file_path, PublishedEndpoint};
use super::test_support::{runtime_toml, tcp, Scratch};
use super::token_file::token_file_path;
use super::{resolve_control_target, EndpointSource};

/// A program store in `store` with the bundles `a` and `b`, each a project whose control endpoint
/// is configured on port 0, and `current` naming `a`. Returns the `current` link.
fn store_with_two_bundles(store: &Path) -> PathBuf {
    for label in ["a", "b"] {
        let bundle = store.join(BUNDLES_DIR).join(label);
        std::fs::create_dir_all(&bundle).expect("create a bundle");
        std::fs::write(
            bundle.join("runtime.toml"),
            runtime_toml("endpoint = \"tcp://127.0.0.1:0\""),
        )
        .expect("write the bundle's runtime.toml");
    }
    point_current_at(store, "a");
    store.join(CURRENT_LINK)
}

/// Moves `current` to `bundles/<label>`, as an install or a rollback does.
fn point_current_at(store: &Path, label: &str) {
    replace_symlink(
        &store.join(CURRENT_LINK),
        &store.join(BUNDLES_DIR).join(label),
    )
    .expect("point current at a bundle");
}

/// Spec 11, section 6.8.0a: a runtime started on a store's `current` link writes its endpoint file
/// in the store folder, a client given `current` still finds it after an install moved `current`
/// to another bundle, and the runtime's clean exit removes it from there. Wrong behaviour caught:
/// the file is written into the bundle `current` named at the start, so after the install the
/// client finds no file in the new bundle and takes its configured `tcp://127.0.0.1:0`, and the exit
/// leaves the first bundle's file behind.
#[test]
fn a_store_runtime_is_found_through_current_after_an_install_moves_current() {
    let scratch = Scratch::new("store-endpoint-file");
    let current = store_with_two_bundles(scratch.path());
    let published = PublishedEndpoint::publish_with(
        &current,
        &tcp("127.0.0.1:43210"),
        Some("store-token"),
        None,
    )
    .expect("publish the endpoint file");

    point_current_at(scratch.path(), "b");
    let target =
        resolve_control_target(Some(&current), None, None).expect("resolve through current");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:43210");
    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.auth_token.as_deref(), Some("store-token"));
    let store_file = scratch.path().join(".trust-runtime").join("control.json");
    assert_eq!(endpoint_file_path(&current), store_file);

    drop(published);
    assert!(
        !store_file.exists(),
        "the exit removes the store's endpoint file"
    );
    for label in ["a", "b"] {
        let bundle = scratch.path().join(BUNDLES_DIR).join(label);
        assert!(
            !bundle.join(".trust-runtime").exists(),
            "{label} holds no runtime files"
        );
    }
}

/// Spec 11, section 6.8.0a: `<h>`, which names the per-user token file and the fallback socket, is
/// taken over the store folder's canonical path, and the project-local socket lies in the store
/// folder, so neither moves with `current`. Wrong behaviour caught: both follow the bundle that
/// `current` names, so after an install a Windows client looks for another token file and a Unix
/// client for another socket, and reports the runtime not running.
#[test]
fn a_store_runtimes_token_file_and_default_socket_stay_when_current_moves() {
    let scratch = Scratch::new("store-defaults");
    let current = store_with_two_bundles(scratch.path());
    let token_home = Scratch::new("store-token-home");
    let store_token_file =
        token_file_path(token_home.path(), scratch.path()).expect("the store's token file");
    let socket_before = platform_default_endpoint(&current);

    point_current_at(scratch.path(), "b");

    assert_eq!(
        token_file_path(token_home.path(), &current).expect("token file through current"),
        store_token_file
    );
    let socket_after = platform_default_endpoint(&current);
    assert_eq!(
        format!("{socket_after:?}"),
        format!("{socket_before:?}"),
        "the default endpoint does not move with current"
    );
}
