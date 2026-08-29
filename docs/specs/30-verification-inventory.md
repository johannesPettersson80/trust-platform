# Verification Inventory Contract

Specification identity: `SPEC_VERIFICATION_INVENTORY_001`.

This document defines repository-governance behavior for inventories that link
verification metadata to test source. It does not define Structured Text
language behavior and does not provide IEC 61131-3 product proof.

## IEC table test inventory

The canonical IEC table inventory is the tracked repository file
`docs/specs/coverage/iec-table-test-map.toml`. The inventory check must read
that exact file and parse it as TOML containing table entries with an
identifier, an optional specification path, and a list of Rust test names. An
unreadable or malformed map is a failed inventory check.

For this check, the Rust test inventory consists of test functions discovered
recursively below the repository `crates/` and `tests/` trees. Discovery
considers Rust source files, skips generated or dependency trees named
`target`, `.git`, and `node_modules`, and recognizes functions following
`#[test]` or `#[tokio::test]`, including intervening attributes. Test names are
matched exactly.

Every test name listed by every table entry must resolve to at least one source
path in that Rust test inventory. A name that resolves nowhere is rendered as
missing and fails the check. A name that resolves in more than one source file
is not silently collapsed: every resolving path is displayed.

The human-readable report preserves table-entry and listed-test order from the
map. For each resolved name, displayed source paths are repository-relative,
use `/` separators, and are sorted lexically. The complete report is compared
with its reviewed snapshot so additions, removals, path changes, and ordering
changes require explicit review.

## Evidence boundary

This inventory establishes only that the map is structurally readable, each
listed name exists in the scanned Rust source inventory, and the report is
deterministic and review-locked. It does not:

- execute any listed test;
- establish that a test passes, reaches production behavior, or has an
  adequate assertion;
- establish that every relevant IEC table or every relevant test is listed;
- resolve a test name to a unique source file;
- prove IEC conformance or any IEC language semantic claim.

IEC product authority is the written product specification paired with a native
executable test that directly asserts the specified behavior. This governance
inventory may describe additional maintenance and execution history, but its
invariant, catalog, oracle, or evidence records cannot create product work,
reject an otherwise valid specification-and-test change, or substitute for the
native assertion.

## Reachable assertion-helper closure

For a literal VS Code test, the assertion inventory begins at that test's exact
callback. It includes direct assertions and assertions in an unambiguously
named, same-file function or function-valued variable that is reachable from
the callback. Local helper traversal is recursive, visits each helper at most
once, and de-duplicates an assertion by its stable assertion identity.

For a Rust integration or unit test, the inventory begins at the exact
`#[test]` function in the owning Cargo target. Unit-test extraction resolves
the target that owns the selected source, loads its complete module graph
including `cfg(test)` sources, and binds every participating source digest.
Reachable test-only helpers may cross source files through function-local or
module-level named imports, glob imports, and transitive re-exports. Resolution
honors private, public, `pub(crate)`, `pub(self)`, `pub(super)`, and
`pub(in path)` scopes and remains cycle-safe and ambiguity-preserving.

Ordinary imported product functions are not traversed as test-oracle helpers.
A same-source production helper may contribute its direct assertion macros,
but production method calls, runtime file observations, and further product
calls do not become delegated test assertions. Dynamic file reads reached in
the test-only closure remain explicit `unresolved-oracle` facts until closed
generated-path provenance or immutable repository input authority is proven.

Repository-backed oracle reads are closed only through a literal declared tree
and extension contract. Extraction rejects empty, absolute, parent-traversing,
or dynamically named tree roots, rejects missing or symbolic-link roots, and
binds the digest of every regular file with the declared extension below every
declared root. The matching runtime helper rejects symbolic-link or non-file
inputs, canonicalizes the repository root and selected path, and requires the
path and extension to remain inside one declared tree. A repository input
cannot be reclassified from a runtime value, an unchecked path helper, or a
passing test alone.

