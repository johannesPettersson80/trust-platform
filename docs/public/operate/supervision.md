# Supervision

Use this page when the runtime must stay up on a real machine and recover
cleanly after restart or crash.

## Recommended Production Baseline

- foreground runtime for development
- supervised service for production
- explicit log retention
- explicit restart policy

## `systemd` Baseline

The repo ships:

- `docs/deploy/systemd/trust-runtime.service`

Use that as the starting point for:

- restart policy
- boot-time enablement
- journald logging

## What To Verify

1. runtime starts on boot
2. runtime restarts after failure the way you expect
3. logs do not grow without bound
4. control/web endpoints are reachable after restart

## Related

- [Install On Target](install-on-target.md)
- [Backup And Restore](backup-and-restore.md)
- [Deploy And Rollback](deploy-rollback.md)
