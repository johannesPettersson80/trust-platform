# Independent source review of root-owned run-3 corrections

Reviewed paths:

- `crates/trust-runtime-core/src/vm/dispatch_refs.rs`
- `crates/trust-runtime/tests/initializer_architecture.rs`
- `crates/verification-cases/examples/refresh_portability_provenance.rs`

Identity `cc93b1e4b1432ca6688e955f31360ebfb5f622444cc4d317e5cb70a9b459e2d1` uses sorted path/NUL/file bytes/NUL.

The Global reference-address arm removes an unnecessary frame requirement while
retaining read-policy checks in peek_ref. Both scalar and compound Global reads
continue through storage; local/instance ownership still requires its frame.
The existing tier-1 behavior assertion is retained unchanged.

Source architecture assertions now inspect the moved core dynamic_ref_index and
the actual register decoder span through collect_block_leaders. Visibility changes
cannot hide the named functions from these checks. No-clone and inline-operand
assertions remain unchanged.

The invariant_source_digest extraction retains the authoritative existing Python
projection, command status handling, UTF-8 decoding and digest shape validation.
It changes structure only; collected source/pin writes still occur after discovery.
No suppression or validation bypass was added. No additional source blocker found.

No builds, tests, formatters, validators or mutation discovery were executed.
The provenance helper's earlier author-owned implementation is not independently
certified by this narrow review of the root's extraction.
