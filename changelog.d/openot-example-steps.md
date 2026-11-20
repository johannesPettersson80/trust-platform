### Fixed
- The OpenOT database examples build and run as their READMEs say. `examples/openot_database` copied only the
  workload and a backend's `runtime.toml`, so `trust-runtime build` failed (`'OotProducer' is not callable`, the
  OpenOT ST library was missing) and `trust-runtime run` refused the copy (`missing io.toml`). Each README now
  fetches the library once with `scripts/checkout_openot_ref.sh`, and the project copy gets `workload/io.toml`
  and a `trust-lsp.toml` naming the library. `examples/openot_multi_program` says to fetch the library too, and
  its copy for another database names the library by its absolute path, which the example's relative path did
  not reach. The PostgreSQL and MySQL READMEs name the database and user the commands use, `trust_logging` and
  `trust_logging_writer`, and the OpenOT authoring page lists the example's six producers and the tests that
  build it.
