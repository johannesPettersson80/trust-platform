# Production Code-to-Spec Coverage Plan

Status: design contract accepted after the 2026-07-20 r19 review findings were
folded. This plan is not evidence that coverage exists. The implementation
board owns execution status through Phase 17.

## 1. Objective

Establish an explicit, fail-closed chain from production code to written
behavior:

```text
production code unit
  -> active specification fragment or open specification gap
  -> invariant
  -> cataloged test
  -> suite and evidence
```

The existing verification program starts from registered specifications and
tests. Phase 17 adds the missing production-code denominator so that a function
cannot silently exist outside the specification graph.

This phase detects absence, staleness, and broken traceability. It does not
claim that a linked specification is semantically correct or complete; that
still requires review and behavioral evidence.

## 2. Non-Goals

- Do not infer expected behavior from function names, paths, comments, tests,
  neighboring code, or another implementation.
- Do not require a separate prose specification for every source line.
- Do not treat compilation, type checking, lint success, or a passing test as
  proof that a specification exists.
- Do not auto-promote invariants or close specification gaps.
- Do not append this logic to an existing large validator or scanner module.
- Do not make the historical denominator strict until it is exhaustively
  reviewed. New and changed code becomes strict earlier.

## 3. Canonical Denominator

The scanner inventories stable code units rather than lines.

### 3.1 Rust production facts

Derive the Rust file universe from `cargo metadata`, including workspace and
reviewed path-patched packages, every library/binary/example target, custom
target path, and build script. Cross-check that universe against repository
Rust files so an unrepresented manifest, target, or `build.rs` fails visibly.
Starting at each cargo target root, the extractor resolves the complete module
graph, including `mod.rs`, same-name module files, `#[path]`, and `include!`
inputs. An unresolved or multiply-resolved module is a blocking diagnostic.
This includes:

- product targets and build scripts under `crates/`, including
  `crates/trust-runtime/build.rs`,
- product Rust under path-patched `third_party/`, including
  `third_party/tiverse-mmap`,
- custom target paths even when they sit outside conventional `src/` geometry.

Inventory:

- functions and methods,
- trait methods with behavior contracts,
- public structs, enums, unions, type aliases, constants, and statics,
- macro invocations that register product handlers, commands, protocols, or
  runtime behavior,
- unsafe blocks/sites as separate risk-bearing facts where the existing unsafe
  register does not already provide the authoritative identity.

Exclude test-only items only when `#[cfg(test)]`, cargo target geometry, or
another source-derived test boundary proves that they cannot ship. Generated
code is not omitted: it uses an explicit generated disposition bound to its
generator and digest.

### 3.2 TypeScript and browser production facts

Scan non-test production files under:

- `editors/vscode/src/**/*.{ts,js}`,
- `crates/trust-runtime/src/web/ui/**/*.js`, including `modules/`, `wasm/`, and
  shipped chunk sources,
- `crates/trust-runtime/web/ide-frontend/src/**/*.{ts,js,mjs}`.

The denominator also includes the exact executable packaging inputs
`crates/trust-runtime/web/ide-frontend/vite.config.ts` and
`crates/trust-runtime/web/ide-frontend/vite.monaco.config.ts`. They are scanned
as whole-file facts so each generated IDE bundle can resolve to a governed
generator identity and exact input digest. This is an explicit packaging join,
not a recursive widening to arbitrary frontend build tooling.

Inventory functions, methods, command registrations, request handlers,
webview message handlers, and exported product types/constants. Test files and
fixtures remain in the existing test catalog, not this denominator. Bind the
reviewed roots to the extension/runtime packaging inputs and fail when a new
shipped JavaScript or TypeScript root is not represented.

### 3.3 Verification and operational tooling facts

After the two product denominators are stable, scan behavior-bearing tooling
under these explicit roots:

- Rust tooling under `xtask/src/**/*.rs`,
- Python and shell under `scripts/` and `scripts/verification/`,
- tracked agent hooks under `.claude/hooks/`,
- tracked operational skill scripts under `.codex/skills/**/scripts/`.

Classify them as verification or operational tooling, not product proof.
Existing gate-inventory identities remain authoritative for executable gate
entrypoints and are joined rather than duplicated.

Fuzz targets under `fuzz/fuzz_targets/` and crate-local fuzz workspaces belong
to the tooling denominator and join the existing fuzz registry. The Phase 17
TypeScript denominator explicitly excludes `editors/neovim/` and
`editors/zed/` while they contain only the reviewed declarative integration
configuration. Their file census and digests remain monitored; adding an
executable source surface forces a P17 scope review rather than passing
silently.

