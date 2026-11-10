### Fixed

- A runtime started on a program store's `current` link now keeps the OpenOT persistence's SQLite
  database and InfluxDB spool in the store folder when their paths are relative, so an install, a
  deployment or a rollback no longer starts the persistence on an empty database. The first start of
  this version moves such a database, with its journal files, from the `current` bundle into the
  store folder.
