//! A store runtime's SQLite database (specification 33; specification 11 §6.8.0a): the database
//! that the persistence writes at a relative `path` lies in the folder of the runtime's files, the
//! store folder for a runtime started on a program store's `current` link.

use std::fs;
use std::path::{Path, PathBuf};

use super::OpenOtDocumentSink;
use crate::config::{
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
};
use crate::deployment::program_store::{replace_symlink, BUNDLES_DIR, CURRENT_LINK};

/// The configuration of both tests: a SQLite database at the relative path
/// `history/trust-logging.sqlite3`, as specification 33's example names it.
fn sqlite_at_a_relative_path() -> OpenOtPersistenceConfig {
    OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: PathBuf::from("history").join("trust-logging.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    }
}

fn database_in(folder: &Path) -> PathBuf {
    folder.join("history").join("trust-logging.sqlite3")
}

/// Wrong behaviour caught: the database is made in the bundle that `current` names, so an install,
/// which moves `current` to another bundle, starts the persistence on an empty database and leaves
/// the recorded documents behind with the old bundle.
#[test]
fn a_store_runtimes_sqlite_database_lies_in_the_store_folder() {
    let store = trust_test_support::unique_temp_path("openot-store-sqlite");
    let bundle = store.join(BUNDLES_DIR).join("a");
    fs::create_dir_all(&bundle).expect("create the bundle");
    let current = store.join(CURRENT_LINK);
    replace_symlink(&current, &bundle).expect("point current at the bundle");

    let sink = OpenOtDocumentSink::open(&sqlite_at_a_relative_path(), &current)
        .expect("open the database");
    drop(sink);

    assert!(
        database_in(&store).is_file(),
        "the store folder holds the database"
    );
    assert!(
        !bundle.join("history").exists(),
        "the bundle holds no database"
    );
    let _ = fs::remove_dir_all(&store);
}

/// Wrong behaviour caught: the first start of this version leaves the database that an earlier
/// version made in the `current` bundle behind, so the persistence starts on an empty one; or the
/// move makes the database's folder in the store folder with the process's umask, where the
/// persistence makes it for the runtime's user only: with the umask 022 other users may read the
/// folder, and with 002 the persistence refuses the group-writable folder and does not start.
#[test]
fn a_store_runtimes_start_moves_the_bundles_database_into_a_private_folder() {
    let store = trust_test_support::unique_temp_path("openot-store-sqlite-move");
    let bundle = store.join(BUNDLES_DIR).join("a");
    fs::create_dir_all(&bundle).expect("create the bundle");
    let earlier = OpenOtDocumentSink::open(&sqlite_at_a_relative_path(), &bundle)
        .expect("make the database in the bundle, as an earlier version did");
    drop(earlier);
    assert!(
        database_in(&bundle).is_file(),
        "the bundle holds the database"
    );
    let current = store.join(CURRENT_LINK);
    replace_symlink(&current, &bundle).expect("point current at the bundle");

    let sink = OpenOtDocumentSink::open(&sqlite_at_a_relative_path(), &current)
        .expect("open the moved database");
    drop(sink);

    assert!(
        database_in(&store).is_file(),
        "the store folder holds the database"
    );
    assert!(
        !database_in(&bundle).exists(),
        "the bundle holds the database no more"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(store.join("history"))
            .expect("read the database folder")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o700,
            "the database's folder is the runtime user's only"
        );
    }
    let _ = fs::remove_dir_all(&store);
}
