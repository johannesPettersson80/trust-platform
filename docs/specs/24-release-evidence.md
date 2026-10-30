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
and `SHA256SUMS` covers the provenance record and the listed artifacts. The
released distributables are the VS Code packages, the runtime and language
server archives, and the toolchain bundles of specification 44 §4.6, one per
platform; each kind is named in the record, and a release file of no reviewed
kind fails the provenance step.

The workspace, VS Code package and lockfile versions, annotated tag, successful
release workflow, published GitHub release, required assets, and GitHub
`releases/latest` pointer must agree. Only the Release workflow creates a
release tag, after the release's builds and tests passed (Release after main
CI); a rehearsal creates none.

### Release rehearsal on pull requests

A pull request whose head changes the workspace version runs the Release
workflow as a rehearsal on its head: the preflight, every build, every test and
every Publish script run, no tag is created, nothing is published, and the
release text and evidence are kept as the artifact `release-rehearsal-v<version>`.
A pull request that keeps the version skips the jobs after the workflow's plan.
A version change merges only with a passed rehearsal on its exact head that kept
that artifact. A pull request from a fork is not rehearsed on the project's
runners; a maintainer rehearses its head with a manual run, which rehearses the
commit it runs on. A new push to a pull request replaces the rehearsal of its
previous head.

### Release after main CI

A version is released after `main`'s CI passed on the commit that brings it, so
no one pushes the tag. When the CI workflow completes for a push to `main`, the
Release workflow's plan releases that run's commit when all of these hold: the
CI run concluded `success` for a push to the `main` of this repository; the
repository is the official one; and the workspace version at that commit has no
tag `v<version>`, or that tag is annotated, peels to the same commit, and its
release is not complete. A version whose tag is on another commit was released
before, so nothing happens; a lightweight tag on the commit fails the plan, as
does a failed git or GitHub request. The plan runs the workflow's own commit and
reads the commit it plans for only as data, so the code of a fork's pull request
never runs with the release's token. Every job of the release checks out that
commit, never the branch's newest one, and releases from `main` run one at a
time.

The preflight and every build and test of every target run first; a failure
among them creates no tag and publishes nothing. Then one job creates the
annotated tag `v<version>` on the commit and pushes it, creates the GitHub
release with every required asset and the version's changelog section as its
text, publishes every target's VS Code package to the Marketplace, and verifies
the release. A missing Marketplace token fails the release. Re-running the
failed jobs of a release continues where it stopped: a tag on the same commit,
an existing release and a target already published are accepted. The
workflow's conclusion job fails when any job failed or was cancelled, and
passes when every job passed or the plan skipped it.

The verification requires that the tag is an annotated tag object peeling to
the released commit; that the release at the tag exists, is published,
non-draft and non-prerelease; that GitHub `releases/latest` names the same tag;
that the release's asset names are nonempty and unique and include
`SHA256SUMS`, `release-provenance.json`, `conformance-status.json` and
`conformance-status.md` (duplicate names do not satisfy a set requirement); and
that the Marketplace lists the version for every released target within 60
minutes. Polling never converts an unknown or incomplete response into success.

GitHub API verification requires a nonempty token, preferring `GITHUB_TOKEN`
over `GH_TOKEN`. Credentials are sent only in the authorization header and are
never included in diagnostic output. HTTP error bodies are decoded when
possible; malformed or empty bodies still produce a stable status-bearing
failure. Each nonempty GitHub HTTP 200 response body must decode to a top-level
JSON object. Malformed JSON or a non-object top level produces one stable
failure that names the request endpoint and actual HTTP status, emits no
traceback, and cannot contribute release evidence. An empty HTTP 200 body
retains empty-object decoding and therefore remains subject to the endpoint's
required-field checks.

### Main-push version alignment

On every push to `main`, the CI workflow's `version-release-guard` job requires
the workspace, VS Code package and lockfile versions to agree. It does not wait
for the release, which follows CI. Until 0.26.0 it waited for a release started
by a hand-pushed tag, and 0.25.1's release outlasted its 90 minutes
(29 October 2026).

### Exact-SHA candidate execution

Release-sensitive pushes require a clean exact-SHA candidate artifact whose
recorded local and remote commands all ran against the candidate revision. The
remote VS Code gate uses a unique, bounded-length temporary directory under
`/tmp` for browser and Extension Host process sockets, removes that directory
when the gate exits, and keeps Cargo build output in the separately validated
generated target. A caller-supplied target path must not lengthen browser socket
paths or make an otherwise valid rendered gate fail.

### Release preflight and manifest alignment

Before a release or a rehearsal builds artifacts, the workspace package version,
VS Code `package.json` version, and both the top-level and root-package versions
in `package-lock.json` must be nonempty strings and exactly equal. Every
mismatch is reported in one invocation so a partial alignment cannot pass.
Unreadable, malformed, or wrongly shaped manifests are failures, never an
implicit version.

For a release, the tag `v<workspace-version>` must not exist, or must be an
annotated tag object peeling to the checked-out release SHA, and that SHA must be
reachable from `origin/main`. A release's CI evidence is the run that started
it: the `ci.yml` run of that exact SHA whose `event` is `push`, whose
`head_branch` is `main` or `master`, whose repository is this one, and whose
conclusion is `success`. A successful pull-request, workflow-dispatch, branch,
tag, other-repository or other-SHA run is not final-main CI evidence and
releases nothing. A release also requires the Marketplace token before it
builds. `GITHUB_TOKEN` takes precedence over `GH_TOKEN` and neither may appear
in output.

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
