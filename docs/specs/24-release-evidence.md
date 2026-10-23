# Release Evidence Contract

This document defines the evidence needed for truST platform, source-build,
dependency, artifact, version, hardware, conformance, and behavior-lock claims.
It is a product release contract, not an IEC 61131-3 semantic rule.

## Platform Support Matrix

Each public platform is assigned one evidence tier:

- **native CI**: tests run on that operating-system and architecture;
- **artifact only**: a release artifact is built, checksummed, and inspected,
  but native execution is not established;
- **hardware qualified**: a named hardware model has dated device-in-loop
  evidence;
- **unsupported**: no support claim is made.

Linux x86-64, Linux AArch64, macOS x86-64, macOS AArch64, and Windows x86-64
must map to the actual CI jobs and release assets. Raspberry Pi is Linux
AArch64 artifact support unless a named model has hardware-qualified evidence.
PREEMPT_RT is Linux compatibility evidence; it is not deterministic-latency
certification without a recorded kernel, hardware topology, workload, and
threshold result.

Paths, archive layouts, executable suffixes, and VSIX embedded binaries must be
validated for the target platform. Unsupported targets fail with a named
diagnostic rather than borrowing another platform's claim.

## Source Builds

The documented normal source build of shipped Rust binaries must succeed from
a clean checkout without a sibling OpenOT Structured Text source checkout.
Optional OpenOT examples, conformance fixtures, and telemetry tests may require
that sibling and must say so explicitly.

## Dependency Exceptions

Every ignored dependency advisory has a stable identifier, owner, rationale,
removal condition, review date, and expiry date. Expiry must be no more than 90
days after review. Missing, expired, or overlong exceptions fail the supply
chain check. The Rust workspace runs the owned-exception policy with
`cargo-deny` and `cargo-audit`; the VS Code lockfile must pass `npm audit` at
the configured release threshold without an unowned exception.

## Vendored Dependency Provenance

Every source fact owned by an externally maintained vendored dependency is
classified as a vendored boundary rather than as unspecified truST behavior.
Its provenance record must name the upstream project and exact version or
revision, identify the applicable license files, state whether the captured
upstream tree was clean, and list every repository-local patch commit that
changes the vendored bytes. A missing upstream identity, license boundary, or
local-patch status leaves the vendored fact open; a broad dependency or package
name alone is not sufficient provenance.

The boundary classification does not prove the upstream implementation's
behavior, safety, platform support, or adequacy. truST remains responsible for
tests and safety evidence at every product-facing call boundary, and local
patches remain first-party code changes for review and release purposes.

## Artifact And Version Provenance

A release provenance record binds schema version, tag, full Git commit,
workflow run identifier and URL, timestamp, and an exhaustive list of released
distributable and result-derived conformance artifact paths, kinds, target
platforms, and SHA-256 digests. The provenance record does not list itself or
`SHA256SUMS`, avoiding a self-digest cycle. Both remain required release assets,
and `SHA256SUMS` covers the provenance record and the listed artifacts.

The workspace, VS Code package and lockfile versions, annotated tag, successful
release workflow, published GitHub release, required assets, and GitHub
`releases/latest` pointer must agree. A feature branch may validate this
contract but does not create the tag.

### Main-push version release guard

The `version-release-guard` is a fail-closed state machine over one GitHub
event. It skips only when the event is not a push to `main` or `master`, or
when the previous reachable revision has the same workspace version as the
current checkout. A null, absent, unreadable, or malformed previous revision
does not prove that the version is unchanged; the guard enforces release
evidence for the current version.

For an enforced version `X.Y.Z`, the guard requires `vX.Y.Z` to be an annotated
Git tag. Peeling that tag must produce the exact pushed main SHA under review,
not merely an ancestor of it. A lightweight tag, a tag on another commit, an
unresolvable tag, and a tag that appears only outside the configured discovery
window all fail. Fetch, revision, and tag-type inspection failures are visible
guard failures rather than evidence of absence or success.

