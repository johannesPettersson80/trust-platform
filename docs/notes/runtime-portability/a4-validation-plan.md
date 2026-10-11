Run 11 passed all 29 required checks and both advisories; final independent
acceptance is recorded in `a4-review/run-11-final-independent-acceptance.md`.
No further A4 validation run is planned. The command plan and failure history below
are retained as executed records, not authorization to restart them.

# Completed A4 run-11 correction batch plan

Generic counter type-policy implementation, native policy tests and the
hierarchical I/O negative fixture repair are complete and independently reviewed
for this consolidated batch. Run 10 and all 50 raw artifacts remain intact. Commands
match run 10; only setup/evidence paths change under
`/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-11/`.
Use standing correction authorization and coordinate the heavy builder slot.
No per-command retries, A4 commit/push or hardware work.

---

# Historical A4 run-10 correction batch plan

Run 9 is preserved with all 50 raw artifacts. The one-file hosted integration
import correction passed independent review before this complete consolidated
batch. The commands match run 9, with only setup/evidence paths changed under
`/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-10/`.
Use standing correction authorization; preserve source freeze and builder
coordination. No individual retries, A4 commit/push or hardware execution.

---

# Historical A4 run-9 correction batch plan

Run 8 and all 48 raw artifacts are retained. Four corrected source files passed
independent review; this does not prove compilation or behavior. Run 9 repeats
the complete run-8 allocation, with only evidence/setup paths changed under
`/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-9/`.
Honor standing correction authorization, source freeze and builder coordination.
No per-command retries, A4 commit/push or hardware execution.

---

# Historical A4 run-8 correction batch plan

Run7's complete failure ledger is retained. Eight corrected source files passed
independent review; this is not compilation or behavior proof. Run8 repeats the
unchanged consolidated command allocation from run7 with new evidence/setup paths
under `/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-8/`.
Use the recorded standing authorization for reviewed correction cycles, preserve
source freeze and every outcome, and coordinate the builder slot with A2. No
individual command retries, A4 commit/push or hardware execution.

---

# Historical A4 run-7 correction batch plan

Run6 completed with compilation failures; its complete ledger and raw source are
preserved. Five owning source fixes received independent review. Under the recorded
standing authorization for reviewed correction cycles, run7 repeats the complete
run6 command allocation after these corrections, changing only setup/evidence paths
to `/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-7/`.
No runtime behavior suite from run6 is counted as passed. Freeze source after the
same one-time preparation, collect all independent failures, and never retry an
individual command. No A4 commit/push or hardware run.

---

# Historical run-6 external-review correction batch plan

The correction source is not yet frozen or validated. Finish implementation and
independent review before the next consolidated builder batch. Its prepared
orchestration is external at
`/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-6/`.
No script in that directory has been launched.

Reuse run 5's complete command allocation, adding hosted integration binaries
`source_free_execution_cost`, `borrowed_reference_allocations`, and
`source_free_blocks_and_io`. Core all-features/no-default and i686 lib lanes include
the new budget and forged-initializer execution tests. Retain runtime unit, full
affected integration/vertical, portable MCU library checks, no-dev feature graphs,
Clippy, cross-target warnings, supply chain, architecture, diagrams/drift, formatting
and provenance assertions. Newly touched include fragments need the explicit one-pass
formatter alongside cargo fmt; the allocator unsafe-site register must name the final
formatted source lines.

The builder heavy slot is currently reserved for the independent A2 publication
batch. Recheck capacity and active processes before A4 takes the target lease.
Freeze after the single formatting/provenance/fixture preparation sequence, retain
all outcomes and do not retry failed commands automatically. Independent final
evidence reconciliation must cover the complete authored source manifest and classify
exact moves separately. Scope B hardware, A4 commit and push remain outside this batch.

---

# Historical A4 run-2 validation plan

Historical completed evidence: [run-5 ledger](a4-evidence/run-5/ledger.tsv) and
[exact run-5 commands](a4-evidence/run-5/commands.txt). The text below preserves
the earlier plan. From run 3 onward, mutation-runner tests clear GIT_INDEX_FILE;
mutation contracts use the isolated index in a separate step.

At this historical checkpoint, run 1 failed and its complete evidence is retained in `a4-evidence/run-1`. The
corrections and independent source review are complete. This record describes
the consolidated run-2 plan under the owner’s standing correction authorization;
it is not a claim that run 2 has executed. The exact commands are retained in
`a4-evidence/batch-setup-run-2`.

## Preparation and freeze order

1. Verify canonical AGENTS/full skills, Rust 1.95.0/edition 2021, target availability,
   selected target/check-out/TMPDIR filesystem capacity and concurrent workloads.
   Use the builder target lease for all Cargo invocations. Save the actual commands,
   tool versions, source identity and exit ledger.
