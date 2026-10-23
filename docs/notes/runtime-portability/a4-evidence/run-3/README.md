# A4 run 3 evidence

Run 3 completed with 23 required PASS steps and five required FAIL steps; both
advisory steps passed. No step was unrun. See `ledger.tsv` and original logs.

| Native suite | Passed | Failed |
| --- | ---: | ---: |
| Core all features, including doctests | 298 | 1 |
| Core no default features, including doctests | 260 | 1 |
| i686 portable lane | 181 | 0 |
| Hosted unit suite | 3840 | 1 |
| Hosted integration, 64 binaries | 508 | 15 |

The core failure is a profile-rejection fixture failing positive admission.
Hosted failures include invalid ST fixtures, an explicit static-FB producer
restriction, two stale architecture-test source boundaries, and global-reference
lookup incorrectly requiring a call frame. These are unresolved by this run.
Clippy rejected the provenance helper's 103-line main function.

The saved numeric artifact was generated and replayed; the original saved artifact
remained byte-identical. F401/C6 core library checks and Linux/Windows cross-target
warning checks passed. Supply chain, architecture, diagram render/drift, formatting,
diff integrity, mutation tooling (16 tests), mutation contracts (20 tests), and
advisory metadata (1029 records) passed. No firmware link or hardware execution.

All 364 frozen manifest records matched the builder after the batch. Manifest SHA:
`88835281049ef84eea7fd8d27f8e296a1df642c96dad986cb98d0d7d98b71f3a`.
The source archive omits prior evidence to avoid recursive archives; omitted hashes
are recorded in `snapshot-exclusions.json`. This README is indexed; the artifact
index itself and the temporary Git index are excluded.
