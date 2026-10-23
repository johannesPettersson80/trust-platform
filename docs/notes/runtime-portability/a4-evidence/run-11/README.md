# A4 run 11 — all planned checks passed

The consolidated batch completed without command retries: all 29 required steps
and both advisory steps passed. Source remained frozen throughout validation.

| Native suite | Passed | Failed / ignored / filtered |
|---|---:|---|
| Core all-features, including doctests | 318 | 0 / 0 / 0 |
| Core no-default-features | 278 | 0 / 0 / 0 |
| i686 musl library and selected suites | 199 | 0 / 0 / 0 |
| Hosted runtime unit | 3,841 | 0 / 0 / 0 |
| Hosted integration, 67 binaries | 536 | 0 / 0 / 0 |

The four Rust provenance-helper assertions and Python tooling suites (16 and 20)
also passed. Both isolated F401/C6 core checks and no-dev feature graphs, affected
Clippy, Windows cross-target warnings, supply chain, architecture, diagram render
and drift, formatting (including include fragments), and diff integrity passed.
Metadata validated 1,029 records. Unexecuted mutation/fuzz programs reported by the
architecture inventory are not promoted into measured evidence.

The integration corpus includes actual STBC 2.0 counter/timer/trigger/bistable and
falling-edge execution, hierarchical I/O, shared-budget/deadline fault withholding,
3,000 declarations with 2,000 stores, 2,000 input bindings, sparse 1 MiB marker
publication, and zero allocated reference paths in hosted field loads/stores.
Core tests execute a forged initializer write outside staging and reject it.
These finite regressions do not establish worst-case execution time, allocation-free
operation for every opcode, physical memory fit, or functional-safety certification.

Frozen source: 643 records, SHA-256
`abc68ced75de1b39e1510028c4ce9669dd435ce310fa5be2892d9e2d4599e83b`.
All 50 raw artifacts match their local/builder hashes. Source archives and full
logs are external; resolve artifact-sha256.json paths through external-root.json.
retained-sha256.json includes this README, excluding itself. Failed runs 1–10 retain
their original outcomes and identities. [Final independent reconciliation](../../a4-review/run-11-final-independent-acceptance.md)
passed separately; a passing ledger alone is not an independent review.

No A4 commit, push, release guard, firmware link, physical board execution,
measured STM32 stack/heap/scan timing or native Windows/macOS run is claimed.
Those remain separately scoped work. The A1/A2 release work is independent.
