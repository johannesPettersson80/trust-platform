# Phase 18 Windows URI assertion-oracle closure

Date: 2026-07-29

## Scope and result

This batch closes the final two `assertion_oracle_unresolved` discoveries on
their native Windows compilation path:

```text
DISC_06F21CDCB3CA7DD26BAF  uri_to_path_decodes_drive_letter
DISC_A14C9116E67DFD0D61A8  path_to_uri_strips_extended_length_prefix
```

Both tests are catalog-mapped to the existing reviewed
`LSP_CORE_FEATURES_001#behavior:12` URI/path contract and its
`SPEC_LSP_CONTRACT_001#663-uri-and-virtual-document-handling` authority.
The accepted assertion review is exhaustive:

```text
tests:                         2
unique assertions:             7
direct product oracles:        6
setup or preconditions:        1
non-oracle observations:       0
assertion-behavior bindings:   6
```

The atomic mapping transaction moves the exact denominator from
4,401 = 4,226 `catalog_mapped` + 175 `reviewed_nonmapping` to
4,401 = 4,228 `catalog_mapped` + 173 `reviewed_nonmapping`. No unresolved
assertion-oracle discovery remains. The 173 remaining facts are explicit
governed inventories:

```text
not_catalogable_product_oracle     83
gate_inventory_owned               59
ignored_test_registry_owned        23
fuzz_program_owned                  8
```

This is P18-003/P18-005 slice progress only. Those rows remain globally open
while the four governed inventory classes, 203 `gap_open` coverage cells,
dynamic reachability, mutation adequacy, and the final broad candidate gate
remain outstanding.

## Exact checkout and platform lane

- Canonical local checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch:
  `plc-verification-program`
- Exact selection and execution checkpoint:
  `481d0458ac7893efd0379a45ecf181ac8ada376b`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Remote primary source mode:
  detached HEAD at `3479807f26f56b8347d588d658cd9301d6735050`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-windows-uri-481d0458`
- Validation destination mode:
  detached HEAD at the exact selection checkpoint
- Builder host:
  `scena-rust-builder`

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the remote primary source before validation. Exact
verification passed:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills
  diff -qr against the remote primary source: no differences
```

The required destination copies of `trust-test-authoring`,
`trust-remote-builder`, `st-lsp-solid`, `trust-release-hygiene`, and
`trust-lsp-iec` were read before the focused work. The platform lane used:

```text
target:  x86_64-pc-windows-gnu
runner:  wine-10.0 (Ubuntu 10.0~repack-12ubuntu1)
rustc:   1.97.0
cargo:   1.97.0
```

Wine was installed on the shared builder for this lane. The post-install disk
preflight retained 28 GiB free on `/home/johannes` and 1.9 GiB on `/tmp`,
which was sufficient for the focused Windows target build. No broad gate ran.

## Native target execution

Each exact test was compiled for `x86_64-pc-windows-gnu` and executed through
Wine:

```text
uri_to_path_decodes_drive_letter
  selected 1, passed 1, failed 0, filtered 182
  cold compile and run: 3m56s
  stdout sha256 7cc93197296bb1375953b1dcdbca8d27158ffdddeaa5baae9b1cbf6a8271d351
  stderr sha256 925fec16e58e7293c54287b144683f9527144c0e85aefcce6953e877995ded29

path_to_uri_strips_extended_length_prefix
  selected 1, passed 1, failed 0, filtered 182
  reused build and run: 1.25s
  stdout sha256 cdc927fe5aaca2805c1afec4df2ce80be47c42a42b3c6bc3b6f45f81caa52c56
  stderr sha256 27670efbbbe89037ddaa96428d40fa83e458684686c08f1223fc5ecd7ff8f98c
```

The retained machine-readable execution summary is
`target/gate-artifacts/verification/p18-windows-uri-2-481d0458/execution-summary.json`
with digest
`32dd31b4e03e04292813dff63b7afb4d792f60eb167a153d24530531b6d57218`.

## Cross-platform extractor defect: red and green

Running the assertion extractor as a Windows binary exposed a validator bug:
host-runtime absolute reads such as Linux `/proc/self/stat`, Windows
drive-rooted paths, and UNC/rooted paths could be mistaken for
repository-relative runtime fixtures when their syntax came from a
cross-compiled source closure.

