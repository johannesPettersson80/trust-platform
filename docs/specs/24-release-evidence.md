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

## Hardware And Conformance Claims

Evidence vocabulary is `mock`, `loopback`, `simulation`, `interoperability`,
or `device_in_loop`. Only `device_in_loop` with a named topology supports a
hardware-qualified claim. Lower tiers remain useful tests but cannot be
presented as physical-device proof.

A published conformance status is derived from a machine-readable suite result
and includes commit, toolchain, timestamp, executed/pass/fail totals, and known
gaps. Expected artifacts and unexecuted cases are inputs, not passing proof.

## Behavior-Locked Claims

“Behavior locked” applies only to invariants with explicit test, suite, and
durable evidence mappings. It is not a repository-wide claim and does not imply
release validation or IEC conformance.