### 3.4 Identity and digest rules

Each fact receives an identity matching the anchored namespace
`^CODE_[0-9A-F]{64}$`, derived from the full SHA-256 of:

```text
source kind + normalized repository path + qualified symbol + item kind + normalized signature
```

Rust qualification uses `module::function`, `Type::method`, or
`<Type as Trait>::method` as applicable. Anonymous TypeScript/JavaScript
registration handlers use their containing symbol, registration API, literal
registered key, and structural occurrence ordinal. The validator rejects an
identity collision instead of extending or matching the prefix loosely.

Line numbers are display-only. Moving a symbol within the same file does not
change its identity. Renaming, changing its signature, moving it across files,
or changing its source kind creates a new identity and makes the old mapping
stale.

Each fact also carries a canonical body digest over the exact UTF-8 source byte
slice owned by the parsed item, excluding only its file position. Whitespace
and comment changes inside the item therefore require re-review. A body change
keeps the stable identity but invalidates its reviewed mapping until the
changed behavior is re-reviewed.

## 4. Machine-Readable Mapping

Add `verification/code-spec-map.toml` with a closed schema. One record must
resolve to exactly one live code fact. The record binds path, qualified symbol,
item kind, signature digest, and reviewed body digest to prevent rebinding.

At ratchet start, also commit
`verification/code-spec-debt-baseline.toml`. Each grandfathered record binds an
exact live code ID, path, signature digest, and body digest from the frozen
source commit. The baseline is append-forbidden and field-edit-forbidden after
creation; entries may only be deleted when the fact is mapped or removed.
Any live fact absent from both the map and baseline is new uncovered code. Any
baseline fact whose signature or body digest changes is changed uncovered code.
This rule is independent of merge-base selection, branch topology, and PR
splitting. `VERIF-P17-012` requires the baseline to be empty.

Every live fact has exactly one disposition:

| Disposition | Required binding | Meaning |
|---|---|---|
| `specified` | active oracle-eligible spec fragment, invariant IDs, catalog test IDs | behavior is written and exercised |
| `spec_gap` | one open spec-gap ID and owner | behavior exists but expected behavior is not yet decided |
| `inherits_spec` | one exact parent code ID | private mechanical helper inherits a specified parent contract |
| `non_behavioral` | closed reason code, owner, reviewed rationale | no independent product decision is made here |
| `generated` | tracked generator code identity and generator/input digest | generated behavior is governed at its scanned source |
| `vendored_external` | tracked provenance, version/revision, reviewed boundary contract | vendored implementation is externally owned but its product boundary is specified |

Omission is `missing_spec`; it is a finding, not an allowed committed
disposition. There is no wildcard, directory-level blanket, name-derived
mapping, or generic `reviewed_nonmapping` escape.

`inherits_spec` is allowed only for private helpers with an acyclic, exact
parent chain. The terminal parent must be `specified` or `spec_gap`; inheritance
cannot cross owners or canonical verification areas without an explicit review
record.

`non_behavioral` uses this closed v1 reason vocabulary:
`pure_data_access`, `mechanical_adapter`, `serialization_shape`,
`build_scaffolding`, and `test_support_in_production_file`. Extending the
vocabulary requires a schema and validator review. The validator rejects a
non-behavioral claim when the code unit carries any closed v1 risk signal from
Section 9.

A `generated` record must resolve its generator to a scanned `CODE_*` fact.
Before the owning scanner stage opens, a temporary record may bind a tracked
generator path plus exact digest, but it is reported as
`pending_generator_join` and cannot satisfy full-denominator enforcement.

Every reviewed body-digest change is rendered in a distinct report section
with code ID, old digest, new digest, owner, and rationale. A high-risk fact
also requires an explicit `owner_ack` value. Rebinding the digest in the map
does not remove this review-visible delta from the changed-code gate.

## 5. Semantic Validation

For every `specified` record, validation requires:

1. The spec source exists, is active, is oracle-eligible, and the cited fragment
   exists.
2. Every invariant exists and explicitly references the same oracle or owning
   specification requirement.
3. Every test exists in `verification/test-catalog.toml` and links back to at
   least one named invariant.
4. The test has an executable suite route and is not silently ignored.
5. Code, spec, invariant, and test areas agree or carry a reviewed cross-area
   decision.
6. The reviewed body digest equals the live fact digest.

For every `spec_gap` record, the gap must exist, remain open, own the same area,
and name the affected code identity. Closing the gap without converting the
code record to `specified` is a gate failure.

