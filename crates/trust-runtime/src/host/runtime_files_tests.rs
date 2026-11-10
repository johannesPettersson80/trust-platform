//! Where a runtime keeps the files it writes (specification 11, section 6.8.0a): a store runtime's
//! file that an earlier version left in the `current` bundle moves into the store folder at the
//! start, with the files that belong to it.

use super::*;
use crate::deployment::program_store::{replace_symlink, BUNDLES_DIR, CURRENT_LINK};

/// Wrong behaviour caught: a database moved into the store folder without its journal, which still
/// holds writes that the database file does not; a companion that does not exist created; or the
/// bundle keeping a copy.
#[test]
fn a_store_runtimes_file_moves_into_the_store_folder_with_its_companions() {
    let store = trust_test_support::unique_temp_path("runtime-files-companions");
    let bundle = store.join(BUNDLES_DIR).join("a");
    fs::create_dir_all(bundle.join("data")).expect("create the bundle's data folder");
    let current = store.join(CURRENT_LINK);
    replace_symlink(&current, &bundle).expect("point current at the bundle");
    fs::write(bundle.join("data/tags.sqlite"), b"database").expect("write the database");
    fs::write(bundle.join("data/tags.sqlite-wal"), b"journal").expect("write its journal");

    let path = prepare_runtime_file(&current, Path::new("data/tags.sqlite"), &["-wal", "-shm"])
        .expect("prepare the database");

    assert_eq!(path, store.join("data").join("tags.sqlite"));
    assert_eq!(fs::read(&path).expect("the moved database"), b"database");
    assert_eq!(
        fs::read(store.join("data").join("tags.sqlite-wal")).expect("the moved journal"),
        b"journal"
    );
    assert!(
        !bundle.join("data/tags.sqlite").exists(),
        "the bundle keeps no database"
    );
    assert!(
        !bundle.join("data/tags.sqlite-wal").exists(),
        "the bundle keeps no journal"
    );
    assert!(
        !store.join("data").join("tags.sqlite-shm").exists(),
        "a companion that did not exist is not made"
    );
    let _ = fs::remove_dir_all(&store);
}
