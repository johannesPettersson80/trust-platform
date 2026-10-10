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
cycles. Final source preparation passed; no A2 integration native test,
exact-SHA guard, commit or push has run at this checkpoint. The release target
is 0.24.72, following main's completed 0.24.71 release.

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
diagrams and source-diff retention. The supplemental script is unexecuted.
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
those two documentation changes require the final narrow review before commit.
This preparation does not claim native-test, firmware or exact-SHA guard success.