The validator rejects:

- missing and duplicate facts,
- invented or stale code identities,
- path traversal, symlink escape, and ignored/untracked inputs,
- signature or body-digest drift,
- inheritance cycles and forks,
- inactive/public-claim/non-oracle specification links,
- dangling invariant or test links,
- broad non-behavioral suppressions,
- generated/vendored claims without durable provenance.

## 6. Architecture and Ownership

Keep responsibilities separate:

- `code_spec_common.py`: fact shapes, canonical identities, digest helpers.
- `xtask/src/code_spec_scan.rs`: `syn`-based Rust AST extraction and canonical
  JSON emission over the cargo-metadata source universe.
- `code_spec_rust.py`: invoke and validate the Rust extractor output.
- `code_spec_ts_scan.mjs`: TypeScript compiler-API extraction using the exact
  `typescript` version pinned by the VS Code/runtime frontend lockfile.
- `code_spec_typescript.py`: invoke and validate the JS/TS extractor output.
- `code_spec_tooling.py`: Rust/Python/shell operational-tooling joins when its
  stage opens.
- `code_spec_contract.py`: mapping schema and semantic validation.
- `code_spec_live.py`: complete live join with existing metadata.
- `code_spec_report.py`: canonical JSON and Markdown rendering.
- `scripts/code_spec_coverage_gate.py`: thin CLI entrypoint.

The Rust parser decision is frozen: use `syn` in the small xtask extractor and
consume its canonical JSON from Python. Verify the extractor on local aarch64
and `trust-builder` before accepting P17-003. TypeScript/JavaScript extraction
uses the TypeScript compiler API with `allowJs` and its lockfile-pinned version.
Python tooling uses the standard-library `ast` module. Shell entrypoints remain
file-level facts joined to the authoritative gate inventory and must pass
`bash -n`; Phase 17 does not invent a shell function parser. Regex may be used
only for an explicit fail-loud sentinel around syntax the selected parser does
not support.

`xtask/src/full_map.rs::function_summaries_from_source` is not an authoritative
Phase 17 inventory and must not become a second production parser. Before the
Rust denominator closes, the software map must consume the Phase 17 facts or
retire its overlapping function-summary extraction. Cargo package and unsafe
summaries may remain only through an explicit join to the authoritative facts.

No module may combine extraction, metadata validation, report rendering, and
CLI orchestration. Split before any file approaches the repository's 1,000-line
limit.

## 7. Tests-First Implementation Sequence

### Stage A - Contract fixtures

Write failing fixtures before production implementation for:

1. stable identity under line movement,
2. identity change on rename/signature/path change,
3. body change producing mapping staleness,
4. missing mapping producing `missing_spec`,
5. exact `specified` join through spec, invariant, and test,
6. inactive or non-oracle spec rejection,
7. open-gap acceptance and closed-gap rejection,
8. inheritance success, cycle, cross-owner, and missing-parent rejection,
9. non-behavioral risk-signal rejection,
10. generated and vendored provenance validation,
11. duplicate, invented, deleted, symlinked, ignored, and escaping facts,
12. canonical report and Markdown tamper rejection,
13. nonzero per-surface source and fact floors plus an empty-parser result,
14. unsupported Rust and TypeScript syntax producing a blocking diagnostic,
15. anonymous registration and Rust trait/inherent method disambiguation,
16. digest rebind visibility and missing high-risk owner acknowledgement,
17. debt-baseline append/field-edit rejection and delete-only burn-down,
18. a generated record whose generator does not resolve to a governed fact,
19. cargo custom targets, build scripts, nested modules, `#[path]`, and
    unresolved `include!`/module diagnostics,
20. TypeScript anonymous handlers parsed through the pinned compiler API.

Every reviewed surface has a committed minimum source count and fact count.
Lowering a floor is a review-visible contract change. A nonempty behavior source
that yields zero facts is an error unless the file has an exact, reviewed
data-only disposition. Cargo/packaging discovery and the root census must agree
so a missing root cannot make both counts vacuously zero.

### Stage B - Rust pilot

Implement the Rust scanner and run it first against
`crates/trust-runtime/src/stdlib/time.rs`. The pilot must enumerate
`runtime_clock_value` and `current_dt` without deciding their semantics.

The pilot closes only when each live fact is explicitly mapped to an active
specification, an open spec gap, an exact inherited parent, or an allowed
non-behavioral disposition. If the `CURRENT_DT` clock contract is missing, the
expected result is a blocking `missing_spec` or `spec_gap`, not an inferred
fix.

