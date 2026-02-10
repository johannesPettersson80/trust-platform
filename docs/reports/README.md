# Reports Index

This folder contains durable, developer-facing quality reports and baseline artifacts.

## Keep in this folder

- Audit reports (for example integration/security/performance audits)
- Upgrade/migration reports
- Hardening/overnight validation reports
- Baseline files used by quality gates:
  - `salsa-hardening-perf-baseline.env`
  - `salsa-memory-baseline.env`

## Do not keep in this folder

- Local scratch logs (`*.log`) from ad-hoc runs
- Prompt templates and one-off drafting notes
- Temporary experiment output

Use `logs/` for raw run logs and `docs/internal/` for planning drafts.

## Current canonical reports

Some reports are historical snapshots that describe issues that were fixed later.
For current Salsa status, read `salsa-upgrade-report.md` and the latest overnight hardening report.

- `salsa-integration-audit.md`
- `salsa-upgrade-report.md`
- `salsa-overnight-hardening-20260209.md`
