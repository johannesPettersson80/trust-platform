# Conformance

Conformance suite map: checked corpus, standards-facing claims, and evidence
surface for language/runtime compatibility.

For day-to-day language questions, start with the specification index. Release
or compatibility claims belong next to the evidence surface that backs them.

Public claims should point at a suite, corpus, or reference page instead of
marketing text.

## Suite Overview

--8<-- "conformance/README.md:3"

## Public Proof Surface

The current CI conformance gate runs the suite twice, normalizes timestamps and
durations, diffs the summaries, validates the summary contract, and uploads both
machine-readable JSON and human-readable Markdown reports as CI artifacts.

The committed `conformance/expected/` artifacts are the runtime baseline.
Generated files under `conformance/reports/` are not part of the public docs
source; use CI artifacts for run reports.

## Related

- [Benchmarks](benchmarks.md)
- [Specifications overview](specifications/index.md)
- [Program examples](../examples/index.md)