2. Perform the single planned formatting pass, including retained include fragments.
3. Invoke `bash scripts/refresh_a4_provenance.sh "$A4_EVIDENCE/provenance"` under
   that lease. The Rust helper reuses the existing invariant execution-contract
   digest implementation, changes only case source-digest headers, updates matching
   current catalog case hashes and all native literal pins referencing those files,
   and refreshes planned mutation source hashes/selectors after formatting. Exact
   expression selectors win; fallback requires one unique matching function/genre/
   replacement. Ambiguity fails instead of choosing an arbitrary occurrence.
   Candidate selector JSON is retained as new batch evidence. This is discovery,
   not a mutation campaign. Existing measured artifacts/status remain untouched.
4. Generate both saved application and numeric STBC fixtures using the existing
   portability fixture producer (including the numeric companion). Capture bytes,
   hashes and tool output. Dependent saved-byte tests cannot run if generation fails.
5. Freeze source/fixtures after these preparation changes. A metadata preparation
   failure is recorded separately; do not silently retry it or suppress unrelated
   executable checks. No source edits during the validation portion.

The helper deliberately does not regenerate case bodies, expectations, historical
traces, measured mutation reports or historical evidence commands. Existing
`gen_cases.py` first validates the entire metadata registry, so invoking its CLI
against stale source hashes creates a circular refresh prerequisite; the helper
calls its shared authoritative execution-contract digest function instead.

## Affected native suites

One core all-feature run and one portable no-default run cover moved unit suites:

```
cargo test --locked -p trust-runtime-core --all-features
cargo test --locked -p trust-runtime-core --no-default-features
cargo test --locked -p trust-runtime-core --no-default-features --target i686-unknown-linux-musl --lib
cargo check --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf
cargo check --locked -p trust-runtime-core --no-default-features --target riscv32imac-unknown-none-elf
```

The complete core test commands include the new saved-artifact tests:
`runtime_core_compiler_free_load`, `source_free_numeric`, `source_free_cycle`,
`source_free_profile_admission`, `portable_execution_frames`, plus retained loader,
validator, numeric, I/O and scheduler suites. Do not also run those binaries separately.

Hosted unit suite and affected integration suites (one command may select them all):

```
cargo test --locked -p verification-cases --example refresh_portability_provenance
cargo test --locked -p trust-runtime --lib
cargo test --locked -p trust-runtime \
  --test runtime_core_compiler_free_load --test bytecode_source_free_authoring \
  --test runtime_core_behavior_lock --test portable_numeric_contract \
  --test source_free_assignment --test source_free_cycle_contract --test source_free_restart_graph --test source_free_readonly \
  --test bytecode_container --test bytecode_validation --test bytecode_verification_cases \
  --test bytecode_vm_core --test bytecode_vm_differential --test phase11_seam_contract \
  --test vm_resource_limit_cases --test runtime_restart --test runtime_restart_trace_cases \
  --test vars_retain --test tasks --test tasks_fb --test scheduler_resource \
  --test user_type_runtime_contract --test struct_initializers \
  --test stdlib_core_contract --test stdlib_conversion_contract --test stdlib_helper_contract \
  --test stdlib_fb_contract --test stdlib_split_locals --test stdlib_enum_validate \
  --test stdlib_assertions --test stdlib_numeric --test stdlib_numeric_full \
  --test stdlib_conv --test stdlib_conv_full --test stdlib_bit_full \
  --test stdlib_select --test stdlib_select_full --test stdlib_string --test stdlib_string_full \
  --test io_address --test io_cycle --test io_struct_array --test io_wildcard \
  --test io_fb_vars --test io_hierarchy --test io_driver \
  --test api_smoke --test debug_control --test complete_program --test runtime_reliability
```

Root integrates required format/lint/cross-warning, architecture, diagram/drift,
supply-chain and affected tooling commands into this same ledger. Generated
registry/denominator maintenance remains advisory; old discovery identities must
not be relabeled as newly measured results. Source hashes and native static pins
are kept consistent before their case runners execute. Builder-only checks do not
prove native Windows/macOS or MCU hardware execution. No release guard, push,
commit, A5 scope or hardware run is authorized by this plan.

The refresh helper prepares all proposed edits before writing, but individual
filesystem writes are not an atomic multi-file transaction. If a disk/I/O write
fails, preserve the partial state and ledger; do not freeze it as consistent or
launch a retry automatically. Review its listed updates and the freeze manifest.


Run 2 also executes both affected Python modules in one invocation:
`python3 -m unittest scripts.verification.focused_mutation_runner_tests scripts.verification.mutation_program_contract_tests`.
Python bytecode-cache writing is disabled. The complete reviewed lease/path-policy
helpers are now present in this checkout, as required by the canonical guard tests
inside the supply-chain gate. Its lease wraps the entire batch.
