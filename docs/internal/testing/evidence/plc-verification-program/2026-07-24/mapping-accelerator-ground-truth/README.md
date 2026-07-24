# Mapping accelerator ground-truth pilot

This retained focused pilot exercises two existing catalog-mapped Rust tests at
the exact source commit recorded inside the generated artifacts:

- one execution-guided candidate is semantically exact;
- one execution-guided candidate is false because shared startup coverage
  dominates the trace.

The false signal remains unreviewed and quarantined. Independent
assertion-to-oracle review accepts only the exact signed-integer mapping. The
generated qualification report therefore records one exact reviewed acceptance,
zero false accepted mappings, and `creates_catalog_mappings = false`.

Retained generated artifacts:

- `test-coverage-trace.json`: focused per-test execution attribution;
- `mapping-candidates.json`: quarantined execution-guided signals;
- `semantic-review.json`: generated assertion-to-behavior review record;
- `ground-truth-report.json`: qualification against existing reviewed mappings.

The pilot did not run a repository-wide scanner, a broad coverage campaign, or
the full test suite.
