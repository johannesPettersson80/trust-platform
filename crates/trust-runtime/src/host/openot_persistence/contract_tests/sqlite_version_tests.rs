//! The SQLite that `trust-runtime` bundles holds the fix of advisory RUSTSEC-2026-0459.

// RUSTSEC-2026-0459 (CVE-2026-119375), which has no exception under specification 24, "Dependency Exceptions", so it must
// stay fixed: SQLite before 3.54.0 can return uninitialized page-cache memory for a crafted database and `-wal` pair.
// Worked by hand: SQLite numbers a version as major * 1,000,000 + minor * 1,000 + patch, so 3.54.0 is 3,054,000 and the
// affected 3.53.2 is 3,053,002. Catches a dependency update or a merged lock file that brings an affected SQLite back.
#[test]
fn the_bundled_sqlite_holds_the_fix_of_its_page_cache_advisory() {
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "SQLite {} is affected by RUSTSEC-2026-0459",
        rusqlite::version()
    );
}
