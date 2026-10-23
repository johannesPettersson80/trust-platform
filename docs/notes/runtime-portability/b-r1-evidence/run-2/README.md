# B-R1 software batch, run 2

One consolidated batch, no automatic retry. Source frozen at manifest
`95f0b700daa8f74f9b463724c11afecefe70e66a29ac426e84d9a541c94a2e1d`.
The native behavior suites passed; affected/portable Clippy and architecture failed.
Diagrams and drift were unrun because architecture failed. Hardware remained unrun
because the separately measured optimized firmware exceeded L2 by 34,784 bytes.

Ledger: 42 rows: 32 required passes, 3 required failures, 4 required unrun,
2 advisory passes and 1 retained failed size prerequisite. Compiler diagnostics in
passing compile-fail doctests are expected test output, not extra failures.

All raw top-level logs/commands and the frozen changed-path manifest are retained.
Raw linker maps, binary objects, source worktrees and temporary Git indexes remain
outside Git. No claim that a standalone map report or native allocation estimate is
physical hardware qualification. See ../../b-r1-execution.md for corrections and limits.

Current sources contain nine reviewed, unvalidated post-batch corrections; the logs
refer to the frozen snapshot, not those corrections. No commit or push occurred.
