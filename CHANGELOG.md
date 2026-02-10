# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project adheres to Semantic Versioning.

## [Unreleased]

Target release: `v0.2.0`

### Added

- ST unit-testing tutorials:
  - `examples/tutorials/unit_testing_101/`
  - `examples/tutorials/unit_testing_102/`
- Salsa hardening gates and overnight validation scripts/reports:
  - `scripts/salsa_*_gate.sh`
  - `scripts/salsa_overnight_hardening.sh`
  - `docs/reports/salsa-overnight-hardening-20260209.md`
- Runtime/UI multi-driver coverage and integration tests for Modbus + MQTT.

### Changed

- Migrated `trust-hir` semantic path to Salsa-only backend and upgraded Salsa to `0.26`.
- Enabled VS Code extension integration tests in CI under virtual display (`xvfb`).
- Expanded cancellation checks in workspace-scale LSP operations.
- Documentation organization:
  - Public durable reports remain in `docs/reports/`.
  - Working remediation checklists are no longer published in `docs/reports/`.

### Fixed

- `%MW` memory marker force/write synchronization in runtime I/O panel flow.
- Debug adapter force latch behavior and state-lock interaction.
- Debug runner now respects configured task interval pacing.
- Windows CI/test path issues (`PathBuf` import and path hygiene guardrails).
