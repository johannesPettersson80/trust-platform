# Phase 18 final product-oracle denominator closure

Date: 2026-07-29

## Scope and result

This coherent batch closes the final product-oracle population in the live
test denominator and therefore closes `VERIF-P18-005`.

The frozen input was:

```text
4,401 live test facts
4,318 catalog_mapped
   83 reviewed_nonmapping
```

One of the 83 scanner facts was not an executable test. The
`single_axis_negative_stop_active` PLCopen Motion fixture is deliberately
uncompilable because the reviewed PLCopen profile gives `MC_Stop` no public
`Active` member. Its stray `TEST_PROGRAM` declaration was removed. The invalid
source remains in `main.st` and is still executed by the existing
`plcopen_motion_deferred_public_surfaces_remain_absent` negative parent, which
now explicitly checks the absent `MC_Stop.Active` diagnostic.

The remaining 82 facts received accepted, assertion-reviewed mappings. The
final live equation is:

```text
4,400 live scanner facts
4,400 catalog_mapped
    0 reviewed_nonmapping
    0 not_catalogable_product_oracle
```

The catalog has 4,408 rows: the 4,400 generated live-fact mappings and eight
other governed records. The proposal queue is empty.

This closes only `VERIF-P18-005`. `VERIF-P18-003` remains current with 203
`gap_open` coverage cells, nine current exact-behavior-marker debts, and eleven
empty historical placeholder invariants. Bidirectional proof,
native/conditional executions, reachability, mutation sensitivity, adequacy
campaigns, the final candidate gates, and release/audit rows also remain open.

## Checkout and bootstrap

- Canonical local checkout:
  `/home/johannes/projects/trust-platform`
- Local branch:
  `plc-verification-program`
- Frozen source commit:
  `2a44545187a756093b39b5acff68acc7cbec2e48`
