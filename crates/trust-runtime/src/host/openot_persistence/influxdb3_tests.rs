use super::*;

#[test]
fn reopening_generation_1_spool_reapplies_connection_local_durability_pragmas() {
    static CASE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "trust-logging-spool-reopen-{}-{}",
        std::process::id(),
        CASE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create isolated spool directory");
    let path = root.join("spool.sqlite3");
    {
        let connection = Connection::open(&path).expect("create spool");
        initialize_spool_schema(&connection).expect("initialize spool");
    }
    let reopened = Connection::open(&path).expect("reopen spool");
    reopened
        .execute_batch(
            "PRAGMA foreign_keys=OFF; PRAGMA synchronous=NORMAL; PRAGMA journal_mode=DELETE;",
        )
        .expect("simulate connection-local defaults before validation");
    initialize_spool_schema(&reopened).expect("validate reopened spool");
    let foreign_keys: u32 = reopened
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("read foreign-key mode");
    let synchronous: u32 = reopened
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("read synchronous mode");
    let journal_mode: String = reopened
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("read journal mode");
    assert_eq!(
        foreign_keys, 1,
        "foreign keys must be enabled per connection"
    );
    assert_eq!(synchronous, 2, "spool durability requires synchronous=FULL");
    assert_eq!(journal_mode, "wal", "spool durability requires WAL mode");
    drop(reopened);
    std::fs::remove_dir_all(root).expect("remove isolated spool directory");
}

#[test]
fn generation_1_spool_with_changed_index_fails_closed() {
    static CASE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "trust-logging-spool-contract-{}-{}",
        std::process::id(),
        CASE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create isolated spool directory");
    let path = root.join("spool.sqlite3");
    let connection = Connection::open(&path).expect("open isolated spool");
    initialize_spool_schema(&connection).expect("initialize spool");
    connection
        .execute_batch("DROP INDEX logging_delivery_spool_pending;")
        .expect("damage required generation-1 index");

    let error = initialize_spool_schema(&connection)
        .expect_err("changed generation-1 index must fail closed");
    assert!(
        error
            .to_string()
            .contains("incompatible pre-release spool schema"),
        "unexpected compatibility error: {error}"
    );
    drop(connection);
    std::fs::remove_dir_all(root).expect("remove isolated spool directory");
}
