# A2 release integration

The original verified A2 commit is `9f62fd09181c222ae6f8802128600708e3a304ea`,
based on A1 `78d4e641f589734a55b9b3b25981a722d8396e62`. Its original worktree,
three later dirty canonical guard files, and all evidence remain untouched.
Integration is prepared in `/home/johannes/projects/trust-platform-portability-a2-integration`,
branch `integrate/runtime-portability-a2`, on released main
`258fe24b8c3706d566f5f05c25cfcd75e5b297c3`. PR #130 merged reviewed A1 candidate
`bc98519a15c18d7f5766492b8c1b9be94bff9ded`; its v0.24.71 release, main CI,
asset checksums, five Marketplace targets and local/builder cleanup audits passed.
The merge tree equals that candidate tree, so moving this prepared A2 branch
to main changed its base identity without changing product-source bytes.
Canonical AGENTS/CLAUDE and the complete skills directory were manually copied
from `/home/johannes/projects/trust-platform`; all 21 files match locally and on
the separately bootstrapped builder checkout.

The user authorized A1 then A2 integration, independent review, consolidated
validation, push, guarded merge and release, with necessary reviewed correction
cycles. Final source preparation passed and candidate
`e461e0f9d6d36b9e95a66183134cb482c07e7635` was committed locally. Its first
integration guard/supplement batch failed two required checks; corrections are
prepared but unvalidated. No A2 push has occurred. The release target is 0.24.72,
following main's completed 0.24.71 release.

## Complete overlap and source mapping

Applying only the original A2 commit with no commit produced two textual conflicts:
changelog organization and the current checklist checkpoint. Both are resolved by
preserving upstream/A1 changes and the original A2 evidence, with a current
integration checkpoint. Diagram manifest, specification 34 and test catalog
merged automatically and were independently reviewed against both parents.
No Rust path overlaps the A1 integration changes. All 80 added/changed/deleted
crate paths from the original A2 commit match its bytes/deletion state exactly
before final preparation. The standalone mapping is retained in
`/home/johannes/projects/.artifacts/runtime-portability-a2-integration/source-mapping.json`.

Preserve the upstream Salsa dereferences, security fixes, fleet shutdown fix,
public specification projection, current release guard and mounted-volume lease
helpers. A2 adds only its existing exact crc32fast 1.5.0 portable edge/feature
split, not another security upgrade. Hosted runtime and PLCopen explicitly enable
crc32fast/std. Version changes affect the workspace package, its 15
local lockfile packages, and the two extension root version files.

Do not carry A3/STBC 2.0 or A4 execution changes into this candidate. Shared 1.x
loading/validation, bounded validation and the documented LOAD_NULL owner fix are
A2's existing reviewed scope. Historical A2 runs 6–8 remain allocated scope proof;
they cannot substitute for the new committed integration artifact.

## One consolidated release validation batch

A1 release and the stable-main transition are complete. The full integration
source and command-map review found no product-code blocker; its three
bookkeeping findings were corrected and rereviewed. Coordinate the builder heavy
slot with A4 and inspect external workloads. Use the mounted approved target
root, task-owned TMPDIR, current Cargo job configuration and the lease helper.
Record toolchain versions and selected-filesystem free space; preserve the guard's
80 GiB floor and warm-target policy. Reuse the idle, leased
`/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a1-integration`
cache rather than forcing another cold build. Native compiler overrides apply only to native
commands; every cross command explicitly unsets CC and CXX.

