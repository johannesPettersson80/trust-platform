# Initial Public Docs Truth Scan

Date: 2026-07-08

Base commit: `6a7492cae6af`

Worktree state: dirty; this is an initial truth-inventory report for the PLC
verification program, not a clean release proof.

Command seed:

```bash
rg -n 'source-build|cargo build|one runtime|right wire|Supported platforms|behavior-locked|OpenOT' README.md docs/public docs/specs
```

## Findings

| Claim ID | Surface | Current disposition | Next action |
| --- | --- | --- | --- |
| `PUBLIC_CLAIM_SOURCE_BUILD_RUNTIME_001` | `docs/public/start/install-from-source.md` | Mapped to local proof `EVID_SOURCE_BUILD_OPENOT_ISSUE_93_20260708`; normal source builds use pinned public OpenOT Git deps. | Run remote/fresh-checkout proof before issue closeout or release claim. |
| `PUBLIC_CLAIM_RUNTIME_WIRE_001` | `README.md`, `docs/public/concepts/trust-mesh.md` | Public claim recorded; proof matrix is incomplete. | Keep `SPEC_GAP_PUBLIC_WIRE_CLAIM_001` open until protocol/device status proof maps all advertised wires. |
| `PUBLIC_CLAIM_SUPPORTED_PLATFORMS_001` | `README.md` | Public claim recorded; release/platform proof matrix is missing from verification metadata. | Keep `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001` open until release assets and platform smoke evidence are mapped. |
| `PUBLIC_CLAIM_BEHAVIOR_LOCKED_001` | `README.md` | Public claim recorded; current tests are not yet traced to runtime/debugger invariants and durable evidence. | Keep `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001` open until verification catalog links tests, suites, and evidence. |

## Notes

- Public claims are not oracles. They create proof obligations or narrowing
  work.
- Claims that imply hardware, platform support, or behavior locking stay
  unproven until mapped through specs, invariants, tests, suites, and durable
  evidence.
- This scan is intentionally small. It proves the workflow shape before a broad
  public-docs claim scanner exists.
