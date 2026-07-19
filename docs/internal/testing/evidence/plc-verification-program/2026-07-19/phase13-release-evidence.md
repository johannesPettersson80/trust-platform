# Phase 13 Release Evidence Audit

Generator: `phase13-release-evidence-audit v1`
Source revision: `93e644975a9e29063da4461b95871f41774fde59`
Branch label: `plc-verification-program`
Generated: `2026-07-19T22:50:00+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `e7b44bb714312f5e28e6e7d4c347b50946cb96ad97d5af04a7776e6b5ba8ed5e`
Input SHA-256: `sha256:df953636318ce6d89e4b7e5e79aa33e21744479c68aa5cb252f8709f69b15b51`

## Candidate

- Workspace version: `0.24.54`
- Expected tag: `v0.24.54`
- Versions synchronized: `true`
- Changelog names candidate: `true`
- Annotated tag present: `false`
- Release complete: `false`

## Public Release Snapshot

- Latest tag: `v0.24.34`
- Workflow conclusion: `success`
- Matches candidate: `false`
- Missing required assets: `conformance-status.json, conformance-status.md, release-provenance.json`

## Proof Origins

| Origin | Typed evidence rows | Status | Limitation |
| --- | ---: | --- | --- |
| `local` | 120 | `recorded` | Local committed evidence is not remote, CI, hardware, or public proof. |
| `remote_builder` | 130 | `recorded` | Builder evidence is not CI or public release proof. |
| `ci` | 0 | `missing` | Only typed ci_artifact records count; configured jobs do not. |
| `hardware_lab` | 0 | `missing` | Only typed lab_report records count; skipped rows do not. |
| `public_github` | 0 | `snapshot_only` | The checked public snapshot is metadata until a release_object evidence row exists. |

## Platform Matrix

| Platform | Tier | Required proof | Public assets present |
| --- | --- | --- | --- |
| `linux-x64` | `native_ci` | `native_ci_test, release_artifact, sha256` | `true` |
| `linux-arm64` | `artifact_only` | `release_artifact, sha256` | `true` |
| `darwin-x64` | `artifact_only` | `release_artifact, sha256` | `true` |
| `darwin-arm64` | `artifact_only` | `release_artifact, sha256` | `true` |
| `win32-x64` | `native_ci` | `native_ci_test, release_artifact, sha256` | `true` |

## Security And Dependencies

- Owned exceptions: 7
- Expired exceptions: 0
- Cargo policy configured: `false`
- npm audit configured: `true`
- Gate execution claimed: `false`

## Conformance, Hardware, And UI

- Conformance cases cataloged/linked: 21/21
- Published conformance asset present: `false`
- Hardware lab rows skipped/unproven: 5
- UI journeys accepted/total: 0/30

## Known Gaps

- `SPEC_GAPS`: closed - 0 specification gaps remain open
- `CANDIDATE_PUBLICATION`: open - v0.24.54 lacks complete tag/workflow/Latest evidence
- `PUBLIC_RELEASE_ASSETS`: open - Latest snapshot is missing 3 required result assets
- `HARDWARE_LAB`: open - 5 hardware rows remain skipped and unproven
- `UI_ACCEPTANCE`: open - 0 of 30 journeys are accepted
- `CONFORMANCE_PUBLICATION`: open - Latest public release does not carry both conformance status assets

## Boundaries

- `configured_gate_is_execution_proof`: `false`
- `artifact_only_is_native_execution_proof`: `false`
- `skipped_hardware_is_hardware_proof`: `false`
- `provisional_ui_is_acceptance`: `false`
- `version_bump_is_release_completion`: `false`
- `report_emits_product_proof`: `false`

## Limitations

- The report audits checked repository metadata and one reviewed public GitHub snapshot; it does not query GitHub during at-rest validation.
- The evidence index is live-recomputed but excluded from the input digest to avoid a report/evidence self-cycle; Phase 13 evidence rows are excluded from origin counts.
- Configured CI and release jobs are policy, not successful execution evidence; only typed evidence records count as proof origins.
- Artifact-only targets, skipped hardware rows, provisional UI captures, and expected conformance artifacts do not establish native, physical, visual, or conformance proof.