### Stage C - Rust product denominator

Expand to every cargo-metadata-derived Rust product fact, including build
scripts, custom target paths, and reviewed path-patched product crates. Produce
a canonical denominator report with counts by crate, cargo target, area,
disposition, owner, and risk signal. Review every omission; do not bulk-classify
private helpers by directory or naming pattern.

### Stage D - TypeScript/browser denominator

Add the TypeScript/JavaScript adapter and repeat the exact partition for VS
Code and shipped browser code. Visible workflows must connect through their
existing user-story and UI-acceptance records.

### Stage E - Verification tooling denominator

Add Rust, Python, and shell tooling facts for the exact roots in Section 3.3 and
join existing gate and fuzz inventories. Tooling code must link to its
verification contract and self-test fixtures, but cannot create product proof.

## 8. Enforcement Rollout

1. Land scanner, mapping contract, fixtures, report, and frozen debt baseline
   in report-only mode.
2. Immediately block every live fact absent from both the map and frozen
   baseline, and every mapped or baselined fact whose signature/body digest
   changed. Merge-base paths remain useful display context but never decide
   whether code is new or grandfathered. Passing broad tests cannot override
   the result.
3. Keep baseline entries visible as historical debt. The baseline is
   append-forbidden and delete-only.
4. Render every map digest rebind separately and require acknowledgement for
   high-risk facts.
5. Flip whole-denominator enforcement only when every live fact is partitioned,
   the baseline is empty, and an independent review accepts the report.
6. Wire the strict check into the existing verification workflow; do not add a
   second CI workflow or another release dependency.

The planner must surface these outcomes distinctly:

- `missing_spec`: no code mapping,
- `stale_spec_review`: mapped code body changed,
- `spec_gap`: explicit unresolved expected behavior,
- `missing_tests`: specified behavior lacks a cataloged executable test,
- `unmapped`: source geometry or ownership cannot be classified.

## 9. High-Risk Source Policies

After the denominator is live, add reviewed source-policy checks for code that
is reachable from PLC scan execution. The closed v1 risk-signal enum is:

- `host_time`,
- `randomness`,
- `filesystem_or_environment`,
- `network_io`,
- `subprocess_or_thread`,
- `unsafe_or_ffi`,
- `state_mutation`,
- `output_or_safe_state_commit`,
- `authorization`,
- `fault_mapping`,
- `protocol_response_or_status`,
- `broad_error_suppression`,
- `persistence_retain_reload_restart`.

A risk signal does not invent a defect. It requires an explicit specification
or open gap and prevents `non_behavioral` suppression. Allowlist entries are
symbol-specific, owner-reviewed, digest-bound, and expiry-capable.

## 10. Validation Cadence

During implementation, run only the focused scanner/contract fixtures and
metadata tests. Before the first full report, freeze the implementation and run
the cheap complete-diff preflight. Generate the denominator report once from a
clean detached `trust-builder` worktree.

At the final milestone run on `trust-builder`:

```bash
just fmt
just clippy
just test-all
```

Also run the verification focused suite, metadata gate, tooling self-tests,
test-catalog staleness, and the new code-spec checker. Do not run repeated full
workspace gates after each mapping edit.

## 11. Evidence and Definition of Done

Phase 17 is complete only when:

- all scoped production facts are present in a canonical report,
- the report is an exact one-to-one partition with zero unreviewed facts,
- the frozen debt baseline is empty and its history contains no appended or
  edited entries after creation,
- new/changed code enforcement is proven fail-closed,
- whole-denominator enforcement is enabled only after independent review,
- `CURRENT_DT` demonstrates the missing-spec path in the pilot unless an
  active product specification already governs it,
- code-to-spec, spec-to-invariant, and invariant-to-test links validate in both
  directions,
- adversarial fixtures detect disabled catchers,
- no product invariant is promoted merely because a code mapping exists,
- CI remains acyclic and the existing release workflow is unchanged by this
  phase unless a separate reviewed release-tooling change is required.

## 12. Stop Conditions

Stop and request a product decision when:

- a code unit makes observable behavior but no authoritative source defines
  the expected result,
- two eligible specifications conflict,
- a private helper cannot honestly inherit one parent behavior,
- generated or vendored behavior has no durable source/provenance contract,
- parser limitations could silently omit facts,
- a proposed suppression would cover an entire directory, crate, or owner
  without per-fact identities.

Never resolve these conditions by deriving intent from the implementation or
by writing a test that merely preserves current behavior.
