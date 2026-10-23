# A4 run 5 — passing scope evidence

All 28 required steps and both advisory steps passed on the builder. No skipped,
filtered or automatically retried test is claimed as acceptance. The exact commands,
environment and exits are retained in `commands.txt`, `environment.txt` and `ledger.tsv`.

| Native suite | Passed | Failed |
| --- | ---: | ---: |
| Core all features, including doctests | 301 | 0 |
| Core no default features, including doctests | 263 | 0 |
| i686 portable lane | 183 | 0 |
| Hosted unit suite | 3841 | 0 |
| Hosted integration, 64 binaries | 525 | 0 |

The four Rust provenance-helper and 36 Python mutation-tooling assertions also
passed. Clippy, host/Windows warning checks, F401/C6 no-default library checks,
no-dev feature graphs, supply chain, architecture, diagrams/drift, formatting and
diff integrity passed. Advisory metadata validated 1029 records. This is not a
mutation campaign, firmware link, physical-board run or all-platform qualification.

## Frozen source and fixtures

Branch `feat/runtime-portability-a4`, uncommitted on base
`77b91381fcf1b1850611f88b397bbfe8d45e3523`. The 478-record source manifest SHA-256 is
`c14b104c6b93d047148b87af4beb26a1fb20d920687dc8f26011f0cdbc8a9458`.
All records matched the builder after completion; tested source was copied back
before closeout-only documentation changes. The archive excludes earlier evidence
to avoid recursive archives; exclusions retain hashes in `snapshot-exclusions.json`.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| program-v2.stbc | 6164 | `0a09a7190f26bbe3481ca7ae5b7f16a130ae4dcb2ee21937b456dd8581a87e5a` |
| numeric-v2.stbc | 5488 | `66f64b2793e7a45988c9ff93bbac5dbf219282fb576f0b3e63103ec9c0a05588` |
| expected-a4-trace.csv | 2292 | `26364f808213465e5c3bf93c5319c9ec63edb0c71c7252a79d68c8e12f12a451` |

The saved primary artifact replays on x64 and the selected i686 lane; numeric
checks are finite native reference/threshold assertions, not arbitrary bit-identity
proof. MCU evidence is library compilation only. No A4 commit, push, release guard
or hardware work was performed.

## Complete batch history

| Run | Required PASS | Required FAIL | Required UNRUN |
| --- | ---: | ---: | ---: |
| 1 | 10 | 12 | 4 |
| 2 | 16 | 8 | 2 |
| 3 | 23 | 5 | 0 |
| 4 | 27 | 1 | 0 |
| 5 | 28 | 0 | 0 |

Earlier ledgers and source archives remain in sibling `run-1` through `run-4`
directories. The execution record identifies their root causes and reviewed
corrections; they are not relabeled as successful runs. Independent source reviews
are retained in `../../a4-review/`, with the final evidence reconciliation there.
The artifact index covers every file in this directory, including this README,
except the index itself; the temporary Git metadata index is intentionally omitted.