The mandatory planner was run before the fix with `--intent bugfix` and the
two changed xtask paths. It stopped at the known repository-wide metadata
backlog with exit 6. Its empty stdout and summarized stderr digests are:

```text
extractor-planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
extractor-planner.stderr.txt
  f4eaf0154162f2b11ca7508e6e2acba7ac33e78e72bea935e1907307e4b0228a
```

The new focused validator test
`repository_extraction_ignores_cross_platform_absolute_host_reads` first
failed for the expected assertion reason:

```text
cross-platform absolute host reads must not enter fixture closure
0 passed, 1 failed, 126 filtered
stdout sha256 801c35d7307cf2ec7595c2e32c3ccbb83f4c42877cbb9b5f79fb2ff2db7be704
stderr sha256 2924fac4d74716987450647c22f07719f55c48e0fd9f01267b69cf46cd803a69
```

The minimum fix makes the external-oracle extractor recognize native,
POSIX-rooted, Windows drive-rooted, and Windows rooted/UNC paths before it
attempts repository fixture resolution. The same focused test then passed:

```text
1 passed, 0 failed, 126 filtered
stdout sha256 545837aa35a53f0182f74e4e0af5399dd0f1eb3120efb0c98166726180e2ebab
stderr sha256 85ea1c59b230a1c72acd8667fbf4de26c7724b028fe58c4e12015b355affb543
```

The verification-inventory specification and
`VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` invariant now state this
cross-platform host-path boundary. This is verification tooling behavior, not
a product LSP behavior change, and it does not require an IEC decision,
IEC deviation, changelog entry, or version bump.

An initial scoped assertion-inventory gate invocation was blocked because the
isolated validation worktree did not contain the ignored VS Code
`node_modules/typescript` dependency. This was a harness setup failure, not
the required assertion red. The destination was linked to the remote primary
checkout's existing ignored `node_modules`, after which the scoped gate
passed:

```text
Python adapter tests:             8/8 pass
xtask test_assertions tests:     38/38 pass, 89 filtered
```

## Extraction, mapping, and independent review

The Windows-native extractor resolves all syntax and helper closure for both
platform-gated tests:

```text
assertions-2.json
  43f346493f6f27973eb699020d7b001098a9209e931db1d954bc2970b86b5f96
assignments-2.json
  0ca7e85a4aa9234cb14d4ce1bd1bf97ae5dc74b1e0f497edec538213e8962cc9
live-scan.json
  b60eb5a6d2a3032abfba6a818917e2bd47e145fac1c14b6404e8e18705236fa3
```

The atomic transaction accepted exactly the two intended discoveries and
seven assertion decisions. The focused mapping suite passes 72/72. An
independent root-record audit proves:

```text
introduced discovery mappings:       2
added proposal/decision root records: 4
unchanged pre-existing root records:  7,050
```

The repository-wide staleness gate remains red with 1,620 accepted global
Phase 18 metadata findings. Isolation over both discovery IDs and both
accepted catalog IDs finds zero target-linked findings. The retained
staleness artifact digest is
`45fc605ff356aa891fa1d61214066d83585082e979878638bacce913294241c4`.
That global red remains explicit program debt and is not reported as a
passing gate.

The machine-readable review lock is
`verification/p18/review-locks/windows-uri-oracle-2-closure-lock.json` with
digest
`d9bd7ba9af52c13a1fa904f2b7fe7a79260b21cc25b88344fe03cc9dc6160657`.
All nine lock-bound metadata digests and all 20 lock-bound artifact digests
match their live files.

## Staged validation

The mandatory complete changed-path planner was run with `--intent bugfix` and
every one of the 12 changed paths in this batch, including the canonical board,
evidence, and review lock. It exits 6 before emitting a plan because the
repository-wide metadata validator remains red with the pre-existing Phase 18
findings; the summarized stderr reports the same VM/compiler findings and
1,759 additional findings as the prior accepted batches. Planner stdout is
empty. The retained artifact digests are:

```text
complete-planner.stdout.txt
  e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
complete-planner.stderr.txt
  f4eaf0154162f2b11ca7508e6e2acba7ac33e78e72bea935e1907307e4b0228a
```

The planner's global red is retained as open Phase 18 debt and is not treated
as a passing result. The exact four-identifier staleness isolation described
above still finds zero findings for this accepted batch.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remains
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`.
Those user changes were neither overwritten nor included in this transaction.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
