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
`cargo-deny` and `cargo-audit`, and so does every standalone Cargo workspace
of the repository: the two firmware images (`firmware/*`) and the two fuzz
harnesses (`fuzz`, `crates/trust-ads-server/fuzz`). Each committed
`Cargo.lock` is a graph of its own and is read `--locked`, so a lock that its
manifests have left behind fails the supply chain check, as an advisory does.
A new standalone workspace joins `scripts/supply_chain_gate.sh`; the gate's
test fails on a lock that the gate does not read. Each lock lists the workspace
crates it builds at the workspace version, so the version bump
(`scripts/bump_version.py`) moves them in every lock, and the gate's test also
fails on a lock that the bump leaves out. The VS Code lockfile runs
the same policy: every advisory `npm audit` reports, at any severity, needs an
owned exception in `editors/vscode/audit-exceptions.toml`, with the same fields and limit and
the advisory's GitHub id. `scripts/check_npm_audit.py` checks it in CI, in the
release build, and in the preflight of a promoted release, which builds
nothing; an advisory without an owned exception fails, and an exception whose
advisory npm no longer reports is named for removal. The release evidence's
dependency trace (`SEC_DEP_AUDIT_001`, `scripts/release_claim_contract.py`)
checks both exception lists against this policy and that CI and the release
build run the VS Code audit; it reads no registry, so no test depends on the
network: the live audits are those of CI, the release build and the preflight
(8 November 2026: a macOS runner's failed name lookup failed the trace's test
when it ran `npm audit` itself).

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

A release provenance record binds schema version, tag, full Git commit, the
commit's tree, the identifier and URL of the workflow run that released it,
whether its packages were promoted from a rehearsal (Release after main CI)
and, for a promoted release, the rehearsal run that built them and the commit
it rehearsed, timestamp, and an exhaustive list of released
distributable and result-derived conformance artifact paths, kinds, target
platforms, and SHA-256 digests. The provenance record does not list itself or
`SHA256SUMS`, avoiding a self-digest cycle. Both remain required release assets,
and `SHA256SUMS` covers the provenance record and the listed artifacts. The
released distributables are the VS Code packages, the runtime and language
server archives, and the toolchain bundles of specification 44 §4.6, one per
platform; each kind is named in the record, and a release file of no reviewed
kind fails the provenance step. The runtime archive holds `trust-runtime`,
`trust-dev` and `trust-bundle-gen`; the language-server archive holds the two
programs an editor starts, the language server `trust-lsp` and the debug
adapter `trust-debug`, so that a DAP-capable editor other than VS Code, whose
package carries both, can debug a truST runtime.

The workspace, VS Code package and lockfile versions, annotated tag, successful
release workflow, published GitHub release, required assets, and GitHub
`releases/latest` pointer must agree. Only the Release workflow creates a
release tag, after the release's builds and tests passed, in its own run or in
the rehearsal whose files it promotes (Release after main CI); a rehearsal
creates none.

### Release rehearsal on pull requests

