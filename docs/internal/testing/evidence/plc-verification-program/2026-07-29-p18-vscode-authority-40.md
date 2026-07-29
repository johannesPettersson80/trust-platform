# Phase 18 VS Code authority campaign: 40 tests

Date: 2026-07-29  
Source branch: `plc-verification-program`  
Exact executed source commit:
`1c09656eed64d2b8a9800eb402de98b3399c9a72`

## Scope and authority

This campaign reviews one coherent 40-test population:

- 29 retained Ladder, Statechart, and visual-runtime component tests bind the
  explicitly non-primary component locks in
  `VS_CODE_RETAINED_COMPONENT_MODELS_001`;
- 10 shipped VS Code behavior tests bind existing
  `LSP_CORE_FEATURES_001` or `VS_CODE_PRODUCT_CONTRACT_001` behavior rows;
- one test-harness contract binds both exact-selection and commit-bound
  evidence behaviors in `VERIFICATION_VSCODE_EXACT_EXECUTION_001`.

The retained component tests do not establish runtime or IEC semantic
authority. The generated Structured Text and shipped extension journeys remain
the product execution authority defined by the VS Code product specification.
No IEC decision or deviation was created.

## Test-first evidence

The exact-selection seam was implemented through two focused assertion-red to
green loops on `trust-builder`:

1. The focused Mocha test first failed with zero passes and one failure:
   `expected the VS Code test harness to expose configurable Mocha selection`.
   After the minimum selection implementation, the same test passed 1/1.
2. The extended test then failed with zero passes and one failure:
   `expected the VS Code test harness to expose machine-readable evidence`.
   After the atomic JSON evidence implementation, the same test passed 1/1.

The TypeScript assertion extractor was also changed test-first. The focused
helper-closure test first failed because the HMI descriptor-refresh inventory
contained zero assertions. After recursive, cycle-safe, unambiguous same-file
helper traversal and assertion de-duplication, that focused test passed. The
complete assertion-inventory gate then passed locally:

```text
scripts/verification_assertion_inventory_gate.sh
Ran 8 tests in 0.733s
OK
```

The mandatory planners were run before each behavior change. Both stopped at
the known Phase 18 global metadata baseline with exit 6 before producing a
path-specific plan; neither reported a new changed-path-specific blocker.

## Exact builder bootstrap

Canonical source:
`/home/johannes/projects/trust-platform`

Validation destination:
`trust-builder:/home/johannes/projects/trust-platform-p18-vscode-authority-1c09656e`

```text
host=scena-rust-builder
head=1c09656eed64d2b8a9800eb402de98b3399c9a72
branch=<detached>
AGENTS.md sha256=76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills aggregate sha256=253377375e0009b82f732dfd26758e46b173fcedec8e0bf7bd85460946ea37e6
```

The canonical `AGENTS.md` and complete `.codex/skills/**` tree were manually
copied into the destination and matched the canonical hashes before build or
execution.

## Focused execution

The focused harness contract passed at the exact source commit:

```text
VS Code test selection
  passed: uses an opt-in grep while preserving the unfiltered default
1 passing
```

The real VS Code Extension Development Host was launched under Xvfb with the
exact 39-title selector, source commit, and evidence destination configured.
It executed exactly the selected population:

```text
39 passing (2s)
Exit code: 0
exact VS Code evidence reconciled:
39 unique expected tests, 39 passed, 0 failed
```

The commit-bound result artifact records 39 distinct full titles, 39 passes,
zero failures, and source commit `1c09656e...`. Its SHA-256 is
`7ab883a053031abfeeb42737435e59dbcdfd5412ab71827bd2d93c6b7326ea7c`.
The separate one-test Mocha JSON records one pass, zero failures, and has
SHA-256
`3ee1fdbabea05273d61280c216eb922ef4faa96584db119b8d7c488f378887e5`.

## Scanner, assertions, and atomic mapping

After removing only validation-generated source-map changes and cache
symlinks, the clean detached builder checkout produced the authoritative live
scan:

```text
generated 4411 records from 774 source files with 0 warning(s)
commit=1c09656eed64d2b8a9800eb402de98b3399c9a72
scan_status=complete
```

The 40 syntax-aware inventories contain 186 assertions and zero unresolved
assertion helpers. Setup-only HMI and language-model assertions are explicitly
classified as preconditions; all accepted oracle assertions bind exact
zero-based behavior references.

The mapping dry-run and atomic transaction both reconcile:

```text
40 tests
186 assertions
3983 catalog_mapped
428 reviewed_nonmapping
0 missing_spec_authority_tests
0 confirmed_missing_tests
```

The committed semantic lock is
`verification/p18/review-locks/vscode-authority-40-semantic-lock.json`.
The two new invariants are `mapped`, all of their behavior cells are covered,
and only the final `broad_remote_gate` item remains open.

## Validation boundary

Focused validation completed:

- exact harness contract: 1/1 passed on `trust-builder`;
- exact VS Code Extension Host selection: 39/39 passed on `trust-builder`;
- assertion-inventory gate: 8/8 passed locally;
- VS Code registration: 462 discovered facts across 40 registered files;
- clean scanner: 4,411 facts, zero warnings, exact commit;
- atomic catalog/denominator/decision/invariant reconciliation: passed;
- `git diff --check`: passed.

No broad `npm test`, `just fmt`, `just clippy`, `just test`, or `just test-all`
gate ran. Those remain deliberately deferred to the single frozen
`VERIF-P18-016` candidate milestone.
