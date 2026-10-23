# A4 run 7 — warning-denied compilation failures

The complete consolidated batch finished without a command retry. Required steps:
17 passed, 10 failed compilation and 2 fixture-dependent steps were unrun. Both
advisories passed. Core/host behavior assertions did not execute. The provenance
helper's four assertions and Python mutation tooling passed; supply chain,
architecture, diagrams/drift, formatting and diff integrity passed. Metadata
validated 1029 records.

The prior five compile errors are resolved. This run exposed hosted-only derived
debug/name lookup and register-profiling data in no_std, a test/host-only budget
accessor, an obsolete declaration helper and a missing enum-variant comment.
Corrections preserve actual portable decoding/type metadata and gate only hosted
consumers; no warning suppression or passing behavior claim is permitted.

Frozen source: 584 records, SHA-256 `fcf05bff2ed1c9fa0cec6b3308f3f1ad752bf81ebc9a9767459f60a426acd5c6`.
All 48 external raw artifacts match their recorded hashes. Resolve original
`artifact-sha256.json` paths through `external-root.json`; source archives and
logs remain outside git. `retained-sha256.json` covers this retained record,
excluding itself. No A4 commit, push or hardware run occurred.