Generated-output reads are closed only when their path provenance reaches a
test-created temporary root through a closed relative join or an explicit
generated-root/generated-output boundary. The explicit root boundary
canonicalizes a real non-symbolic-link directory below the system temporary
directory. The explicit output boundary canonicalizes a real
non-symbolic-link file and requires it to remain strictly below that root.
Absolute, parent-escaping, dynamic, reassigned, report-derived, external-root,
missing, or symbolic-link inputs remain unresolved or fail at runtime.

Literal host-runtime reads whose path syntax is absolute on any supported host
are excluded from repository-fixture closure even when extraction runs on a
different host. This includes POSIX-rooted paths, Windows drive-rooted paths,
and Windows rooted or UNC paths. The extractor must not reinterpret one
platform's absolute host path as a relative repository fixture merely because
the extraction host uses another path grammar. This exclusion records only
that the input is host state rather than immutable repository authority; it
does not make the host observation a product oracle.

The Linux process-memory observation is a separate fixed host oracle. Its
named boundary accepts no path argument and reads exactly
`/proc/self/status`; a caller cannot use it to hide another filesystem input.
This classification records a host-dependent observation, not immutable
repository authority or cross-platform memory proof.

Imported product functions, ambiguous same-name declarations, and unresolved
calls whose names claim to assert are never inferred as proof. They remain
visible in `unresolved_assertion_helpers` when no exact target can be resolved.
This closure establishes static assertion reachability only. It does not prove
that the test executed or that an assertion is adequate for a product
invariant.

## Exact VS Code execution evidence

The extension test harness accepts the opt-in
`TRUST_VSCODE_TEST_GREP` regular expression. A missing or whitespace-only value
preserves the ordinary unfiltered suite. A configured value selects Mocha test
full titles and does not change the default `npm test` contract.

When `TRUST_VSCODE_TEST_EVIDENCE` is configured, the harness also requires
`TRUST_VSCODE_TEST_SOURCE_COMMIT`. At suite completion it atomically writes a
JSON document containing that source commit, the exact selector, every
executed full title, pass/fail state, duration, and reconciled totals. This
artifact proves only the selected extension-host execution; it is not the
final unfiltered VS Code or repository-wide gate.

## Gate and workflow surface proof contract

Every live scanner fact whose source kind is `gate_script` or
`github_workflow_job` must join exactly one live
`verification/gate-inventory.toml` surface by discovery identity, source kind,
path, and name. A catalog mapping for that fact proves only the registered gate
surface: its exact entrypoint or job, declared command role, suite assignment
or explicit exclusion, native environment, enforcement posture, and artifact
contract. It does not transitively prove every product property examined by
commands below that surface.

An assigned gate becomes executed proof only when its exact registered command
or workflow job runs against the bound source revision in its declared native
environment and returns the registered result. A local replay does not replace
a GitHub workflow-job result, and a successful workflow container or setup
step does not replace the job's complete terminal result. Required nonzero
results fail the gate. Conditional, supporting, report-only, advisory, and
excluded surfaces retain those weaker postures and cannot be promoted to
required product proof by catalog presence or a passing exit status.

When the gate inventory declares artifacts, accepted execution evidence binds
the source revision, command or workflow-job identity, terminal result,
artifact paths, and retention posture. A missing, stale, cross-revision, or
unbound artifact cannot close the gate. Surfaces without a declared artifact
retain the exact command result and source binding; they do not invent an
artifact requirement.

## Fuzz-target proof contract

Every live `fuzz_target` scanner fact in the root fuzz program must join exactly
one live target in `verification/fuzz-program.toml` by path and name. The
catalog mapping records its owning surface associations, exact campaign
entrypoint, execution tier, enforcement status, corpus location, artifact
location, and crash-regression handoff. Association and catalog presence alone
are inventory, not executed fuzz proof.