The preparation phase runs final stable-toolchain formatting, including the touched
host `register_ir/tests/support.rs` and `bytecode_vm_core/positive_paths.rs` include
fragments. Refresh only active planned mutation source digests/selectors with actual
cargo-mutants discovery if formatting moves them, using the existing A2 helper
`/home/johannes/projects/.artifacts/runtime-portability-a2-integration/refresh-mutation-selectors-6.py`
(SHA256 `84e05c825ce7c5d62bd7c3593c8ddc88df0180ea8119fbc7df558ceb8946ed82`),
retained byte-for-byte from the original A2 preparation artifacts. Preserve exact
expression-selector identity; only uniquely identified whole-function selectors may
follow changed line positions. Do not fabricate discovery or rewrite historical
measurement artifacts. Trace-case contracts and pins are unchanged by integration.
Run architecture/full-map before canonical diagram generation; copy back and review
all generated output before committing the final source. The guard repeats its own
mandatory exact-SHA architecture check, but there is only one full workspace test run.

| Proof | Command owner and configuration |
|---|---|
| Exact committed source, canonical rules, cleanliness, strict smoke and final artifact | `release_candidate_guard.py prepare --intent feature` |
| Native workspace unit/integration/doc tests, runtime vertical, LSP/debug, hosted bytecode compatibility and standard-feature core tests | Guard `just test-all`; no separate duplicated hosted/core default suite |
| VS Code lint/compile/tests and capture-lifecycle proof | Guard stages, since extension version changes |
| Clippy, native/Windows runtime warning checks, supply chain, architecture and MP parity | Guard stages |
| F401/C6 portable library graphs | Leased `env -u CC -u CXX cargo +1.95.0 check --locked -p trust-runtime-core --no-default-features --target TARGET` for thumbv7em-none-eabihf and riscv32imac-unknown-none-elf; no firmware claim |
| No-dev portable feature evidence | Same target pair, `cargo +1.95.0 tree --locked -p trust-runtime-core --no-default-features --target TARGET -e features,no-dev` |
| Isolated no-default native behavior | `cargo +1.95.0 test --locked --no-fail-fast -p trust-runtime-core --no-default-features --test portable_foundations --test bytecode_container --test bytecode_decode_resource_bounds --test bytecode_sections --test bytecode_portable_load --test bytecode_validation_budget --test bytecode_validation_scale -- --test-threads=1` |
| Actual 32-bit arithmetic/validation | Same isolated test list plus `--lib --target i686-unknown-linux-musl`; explicit Rust 1.95 rust-lld and `-C link-self-contained=yes`; no system multilib installation |
| Mutation adapter/provenance/schema tooling | `python3 -m unittest scripts.verification.bytecode_validator_mutation_tests scripts.verification.mutation_program_contract_tests scripts.verification.mutation_program_report_tests scripts.verification.gate_inventory_tests scripts.verification.metadata_validator.suite_contracts_tests scripts.verification.fuzz_program_source_contract_tests`; discovery/planned records are not mutation adequacy proof |
| Remaining dependency-resolution pre-push steps | IEC/path hygiene, diagram-workflow tests, corrected Windows GNU LSP test compilation, and eight mesh/TLS stability iterations with one attempt each; reuse the reviewed A1 command environment and deduplicate guard-owned steps |
| Public documentation | Media inventory, public IA/link/example-link checks, strict MkDocs, assets and search; separate exact-SHA docs checkout with canonical bootstrap so generation cannot dirty guard source |
| Diagram and retained-evidence integrity | Drift check against reviewed generated output and retained A2 artifact hashes; historical files remain byte-exact |
| Verification metadata | Advisory validator; honest status only, never a product or release blocker inferred from metadata alone |

Final preparation must be copied back and independently reviewed before the
local commit. The exact-SHA guard must leave that committed tree unchanged.
Keep source frozen through every reachable independent step; preserve failed and
unrun stages. A further correction means the complete known set, review, commit
and one consolidated follow-up under the user's standing instruction, never an
unmodified automatic retry. No push precedes passing required proof.

After push, collect the entire current-head CI/review set, including artifacts
from green retry-rescued jobs. Guarded merge and complete tag/Release/Latest/assets/
Marketplace verification precede the next version merge. Native Windows/macOS,
MCU compilation and physical board execution remain distinct evidence classes.

