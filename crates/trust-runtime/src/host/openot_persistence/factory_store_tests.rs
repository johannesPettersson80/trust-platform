//! A store runtime's SQLite database (specification 33; specification 11 §6.8.0a): the database
//! that the persistence writes at a relative `path` lies in the folder of the runtime's files, the
//! store folder for a runtime started on a program store's `current` link.

use std::fs;
use std::path::PathBuf;

use super::OpenOtDocumentSink;
use crate::config::{
    OpenOtPersistenceBackend, OpenOtPersistenceConfig, OpenOtSqlitePersistenceConfig,
};
use crate::deployment::program_store::{replace_symlink, BUNDLES_DIR, CURRENT_LINK};

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
    let config = OpenOtPersistenceConfig {
        enabled: true,
        backend: Some(OpenOtPersistenceBackend::Sqlite),
        sqlite: Some(OpenOtSqlitePersistenceConfig {
            path: PathBuf::from("history").join("trust-logging.sqlite3"),
        }),
        ..OpenOtPersistenceConfig::default()
    };

    let sink = OpenOtDocumentSink::open(&config, &current).expect("open the database");
    drop(sink);

    assert!(
        store
            .join("history")
            .join("trust-logging.sqlite3")
            .is_file(),
        "the store folder holds the database"
    );
    assert!(
        !bundle.join("history").exists(),
        "the bundle holds no database"
    );
    let _ = fs::remove_dir_all(&store);
}
