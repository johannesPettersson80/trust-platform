# Phase 18 Rust oracle-input and compile-negative closure

Date: 2026-07-29

## Scope and result

This coherent batch completed the source/extraction side of the remaining Rust
assertion-oracle population and repaired the compile-negative fixtures that
could otherwise report false success:

- all 79 inventories with dynamic `unresolved-oracle` reads passed their exact
  focused tests on source checkpoint
  `cbcef0c6d69feacb46a3fd6d6495f94e850ffa54`;
- the 11 assertion-empty inventories were reduced to zero on source checkpoint
  `63532855e2eaedb714469817f8411c56c2743676`;
- the complete remaining extractable population now contains 326 inventories,
  3,539 assertions, zero unresolved helpers, and zero assertion-empty
  identities;
- ten compile-negative Structured Text fixtures are now ordinary programs, so
  they cannot be silently treated as an empty passing test selection;
- unfiltered `trust-dev test` now compiles a project even when discovery finds
  zero test POUs, while `--list` and an explicit filter that selects no tests
  preserve their discovery-only behavior;
- the mapped OSCAT absence test binds both fixture source files and its exact
  compile diagnostic;
- the mapped PLCopen absence test binds and executes all nine selected
  compile-negative fixture projects;
- the OPC UA subscription compile contract now exposes one explicit
  compile-time assertion to the extractor.

This closes assertion extraction and harness false-success debt for this
population. It does not catalog-map the 326 tests or retire the remaining 418
`reviewed_nonmapping` facts. In particular, the 245 denominator records still
labelled `assertion_oracle_unresolved` remain open until their semantic mapping
transactions replace that historical reason; extractor success is not accepted
as mapping proof.

## Exact checkout and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Canonical branch: `plc-verification-program`
- Exact source checkpoint:
  `63532855e2eaedb714469817f8411c56c2743676`
- Clean validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-zero-63532855`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the canonical checkout and verified byte-for-byte. The
canonical and destination hashes were:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills normalized aggregate
  b596ca71b7e269fc7d1911c97d0bba64dcd1f1dd207551d2933a95ba86d2c2a2
```

## Test-first evidence

The compile-negative fixtures and mapped absence tests were green at the
pre-change source checkpoint. After changing the fixtures from
`TEST_PROGRAM` declarations to ordinary `PROGRAM` declarations, the exact
OSCAT test failed for the intended reason: `trust-dev test` discovered zero
tests and returned success without compiling the project. That expected
assertion red established the missing runner behavior.

The minimal runner repair compiles an unfiltered no-test project before
reporting the empty result. The same OSCAT assertion then passed. Expanding the
PLCopen test to the complete nine-project fixture set first exposed an
over-specific enum diagnostic expectation. The fixture-source binding and
diagnostic-class oracle were corrected, then the same test passed across all
nine projects. Compile, dependency, timeout, and unrelated failures were not
counted as red evidence.

## Exact focused proof

On exact clean source checkpoint `63532855`:

```text
cargo test -p trust-dev --bin trust-dev test::tests:: -- --test-threads=1
  21/21 pass

cargo nextest run -p trust-dev --test cli_smoke ... \
  test(=oscat_replaced_upstream_name_remains_absent)
  1/1 pass in 19.937 seconds

cargo nextest run -p trust-runtime --test plcopen_motion_oop_library ... \
  test(=plcopen_motion_deferred_public_surfaces_remain_absent)
  1/1 pass in 23.370 seconds

cargo nextest run -p trust-runtime --test opcua_client_runtime ... \
  test(=opcua_client_subscription_api_surface_is_available_for_phase3_worker)
  1/1 pass in 0.013 seconds

trust-dev test --project crates/trust-runtime/tests/fixtures/init_bench
  project compiles; 0 tests, 0 failures, 0 errors

scripts/verification_assertion_inventory_gate.sh
  8 Python checks + 37 xtask checks = 45/45 pass

python3 scripts/check_diagram_drift.py
  pass
```

The exact live scanner produced 4,401 facts from 764 files with zero warnings
and zero errors. The exhaustive denominator equation was:

```text
4,401 live = 3,983 catalog_mapped + 418 reviewed_nonmapping
0 unreviewed
```

All ten removed fixture discovery identities were absent from the live scan.
The two changed mapped tests were separately re-extracted into 15 assertions
with zero unresolved helpers. Their accepted mapping provenance was then
replaced by hand-reviewed proposal and decision records; fixture/setup
assertions are not counted as product oracles.

## Metadata diagnostics and cadence

The exact planner and catalog-staleness commands remained globally blocked by
the pre-existing repository metadata backlog. The planner reported 1,607
additional metadata findings after its displayed prefix. The staleness report
correctly identified the two changed mapped source closures before their
provenance refresh. Neither result is presented as a passing gate.

The compact machine-readable summary is retained at
`docs/internal/testing/evidence/plc-verification-program/2026-07-29/p18-rust-oracle-closure-326/summary.json`.
The large raw inventories and scanner documents remain on the clean builder
checkout with paths and SHA-256 digests recorded in that summary.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
