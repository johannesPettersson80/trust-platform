# Current A4 Rust review coverage before run 11

This is a current source inventory, not a replacement for historical review hashes
or a claim of final validation. Run 11 has not launched. No tests, builds,
formatters or validators were run to prepare this review artifact.

`run-11-current-rust-review-coverage.json` pins every current dirty Rust path and
every Rust deletion: 239 records. The base is
77b91381fcf1b1850611f88b397bbfe8d45e3523. Classification is conservative:

- 35 deleted source files;
- 3 byte-identical moves from deleted base files;
- 29 adapted moves, including imports, compiler-independent types and API docs;
- 84 modified existing files;
- 88 authored additions or substantial extractions.

Every present file has its current SHA-256. Every deletion has its base SHA-256.
Move records identify the deleted base source. Adapted/documented moves are not
called verbatim. Authored groups cover prepared admission/indexes, shared execution,
engine/lifetimes/transactions, storage/value/I/O, standard functions, host adapters
and register tiers, native tests/fixtures, and crate/lifecycle wiring.

The initial whole-scope independent review is retained under its original identity.
Later changes map to the separate run-6 through run-10 focused independent reviews.
Current bindings are:

- 115 present files match the complete independent review byte for byte;
- 68 match the run-6 formatted source boundary and remain unchanged through the
  subsequent retained candidate manifests;
- 20 match the applicable focused independent correction review byte for byte;
- one index-test file differs from its focused review only by the import order:
  swapping the adjacent alloc::vec and crate::bytecode imports back reproduces
  the historical reviewed SHA-256 exactly;
- all 35 deletions match the reviewed deletion inventory.

The two files added to the dirty scope after the initial full review are the host
sizeof wrapper and vm_resource_limit_cases.rs; their owning caller corrections are
covered by the run-8 and run-9 independent reviews. No current dirty Rust path is
unlisted or lacks an identified review chain. The formatted-boundary classification
records the actual retained bytes and preparation provenance; it does not silently
replace a pre-format review identity with a post-format hash.

All final counter-policy changes and the hierarchical negative fixture remain
pinned to the run-10 correction review. Their new assertions are authored but not
yet executed. The earlier passing core/unit counts and the failed integration
history retain their original meanings. After the next batch, final reconciliation
must compare its frozen Rust/fixture bytes, formatting changes, complete ledger and
raw artifacts independently; this inventory does not pre-approve those results.
