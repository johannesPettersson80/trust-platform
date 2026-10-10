# B-R1 correction batch run 3

Owner authorization: “do it” after the explicit correction-batch question.
Preparation and validation ran once on trust-builder, 10 October 2026,
17:06:28–17:11:59 UTC. No firmware link, device execution or commit.

All native suites passed: core all-features 352, portable 312, hosted unit 3,841,
and runtime vertical 28. Both Clippy lanes, F401/C6 library checks, host/Windows
cross-warning checks, architecture, provenance tooling, formatting, artifact parity
and advisory metadata passed. Ledger: 26 PASS (24 required, two advisory), one FAIL,
one UNRUN. Diagram rendering failed with PlantUML exit 200 at source line 289;
diagram drift was not run. No test assertion failed.

Frozen manifest: 303 source/deletion records, SHA-256
`09d7a5da97080b84c13aee943f9eeea870f65cdab4aca896f17fdabaadc80a3e`.
The nine-file independent correction review predates formatting. The sole formatting
change collapsed one closure without changing tokens; reconciliation is retained.
No lock, provenance binding or saved application changed during preparation.

Raw output is retained outside Git under projects/.artifacts/runtime-portability-b/run-3.
The failed renderer's generated SVGs are retained there under rendered/. They have
not replaced the checked-in diagrams: the VM SVG is an error image. Builder source
still matches the tested snapshot except that generated error SVG. The local diagram
source has a prepared note-syntax correction, unrendered. This batch proves no
firmware fit: the last measured candidate overflowed L2 by 34,784 bytes.

The artifact index covers this README and retained command/log/script/identity files.
Historical size and run-2 evidence remain separate; no old result is relabelled.
