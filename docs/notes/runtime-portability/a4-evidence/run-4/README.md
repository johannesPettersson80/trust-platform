# A4 run 4 evidence

The complete batch finished with 27 required PASS steps, one required FAIL step
(runtime integration), and two advisory PASS steps. Nothing was skipped.

| Native suite | Passed | Failed |
| --- | ---: | ---: |
| Core all features, including doctests | 299 | 0 |
| Core no default features, including doctests | 261 | 0 |
| i686 portable lane | 181 | 0 |
| Hosted unit suite | 3841 | 0 |
| Hosted integration, 64 binaries | 521 | 2 |

The two failures are retained in `runtime-integration.txt`: the architecture test's
source oracle did not declare the relocated core directory; valid REF(fresh.value)
failed at the first scan with TypeMismatch because artifact-backed path type
resolution did not traverse POU fields. The latter is an execution failure before
warm restart, not evidence of a failed retain transfer. Both remain open in this run.

Numeric and original fixture generation/replay, both MCU library checks, Linux and
Windows warning checks, Clippy, supply chain, architecture, diagram rendering/drift,
formatting, diff checks, 36 mutation-tooling tests and 1029 metadata records passed.
There is no firmware link, physical board or timing/memory qualification.

All 422 frozen records matched the builder after completion. Manifest SHA-256:
`5cf6dc4305451ceeb3c68742ea7c3f81189a4906d454d166813de99154a815c0`.
The source archive omits earlier evidence to avoid recursive archives; omitted
hashes are retained in `snapshot-exclusions.json`. README is indexed; the index
itself and temporary Git index are excluded.
