### Security

- The SQLite that the runtime bundles for its databases is now 3.54.1 (`rusqlite` 0.41.1). SQLite before 3.54.0 could
  return uninitialized memory when it opened a database together with a crafted `-wal` file of another page size
  (RUSTSEC-2026-0459, CVE-2026-119375).
