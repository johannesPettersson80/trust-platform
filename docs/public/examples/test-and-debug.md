# Test And Debug

Use this page when you want a small example that proves scan-cycle behavior,
memory markers, and deterministic checks without a large application around it.
It is the shortest path from "I changed logic" to "I can prove the output."

Budget 10-15 minutes. Read [Scan Cycle](../concepts/scan-cycle.md) first if the
`%M` behavior is new to you. Success means you can explain why the counter value
changes one cycle later than a naive read/write mental model would suggest and
you can rerun the deterministic check after editing the example.

Use this before larger capstones when you want one small behavior to prove
debug, test, and cycle reasoning.

It is intentionally small so failures stay easy to localize.

## Memory Marker Counter

--8<-- "examples/memory_marker_counter/README.md:3"