Reviewed external orchestration is retained under
`/home/johannes/projects/.artifacts/runtime-portability-a2-integration/`:
`source-preparation.sh` SHA256
`e83832409c02b1d7e7e8e8bafdbd3c1862ccad5dab5fc40ba7adfc7b6cdc1fa1`,
`supplement.sh` SHA256
`793c638d0cb8831acd1817ce9914afc2d8734e7c368fb2f18c50a53a38edb1d2`,
and `check-retained-evidence-6.py` SHA256
`631b64902729b74fa0ac880bf3b3d312da2881c09b9eb4a9fb58c394b0d968e1`.
The preparation script passed all seven steps: environment, workspace formatting,
fragment formatting, real selector discovery, architecture/full-map, canonical
diagrams and source-diff retention. The supplemental script subsequently ran as part of batch 1 (results below).
The preparation helper refreshes
selectors only after both formatting steps pass; independent architecture still
runs after other failures, while rendering requires architecture success. Capture
both default and explicit Rust/Cargo 1.95 versions in preflight. Use the ordinary
committed index for the exact-SHA batch, never an inherited `GIT_INDEX_FILE`.

Preparation evidence is retained in the same artifact directory under `run-1/`.
Its `preparation-ledger.tsv` has seven exit-zero rows. `prepared-source.json`
and the pre-preparation manifest both contain 269 records with aggregate SHA256
`a926d0548fc680555b83685a225350ca9cce8b68a99bd0b6bd619401198cfb51`:
formatting, selector discovery and rendering changed no source bytes. Existing
source reviews therefore still cover the product and generated output. Only
this note and the checklist were subsequently updated to record preparation;
root independently reviewed them before candidate e461e0f9d was committed.
This preparation does not claim native-test, firmware or exact-SHA guard success.

## First integration batch and prepared correction

The complete batch at `e461e0f9d6d36b9e95a66183134cb482c07e7635` is retained
under the artifact directory's `run-1/`, including `guard-artifact.json`,
`guard-logs/`, both supplemental ledgers and `FAILURE-LEDGER.md`. Source stayed
frozen until all reachable checks finished. No retry occurred.

The full workspace run passed 6,977 tests, failed one architecture oracle and
ignored 16 before Cargo stopped. The oracle searched for the removed
`opcode_operand_len_for_lowering` shim to delimit `decode_pou`; the prepared
correction uses the actual next decoder entry point, `collect_block_leaders`,
while preserving both no-Vec/no-to_vec assertions. Later workspace binaries
and the guard's subsequent MP-001/final-clean stages were unrun. The separate
supplement's final-clean check passed; it does not repair the failed artifact.

The mutation tooling ran 84 tests, but the 18 report tests were blocked in class
setup by two historical IO proof rows incorrectly bound to the current contract.
The prepared metadata change sets only their `proof_contract_binding` to
`source_revision`. Commits `ba719ec1bff31245ad3fcd78d8ea83d76cc33930` and
`79be3970b18c1d6fee34d546d51d539d06de9b76` contain the recorded case-file hash
`5651bab7a19964e2f3e037947628d2fd32c8b6a737fea523b53a5ba0aead009b`.
All measured dates, results, contract/artifact hashes and case digests are preserved;
this does not claim a current measurement. `historical-io-source-map.json` records
the source comparison. The same stale bindings account for the planner/catalog
and final metadata advisories.

Passing batch proof includes VS Code (519 tests), formatting, native/Windows
warnings, supply chain, architecture/full-map and Clippy; isolated no_std tests
(58) and i686-musl tests (172); F401/C6 core compilation and no-dev feature trees;
Windows LSP, all eight one-attempt TLS iterations, hygiene and diagram checks;
all 132 retained evidence hashes; and all 11 strict-docs steps. These do not close
the two failed requirements or establish MCU firmware/hardware execution.

