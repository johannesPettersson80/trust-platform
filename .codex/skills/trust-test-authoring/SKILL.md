---
name: trust-test-authoring
description: Use for every trust-platform bug fix, feature, refactor, malformed input, runtime safety, VS Code, hardware lab, docs-only, or supply-chain change that needs specification-first test selection, catalog registration, red-green or behavior-lock evidence, and verification-gate routing.
---

# truST Test Authoring

Use this skill before changing behavior or test ownership. Detailed contracts live under
`docs/internal/testing/checklists/plc-verification-program/`; this skill only owns the execution
route.

## Required Route

1. Classify the intent and changed paths before writing a test:

   ```bash
   python3 scripts/plan_tests.py --intent <bugfix|feature|refactor|docs|test-refactor> \
     --changed <path...>
   ```

2. If the planner reports `spec_gap`, update the owning specification, IEC decision, or reviewed
   product decision first. Never invent an expected result in a test.
3. Bind the behavior to an invariant and an active oracle. Register the test in
   `verification/test-catalog.toml`; an unregistered test is a gate failure.
4. For a bug fix or intentional behavior change, run the smallest focused test to its expected
   assertion failure before editing production code. Compile, harness, dependency, timeout, and
   unrelated failures are not red evidence.
5. For proof-mapped tests, create durable red and green evidence with the cataloged command:

   ```bash
   python3 scripts/prove.py red --test <TEST_ID>
   python3 scripts/prove.py green --test <TEST_ID> --paired-red <EVIDENCE_ID>
   ```

6. For a behavior-preserving refactor, establish a green behavior-lock baseline and compare before
   claiming completion. Do not manufacture red behavior for a refactor.
7. Run focused checks continuously. Run heavy `just fmt`, `just clippy`, and `just test-all` on
   `trust-builder` only at the final milestone defined by the active checklist.
8. Before pushing a feature, behavior, or product-test change, rerun the planner with every changed
   path. `missing_tests`, `spec_gap`, and `unmapped` are blocking results even when all implemented
   tests pass:

   ```bash
   python3 scripts/plan_tests.py --intent <intent> --changed <every-changed-path>
   python3 scripts/check_test_catalog_staleness.py
   ```

   The staleness command verifies committed mappings. Separately inspect every new scanner fact and
   either catalog-map it or explicitly queue its denominator review; never infer completeness from
   a passing native suite or from the absence of a catalog row.
9. For VS Code work, run `python3 scripts/check_vscode_test_registration.py`. For hardware or
   protocol features, run the cataloged device-in-the-loop case on the named real topology and
   retain its machine-readable artifact before push. Simulator, compile, and unit evidence do not
   substitute for the real hardware case.
10. Record exact commands, red/green outcomes, catalog/invariant IDs, and local versus
   trust-builder evidence. Do not promote proof beyond the evidence tier actually obtained.

## Scenario Routing

- **bug fix**: spec/oracle, cataloged failing regression, expected red, minimal fix, paired green.
- **refactor**: cataloged behavior-lock baseline and compare; no invented failure.
- **malformed input**: negative case with stable failure kind/code and no panic-only oracle.
- **VS Code**: use `trust-vscode-quality`; register the extension test and prove the real rendered
  surface when behavior is visible.
- **runtime safety**: use `st-lsp-solid`; require the runtime vertical and fault-boundary evidence
  named by the invariant.
- **hardware lab**: use `trust-remote-builder`; do not convert missing hardware or a skipped test
  into passing evidence.
- **docs-only**: run the planner and metadata gate; do not create product proof from prose alone.
- **supply-chain**: use `trust-ci-release-gates` and `trust-release-hygiene`; bind artifact,
  provenance, and public-release claims to durable evidence.

## Final Checks

```bash
python3 scripts/run_verification_focused_tests.py
scripts/verification_metadata_gate.sh
python3 scripts/check_verification_tooling_selftests.py
python3 scripts/check_test_catalog_staleness.py
```

The enforcing pull-request gate must remain read-only, run with `--strict`, and block merge on a
red verification command or an uncataloged changed test.