- Remote canonical agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Remote validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-product-83-2a445451`
- Remote destination state:
  detached exact HEAD
  `2a44545187a756093b39b5acff68acc7cbec2e48`
- Builder host:
  `scena-rust-builder`

The destination `AGENTS.md`, complete `.codex/skills/**` tree, and required
private hook were manually copied from the remote canonical checkout and
verified before editing or validation. `AGENTS.md` matched:

```text
76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
```

The required `trust-test-authoring`, `trust-remote-builder`, `st-lsp-solid`,
`trust-release-hygiene`, and `trust-lsp-iec` skills were read and followed.
This batch used the checklist's staged cadence: focused execution throughout,
with no workspace-wide `just fmt`, `just clippy`, `just test`, or
`just test-all`.

The first cold multi-package listing exhausted the generated builder target
cache and was not a behavioral red. After confirming that no compiler or
linker process survived, only the exact generated warmed target
`$HOME/.cache/codex-targets/trust-platform-gate` was removed and recreated.
Free space recovered to 60 GiB. A later atomic metadata write briefly met the
filesystem ceiling before replacement; the transaction journal had not been
written and no mapping file changed. Disk and inode preflight then showed
46 GiB and 15 million inodes free, and the same atomic transaction completed.
No source worktree or non-generated user file was removed.

The exact builder reference environment is retained in:

```text
target/gate-artifacts/verification/p18-product-83-2a445451/reference-environment.txt
sha256 5d89780cd1f34e638cf6d3fe3ac82f41d374623c086b7ab8f4212eabde5036bb
```

## Specification and invariant authority

The batch added exact authority for:

- optimized bytecode backend environment, cache, buffer, profiling, deadline,
  fallback, and native-symbol observability;
- LSP workspace coordination, limiter, cache, projection, type, and range
  helpers;
- CI timing claims bound to the named prebuilt builder reference environment;
- registry initialization CLI parsing;
- the OPC UA client subscription API surface;
- verification integration-harness resource bounds and specification-sync
  checks; and
- the runtime-host ADS setup page's exact local onboarding asset surface.

The ADS setup asset test was initially routed through a broad host-security
invariant. Focused metadata validation rejected that route because its
`SPEC_RUNTIME_ENGINE_001` oracle belongs to `runtime_safety`, not
`control_security`. The runtime specification now states the exact static
surface, `RT_ADS_SETUP_ASSET_SURFACE_001` owns it, and the mapping was
atomically rebound to that invariant.

The `plcopen_devtools` owners were also normalized to the governance-approved
`trust-dev` and `trust-plcopen` owners. These are product and verification
contracts outside IEC 61131-3 normative behavior; no IEC decision or deviation
was added.

## Live scan and assertion review

The scanner after the negative-fixture correction reports:

```text
live facts        4,400
source files        763
diagnostics           0
warnings              0

live-scan-4400.json
  sha256 93810b8cbe2d079146b9ec7e332ed99be045340d8ac6d86a0a2e6171de5462e1
```

The removed false discovery identity
`DISC_458AD738C2D721187347` is absent. The refreshed negative parent retains
nine assertions across its complete 18-file source closure.

The final 82-test review is:

```text
selected tests                    82
observed assertions              455
oracle-bound assertions          297
setup/precondition assertions    158
unresolved helpers                 0

assertions-82-plus-parent.json
  sha256 e0e5dc42ddde7a8f5107ec30fb279cd0755a30b9ca4caab4e106fbbccc7c0dd6
assignments-82.json
  sha256 9f67f88306b60f7d79e4f0768280451dd3c26718878a15b1aaa26c2d287a7339
```

Every selected test has at least one oracle-bound assertion and an exact
behavior reference. The accepted review uses the distinct
`phase18-oracle-audit` and `phase18-semantic-review` reviewer identities.

Strengthening the shared coordinated-motion fixture and the runtime
specification invalidated line- or source-closure-derived identities for 23
already accepted mappings. Their assertion expressions and reviewer
dispositions were compared exactly, rebound to current assertion/source
digests, and assigned new generated proposal and decision IDs without changing
their reviewed behavior semantics:

```text
source-closure-refresh-23.json
  sha256 40c1bdf49e5cb69380313ddcc67697a337d429b558ab1d763991094d20ab98f0
metadata-repair-result.json
  sha256 cee2a3877fd4ea4eadfadb002d8ccfeb33c757cd49f112632719ee7cff97c0ff
```

## Focused execution

All 76 selected Rust facts passed at their exact owning boundaries. The six
selected Structured Text facts also ran; three use their exact native PLCopen
fixture commands and three CI fixture facts run through their registered Rust
parent harnesses.

| Focused population | Result | Time |
| --- | ---: | ---: |
| runtime library | 54/54 | 5.62 s |
| LSP binary | 7/7 | 1.78 s |
| IDE library | 4/4 | 0.63 s |
| LSP specification drift | 2/2 | 8.08 s |
| IDE stdlib coverage | 1/1 | 3.17 s |
| registry CLI parser | 1/1 | 39.52 s |
| ADS web assets | 1/1 | 10.29 s |
| bytecode `SIZEOF` alias | 1/1 | 3.42 s |
| I/O broker lifetime | 1/1 | 2.41 s |
| OPC UA API surface | 1/1 | 2.05 s |
| CI timing facts plus two native parent harnesses | 5/5 | 1.91 s |
| coordinated-motion seed probe | 1/1 | 7.76 s |
| homing seed and probe | 2/2 | 15.32 s |
| negative PLCopen public-surface parent | 1/1 | 27.60 s |

The CI parent command first stopped because the cold cache lacked the sibling
`trust-dev` binary. That harness dependency was built and the same exact
five-case command passed. The setup failure is not counted as a behavioral
expected red.

The direct attempt to execute the false `MC_Stop.Active` identity failed
compilation with E105 because the member is intentionally absent. That is not
counted as a test red or pass. The corrected negative parent passed and remains
the executable proof.

Representative retained artifacts:

```text
runtime-lib-54.jsonl
  sha256 c6c0e8fe314d22b733892a959a5a45cba87758b65f5016afafe9545731b339e9
lsp-bin-7.jsonl
  sha256 6997bc237831e82eae24e5a4d9a67a8606c67d694998acef469ca37bbd851a39
runtime-ci-5-rerun.jsonl
  sha256 ce322e358b5e7b5c267d398cfb669f2d00092cb761d12ed7796a9726da4961fc
plcopen-coordinated-after.txt
  sha256 2e55c9ba456e905504a1e1d0ff88b95952f705ccc92076f4b798789a7239c240
plcopen-negative-surfaces-with-stop-active.jsonl
  sha256 37efb32d854edd38e4f9c2e6647ff842fd48491abe5541d136f51ca1e8f98af9
```

The strengthened coordinated seed probe was a behavior-preserving
test-strengthening slice. Its original smoke assertion passed before the edit,
and the same exact test passed afterward with nine state and transform
assertions.

## Atomic mapping transaction

The transaction result is:

```text
accepted tests                  82
catalog rows                 4,408
denominator rows             4,400
reviewed_nonmapping              0
parent mappings refreshed        1

transaction-result.json
  sha256 70f47b4454eda6b140167d995612250ece8b7f097f56acc50d8f0029105e7fd1
```

The final mapping metadata digests are:

```text
verification/test-catalog.toml
  f1f648073ed0cc60fc15e7cfa3e9e9de69d8b93451752a937d3fccbe4ce5035d
verification/test-catalog-denominator.toml
  4e244ede4b499adaa3db4c99604781a7cf9015af2781ff3551ac13ea6ef0832e
verification/test-catalog-proposals.toml
  f2b3bf4f97f3794e1ff0a7e16ad8283b2f3f1be00db60c8eedbba86883bdfad7
verification/test-catalog-mapping-decisions.toml
  8bd406610c68b808e640d2e30c28066590f511a0efc39c5f63dc5a3365761b53
```

The focused mapping suite passes:

```text
74/74 in 0.031 s
mapping-suite-final.stderr.txt
  sha256 750d35baeaa7925acad5b4a153b0843912a0f0920929beb5dac330416407d133
```

`git diff --check` passes for the frozen remote transaction.

## Governance isolation

The repository-wide catalog-staleness and complete metadata validators remain
red on accepted downstream Phase 18 debt. Their final outputs are:

```text
staleness-final.stdout.txt
  sha256 9481d5ff5aaa1f7c3760584ad4a8b208e5f8b81d706477d4f9cc2bfbff3cba6f
metadata-validator-final.stderr.txt
  sha256 cdb6cbc9d85f22f8cf593d99ea8ed47cd7e69147f684064ba4953068bb6c1cbb
```

Exact isolation over the 82 accepted tests, 23 refreshed historical mappings,
the negative parent, and every new or touched invariant finds zero
batch-blocking findings. One raw name match remains:
`TEST_CASE_TABLE_VM_SEAM_OWNER_001` carries a pre-existing case-generator code
digest mismatch. The generator digest does not depend on the invariant's test
list and the same finding existed before this batch. It remains global
downstream debt rather than being hidden or counted as this batch's pass.

```text
governance-isolation-final.json
  sha256 05e8fce359bcc15d461dc5413ee13f8c0fa900d398bb8c4fed850b81e4a2e7f8
target-blocking findings: 0
pre-existing global findings: 1
```

The pre-change planner covered every then-known specification, invariant,
fixture, parent-test, and mapping path. It exited on the accepted global Phase
18 planner debt:

```text
planner-negative-fixture-before.stderr.txt
  sha256 af12477ee4c38f80efb1cff89bdc77472b9243c828b635dd05e6ac14164f691d
```

The complete changed-path planner covers all 33 final task paths, including
the deleted false test identity, specifications, invariants, four mapping
documents, canonical board, evidence, and review lock. It exits 6 on the
accepted global Phase 18 planner debt:

```text
complete-planner.stdout.txt
  sha256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
complete-planner.stderr.txt
  sha256 6f4264b4a7880aa847d76c965ee55813629854e798f0967da7d5bc42e95c2992
```

That global red is not reported as a pass and does not override the focused
82-test execution or zero batch-blocking governance findings.

## Completion boundary

The machine-readable review lock is:

```text
verification/p18/review-locks/product-oracle-final-82-closure-lock.json
sha256 a21f53055db497c5644340a50559cfd02ded7a0a0f6006ed89237c44a00bc30e
```

No changelog or version bump is required for this internal verification
metadata and test-strengthening batch. No release action is taken here.

The canonical local checkout contains unrelated runtime changes owned by the
user. They are excluded from this batch and must retain the binary diff digest:

```text
01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4
```

No full workspace gate has run. The one final broad `just fmt`, `just clippy`,
and `just test-all` milestone remains reserved for `VERIF-P18-016`.