After independent correction review and builder-slot coordination, prepare the
final formatter output, review any changes and commit the correction. A new
consolidated exact-SHA guard must run the complete mandatory workspace/VS Code
proof, including the formerly unreached suites and MP-001 stage. Its distinct
supplement is `python3 -m unittest scripts.verification.mutation_program_report_tests
scripts.verification.metadata_validator.evidence_proof_tests`, plus advisory metadata.
Reuse the unchanged portable/i686/MCU, TLS, Windows-LSP, diagrams and public-docs
proof from batch 1; their source/configuration inputs are unchanged. No automatic
rerun, source edit during execution, or push without passing required proof is allowed.

## Second integration batch and original-proof restoration

Candidate `e01c9250820c21852f960dcb14479a872ed64005` completed its exact-SHA
release guard successfully: 8,392 native/doc tests passed, zero failed and 24
were ignored; VS Code passed 519 tests. MP-001 and final clean-tree checks passed.
The separate report/proof supplement passed 45 tests but failed report-class
setup, leaving 18 report tests unrun. Advisory metadata reported the same cause.
The candidate is therefore not ready to push despite its passing guard artifact.
All results, commands and source identities are retained under `run-2/`.

The first correction checked the historical case-file bytes but missed that
commit `dd476a78f` had rewritten both July I/O proof-contract digests without
changing their recorded run IDs, source revisions or measured results. Original
baseline record commit `79be3970b18c1d6fee34d546d51d539d06de9b76` and comparison
record commit `d3a0c799607c858eedc0fdb30b69712d07d49783` both contain digest
`sha256:4fd0f346be3c4eaf3b85e83ffbc03ae89946ddd4b8ef91e671cef27a94680241`.
Reconstructing the complete catalog and invariant contract at each recorded
measurement revision yields that same original digest. The contract projection
and hashing implementations are byte-identical between those revisions and today.
`run-2/historical-original-proof-audit.json` retains the original records and
reconstruction details.

The prepared correction restores exactly those two original measured digest
values while retaining `source_revision` binding. It preserves the run IDs,
dates, source commits, result summaries, artifact hashes and case-file hashes;
it neither fabricates a new measurement nor binds historical proof to current
behavior. No validator rule, assertion or metadata requirement is relaxed.

After independent review, commit this metadata/documentation correction and
coordinate one consolidated exact-SHA guard plus the same focused report/proof
supplement and advisory metadata check. Keep all unchanged batch-1 supplemental
proof. A4 owns the builder while this correction is prepared; no new validation
has run for the restored metadata.

## Third integration batch and current-proof lifecycle correction

Candidate `4f5a8fb1b06ab863ec20261fba6bdeea62dc6ebb` passed its complete exact-SHA
guard: 8,392 native/doc tests, zero failed, 24 ignored; 519 VS Code tests;
MP-001 and final clean-tree checks. The historical contract digest errors are
resolved. The focused supplement again passed 45 tests but left 18 report tests
unrun because current invariant promotion still claimed `implemented` / `G1`
using only historical proof. Advisory metadata reported the same lifecycle error.
All commands and failures are retained under `run-3/`; no push occurred.

The complete source audit now covers invariant validation, promotion evidence,
seed lifecycle, catalog/evidence references, semantic digest projection and the
mutation report's full-validator prerequisite. Existing schema supports the honest
current state: `RT_SAFE_IO_001` becomes `gap_open` / `S0`, with
`current_targeted_contract_proof` explicitly missing. Its implementation, native
assertions, behavior, coverage statements, written oracle and historical evidence
remain intact. The seed remains `execution_ready`, catalog entries remain mapped,
and no gate, test or validator is weakened. These lifecycle fields are excluded
from semantic digests, so the current case contract and static pins are unchanged.
This is a proof-status correction, not a product defect or new specification gap.

The successful broad native suite is recorded above; it is not relabeled as a
new targeted provenance measurement. After concrete independent review, commit
this complete lifecycle correction and coordinate one exact-SHA guard with the
same focused report/proof supplement and advisory validator. Keep the unchanged
passing batch-1 supplementary proof. No validation has run on this correction.