The selected Release workflow run must have `event = push`, `head_branch =
vX.Y.Z`, and `head_sha` equal to the peeled tag/main SHA. A same-name run for
another SHA is ignored. Discovery and completion use independent nonnegative
timeouts and a polling interval of at least one second. Missing run identity,
API failure, timeout, cancellation, and every completed conclusion other than
`success` fail. Polling never converts an unknown or incomplete response into a
successful run.

GitHub API verification requires a nonempty token, preferring `GITHUB_TOKEN`
over `GH_TOKEN`. Credentials are sent only in the authorization header and are
never included in diagnostic output. HTTP error bodies are decoded when
possible; malformed or empty bodies still produce a stable status-bearing
failure.

Each nonempty GitHub HTTP 200 response body must decode to a top-level JSON
object. Malformed JSON or a non-object top level produces one stable failure
that names the request endpoint and actual HTTP status, emits no traceback, and
cannot contribute release evidence. An empty HTTP 200 body retains empty-object
decoding and therefore remains subject to the endpoint's required-field checks.

The release at the exact tag must exist, be published, non-draft, and
non-prerelease. GitHub `releases/latest` must name the same tag. Release asset
names are nonempty and unique, and must include `SHA256SUMS`,
`release-provenance.json`, `conformance-status.json`, and
`conformance-status.md`; duplicate names do not satisfy a set requirement.
Only after every state is accepted may the guard report success and emit its
release and workflow URLs.

### Exact-SHA candidate execution

Release-sensitive pushes require a clean exact-SHA candidate artifact whose
recorded local and remote commands all ran against the candidate revision. The
remote VS Code gate uses a unique, bounded-length temporary directory under
`/tmp` for browser and Extension Host process sockets, removes that directory
when the gate exits, and keeps Cargo build output in the separately validated
generated target. A caller-supplied target path must not lengthen browser socket
paths or make an otherwise valid rendered gate fail.

### Tag-triggered preflight and manifest alignment

Before a tag-triggered release builds artifacts, the workspace package version,
VS Code `package.json` version, and both the top-level and root-package versions
in `package-lock.json` must be nonempty strings and exactly equal. Every
mismatch is reported in one invocation so a partial alignment cannot pass.
Unreadable, malformed, or wrongly shaped manifests are failures, never an
implicit version.

The supplied release tag must exactly equal `v<workspace-version>`, must be an
annotated tag object, and must peel to the checked-out release SHA. The
preflight accepts CI evidence only from the `ci.yml` workflow for that exact
SHA where `event` is `push`, `head_branch` is `main` or `master`, status is
`completed`, and conclusion is `success`. A successful pull-request,
workflow-dispatch, branch, tag, or other-SHA run is not final-main CI evidence.
An API failure, malformed run collection, absent URL, missing credential, or
absence of an exact qualifying run fails before artifact construction.
`GITHUB_TOKEN` takes precedence over `GH_TOKEN` and neither may appear in
output.

## Hardware And Conformance Claims

Evidence vocabulary is `mock`, `loopback`, `simulation`, `interoperability`,
or `device_in_loop`. Only `device_in_loop` with a named topology supports a
hardware-qualified claim. Lower tiers remain useful tests but cannot be
presented as physical-device proof.

A published conformance status is derived from a machine-readable suite result
and includes commit, toolchain, timestamp, executed/pass/fail totals, and known
gaps. Expected artifacts and unexecuted cases are inputs, not passing proof.

## Behavior-Locked Claims

“Behavior locked” means that the claimed behavior has a written product
specification and a direct native executable test. Environment-specific proof
classes such as hardware or browser acceptance are stated separately.
Verification invariants, catalogs, suites, and durable evidence may report that
contract, but they neither establish product behavior nor deny an otherwise
agreeing specification-and-test pair. A behavior-lock claim does not by itself
imply release validation or IEC conformance.