A pull request whose head changes the workspace version from the version
where it branched from its base (so a branch from before a version bump on
`main` changes nothing), or changes the
Release workflow, its `release-files` action or a script either runs (listed in
`.github/paths/release.txt`), runs the Release workflow as a rehearsal on its
head: the preflight, every build, every test and
every script that writes the release's files run, no tag is created, nothing is
published, and the release text and evidence are kept as the artifact
`release-rehearsal-v<version>`. The rehearsal also keeps every file its release
would publish, for a release that promotes it (Release after main CI): the VS
Code package, runtime and language-server archives and toolchain bundle of
every platform, the conformance status, its release provenance and
`SHA256SUMS`, and the release text, as one artifact named
`release-bundle-v<version>-<tree>` after the tree of the commit it rehearsed
(`git rev-parse <commit>^{tree}`), kept for 30 days. A rehearsal writes and
keeps those files in a job of its own, only when every build and test of its
run passed (0.30.0's first rehearsal attempt kept its files after a cancelled
build, 3 November 2026), and that job holds no token that writes: no
Marketplace token and a read-only GitHub token; only a release's publishing
job gets write access, and only its Marketplace step the Marketplace token.
The real-database matrix runs on the project's own runner. A pull request's or
merge group's code never runs on that runner (Codex's review of #146), and a
manual rehearsal runs the code of the ref a maintainer chooses. The matrix runs
in a maintainer's manual rehearsal and in every release, a promoted one
included, on the release's commit before anything is published (owner, 4
November 2026: "what you decide is okay with me"). It is given no secret.
A pull request that keeps the version but changes what the packages hold (the
extension, the crates, the examples, the libraries, the Cargo manifests and
toolchain; `.github/paths/package.txt`) builds only the linux-x64 toolchain
bundle and package, the platform of the hosted runners, and runs the
acceptance journeys on that package (specification 22, "Acceptance journeys on
the packaged extension"); it skips the runtime validation, which CI's tests
hold. Any other pull request skips the jobs after the workflow's plan. The plan
names the platforms that the toolchain-bundle and build jobs build: the five of
the platform support matrix for a rehearsal or a release, linux-x64 for a
package. Each build starts beside the toolchain-bundle jobs and takes its own
platform's bundle when it packages the extension, once that platform's bundle
job has uploaded it (`scripts/wait_for_run_artifact.py`); a bundle job that ends
without it fails that build at once. The slowest bundle therefore delays only
its own platform's package: on 6 November 2026 every build waited 49 minutes for
the Intel-Mac bundle. A rehearsal and a release that builds its packages run the
acceptance journeys on their linux-x64 package too, and the rehearsal's files
and the release's publishing wait for them. A version change merges only with a passed rehearsal on its
exact head that kept that artifact. A pull request from a fork is not rehearsed on the project's
runners; a maintainer rehearses its head with a manual run, which rehearses the
commit it runs on. A new push to a pull request replaces the rehearsal of its
previous head. The plan never runs a fork's code: for a fork's pull request it
runs the pull request's base, whose planner skips it without reading it; it
otherwise runs the workflow's own commit and reads the commit it plans for as
data. Its checkout keeps no credentials, and only the runs for `main`, after its
CI and for a push, which ask GitHub whether a release is complete and which
rehearsal kept a commit's tree, have the workflow's token.

### Release after main CI

A version is released from the `main` commit that brings it, so no one pushes
the tag, and its packages are built and tested once (owner decision, 2 November
2026: "yes"). When that commit has the tree of a commit that a passed rehearsal
of the version built and tested, the release promotes the rehearsal's files as
soon as `main` has the commit, without waiting for `main`'s CI to test the same
tree again (promotion, below); the release after `main`'s CI of that commit
promotes the same files. Otherwise, as when a pull request merged between the
rehearsal and the release, the release follows `main`'s CI on the commit and
builds and tests every package itself. 0.29.0's release commit `4465d345a` and
its rehearsed head `06c31e9be` share the tree `1794e85b`, yet `main`'s CI tested
that tree again for about two hours and the release rebuilt every package for
about one and a half; 0.30.0 did the same.

When the CI workflow completes for a push to `main`, the
Release workflow's plan releases that run's commit, promoting the same
rehearsal as a push would when there is one (below), when all of these hold: the
CI run concluded `success` for a push to the `main` of this repository; the
repository is the official one; and the workspace version at that commit has no
tag `v<version>`, or that tag is annotated, peels to the same commit, and its
release is not complete, and no earlier version of that commit's first-parent
line since its newest tagged version lacks a tag. GitHub keeps one waiting run
per concurrency group, so a later run can replace the waiting release of an
earlier version change: the plan then fails, naming that version and its commit,
instead of releasing past it, and running that commit's Release again releases
it. A release is complete once a Release run of its commit,
in any attempt, passed the verification below; the GitHub release exists before
the Marketplace is published, so a release whose publishing or verification
failed is not complete, and running the workflow again releases it from its
tag. A Release run of a commit is one that `main`'s CI of the commit started,
which is named `Release after CI of <commit>`, or one that a push of the
commit to `main` started, whose head is that commit. A run after CI never
counts by its head: that is `main`'s newest commit when it starts, which can be
a later commit than the one it releases. No other run counts: the workflow of a
pull request's run or of a manual run comes from the code it runs, which could
give a job the verification's name. The check reads the runs after CI since the day
before the commit's date; GitHub lists at most 1,000 runs for such a query, so
when it lists that many the check fails instead of answering. A version whose
tag is on another commit was released before, so nothing happens; a
lightweight tag on the commit fails the plan, as does a failed git or GitHub
request. The plan runs the workflow's own commit and
reads the commit it plans for only as data, so the code of a fork's pull request
never runs with the release's token. Every job of the release checks out that
commit, never the branch's newest one, and releases from `main` run one at a
time.

For a push to `main`, the plan checks the same conditions for the pushed
commit: the push is to the `main` of this repository, the repository is the
official one, and the version's tag, its release and the earlier versions are
as above. It then promotes the release when a rehearsal kept its files for the
commit's tree: GitHub lists an unexpired artifact named exactly
`release-bundle-v<version>-<tree>` of a Release run of this repository that a
pull request or a manual run started and that completed with `success`; for
a pull request's rehearsal, of the pull requests that GitHub lists for the
head, exactly one has that head, and it came from the run's head branch of this
repository, targets this repository's `main` and is merged when the release
plans; GitHub's records give that tree to the run's head commit and to the
commit whose workflow the run ran; and a CI run of the same head passed, whose
head commit has that tree, also one that passed when its failed jobs ran again: the plan lists every CI
run of the head and reads each one's conclusion, since GitHub's listing filtered
by `status=success` leaves such a run out. The workflow's commit is the one that GitHub records for the
run's local call of `.github/workflows/openot-real-databases.yml`, which the
Release workflow calls: a local call is recorded with that commit's full sha in
its path and with the run's own ref, which for a pull request's rehearsal is
the test merge `refs/pull/<number>/merge` of its pull request, a commit that
merges the head, and for a manual rehearsal its branch, whose head is the
rehearsed commit. A pull request into a branch with another Release workflow
runs a merge of another tree. This proof rests on GitHub recording a local
call's path with the commit's sha: an explicit call of this repository's
workflow records the ref name in its path, or no ref when it names a sha, and
proves nothing. GitHub may list a merged pull request a few seconds after the
merge, so while it lists none the plan asks again, three times in about 30
seconds, and then promotes nothing. The plan takes these facts from GitHub's
API, never from a pull request's text, and of several such artifacts it takes
the one with the highest id; but when the version's tag is already on the
commit and its GitHub release exists, as after a promotion that stopped
halfway, it takes the rehearsal that the release's `release-provenance.json`
names while that one still qualifies, so that the release continues with the
files it began with. Without one, the push releases nothing and the release
follows `main`'s CI. An error while the plan looks for a rehearsal, as a failed
GitHub request, never fails the plan: it promotes nothing and names the error
in a notice, so that a push's run releases nothing and the release after
`main`'s CI builds every package. The plan job may read pull requests for this
(`pull-requests: read`). A promoted release runs no build or test of its
packages; its preflight audits the VS Code lockfile again, since the audit of
the rehearsal's build can be days old, and it runs the real-database matrix on
the commit, which a pull request's
rehearsal does not run: its publishing job waits for the matrix to pass, then
downloads that artifact into an empty folder outside the checkout, so that
none of its entries can replace a file there, and publishes it only when it
holds the release folders and the release text and nothing else, every line of
the rehearsal's `SHA256SUMS` names a file of the artifact with that digest,
every file of the release folders is listed, every platform has its VS Code
package, runtime and language-server archives and toolchain bundle, the
rehearsal's provenance names the release's tag, the commit's tree and that run,
and the release text written from the commit's changelog is the rehearsal's;
the checked folders move into the checkout only then, replacing nothing. It
writes the release's own provenance and checksums, then tags, releases,
publishes and verifies as below. Its `conformance-status.json` is the
rehearsal's, which names the rehearsed head, where the conformance suite ran;
its provenance names both the released commit and the rehearsed head.

A release never happens twice. One job at a time publishes from `main`: the
publishing job of a promoted release holds the concurrency group of the runs
after `main`'s CI, and every publishing job starts by checking that no Release
run of its commit has verified the release, since a plan decides before that
job holds the group. A promoted release that completed makes the release after
`main`'s CI of the same commit find it complete and do nothing, and a re-run of
an older run whose release another run completed fails before it publishes
anything. A promoted release that failed continues when its failed jobs are
re-run, with the same rehearsal's files; when it is not re-run before `main`'s
CI passes, or GitHub cancels its waiting publishing job for a newer run of the
group, the release after `main`'s CI promotes the same rehearsal's files, so
that both publish the same bytes. The packages of a published version never
change, and its evidence may be written again. Before it tags anything, every
publishing job asks GitHub for the release of the tag: when one exists, every
package on it (a VS Code package, a runtime or language-server archive, a
toolchain bundle) must be one the job publishes, with the SHA-256 digest that
GitHub recorded for the uploaded asset, or the job fails and publishes nothing
more. The evidence (the provenance, the conformance status and `SHA256SUMS`)
is written anew by every run and attempt, so it is not compared, and an asset
still uploading is not published. So the GitHub release and the Marketplace
never hold different bytes of a version: a release that no rehearsal can
finish any more, as when its artifact expired, and that would build its
packages again, stops for a maintainer to decide.

The preflight, the real-database matrix and every build and test of every
target run first, the builds and tests in the release's own run or in the
rehearsal it promotes; a failure among them creates no tag and publishes
nothing. Then one job creates the
annotated tag `v<version>` on the commit and pushes it, creates the GitHub
release with every required asset and the version's changelog section as its
text, publishes every target's VS Code package to the Marketplace, and verifies
the release. A missing Marketplace token fails the release. Re-running the
failed jobs of a release continues where it stopped: a tag on the same commit,
an existing release that holds the same packages and a target already
published are accepted. The
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
releases nothing. A promoted release's evidence is instead the rehearsal whose
files it publishes (Release after main CI). A release also requires the
Marketplace token before it builds. `GITHUB_TOKEN` takes precedence over `GH_TOKEN` and neither may appear
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
