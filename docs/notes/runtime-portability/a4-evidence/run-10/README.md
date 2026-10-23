# A4 run 10 — generic counter construction and I/O fixture

The consolidated batch completed: 28 required passes, one required failure and
two advisory passes. Core all-features/portable/i686 passed 316/276/197 tests;
hosted unit passed 3,841. Hosted integration executed 534 passing and two failing
tests, with no filtered or ignored assertions. Both MCU library checks, Clippy,
Windows cross-target warnings, supply chain, architecture, diagrams and formatting
passed. No firmware link or hardware execution occurred.

The integration failures expose (1) shared type validation rejecting the specified
unbound NULL value of generic counter ANY_INT state and (2) the new hierarchical
I/O negative fixture using an invalid bit index before it reaches the unbound-input
API. The real generic-counter test is preserved; the owning policy gets explicit
allowed/rejected-value coverage. The unbound-input fixture needs a valid address.

All 50 raw artifacts match their local/builder hashes. Frozen source digest:
`f04283f6b39067c84478b3e675572b4fd796b994baa4d2832fc00a4c908e4844`.
Resolve raw index paths with external-root.json. Source archives and logs stay
outside git; retained-sha256.json includes this README and excludes itself.
No source was changed during the batch. No A4 commit or push occurred.