An executed fuzz-target proof is a bounded, source-revision-bound invocation of
the registered target through its native fuzz runner. It records the target,
command, budget, terminal result, and retained machine-readable campaign
evidence. A clean bounded campaign proves only that the target reached its
registered production boundary without an observed crash, sanitizer failure,
or timeout during that campaign. It does not prove universal crash freedom,
semantic correctness for all inputs, or adequate code coverage.

Any discovered crash or sanitizer failure must remain a failing campaign until
it is reduced and joined to the deterministic regression registry required by
the fuzz program. Working corpora and raw crash directories remain
machine-local discovery inputs under their declared storage policy; their size
or existence is not durable product evidence.

## Ignored-test execution proof contract

Every live scanner fact whose ignore state is `ignored` or `conditional` must
join exactly one live `verification/ignored-tests.toml` row by discovery
identity, source kind, path, name, ignore state, and ignore mechanism. The
registry row owns its explicit class, reason, unblock condition, required
environment, topology, and public-claim impact. A catalog mapping for that
fact proves only this registry and execution-routing contract; it does not
prove any product assertion present in the ignored test.

The catalog's structural validation command is separate from the native
execution command named by the registry class and unblock condition. Product
assertions and unresolved helper observations remain outside the structural
mapping. They become proof only when the complete ignored test or its named
parent harness runs in the required native environment against the bound
source revision and produces the required terminal result and retained
artifact. A default-suite skip, filtered result, missing environment variable,
unconfigured lab, absent topology, or successful registry validator cannot
substitute for that execution.

Manual child-process entries become executed only through their named parent
harness or documented manual command. Performance and soak entries require
their explicit measurement lane and retained output. Lab-required entries
require the reviewed physical or external topology and machine-readable
evidence. Until those conditions are satisfied, the fact remains
ignored/unrun proof debt even when its registry identity is catalog-mapped.

## Specification-synchronization proof contract

Repository specification synchronization tests are governance oracles over
their declared immutable source closure. A passing check may establish only
the exact reviewed relationship:

- every named LSP method remains listed in the LSP specification;
- every named indexing and diagnostics configuration token remains listed in
  that specification; and
- every exported standard-library function and function-block name in the
  reviewed denominator remains represented in the standard-function coverage
  document.

The test must bind every specification or coverage source digest it reads and
must fail with the complete missing-name set. Presence proves synchronized
documentation, not implementation reachability, protocol execution, IEC
conformance, or adequate standard-library behavior.

### Owning-spec companion contract

A changed written specification satisfies the direct-change companion rule only
for production paths that it owns. Path-level ownership is a minimum routing
guard, not proof that the prose and assertion are semantically adequate.

Protocol-specific production changes must name their protocol specification.
In particular, changes below `crates/trust-runtime/src/io/mqtt/` or to the MQTT
tag-lowering adapter require the MQTT protocol specification; the generic
runtime-engine specification cannot satisfy that companion by itself. The same
rule applies to the other registered communication owners as their dedicated
specifications are established.

The validator reports every missing owning specification in one run and still
requires a native executable test companion. An unrelated specification, a
planner/catalog mapping, or a generic runtime summary cannot make the result
pass. This routing contract prevents one large catch-all document from becoming
the nominal authority for unrelated behavior.

The specification index lists every direct Markdown specification exactly once.
Numbered documents appear in numeric order and the intentionally non-numbered
SFC profile follows them. Missing, duplicated, or disorderly index entries are
repository contract failures rather than presentation-only drift.

## Integration-harness resource-bound contract

A live integration harness may catalog an assertion over its own resource
budget when that assertion is explicitly separated from product behavior. The
MQTT live-test broker lifetime must be at least four times the per-phase test
timeout so setup, interaction, recovery, and teardown phases cannot outlive
the broker solely because of the declared harness constants.

This assertion proves only the static relationship between the two reviewed
test-harness durations. It does not prove broker reachability, MQTT protocol
behavior, scheduler timing, absence of contention, or successful execution of
any live integration phase.
