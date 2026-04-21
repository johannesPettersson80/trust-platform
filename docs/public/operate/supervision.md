# Supervision

## Recommended Production Baseline

- foreground runtime for development
- supervised service for production
- explicit log retention
- explicit restart policy

## `systemd` Baseline

The repo ships:

- `docs/deploy/systemd/trust-runtime.service`
- `docs/deploy/systemd/trust-runtime-preempt-rt.service`

Use that as the starting point for:

- restart policy
- boot-time enablement
- journald logging
- RT priority / memlock / affinity posture on `PREEMPT_RT`

## What To Verify

1. runtime starts on boot
2. runtime restarts after failure the way you expect
3. logs do not grow without bound
4. control/web endpoints are reachable after restart

## PREEMPT_RT

When the runtime is deployed on a `PREEMPT_RT` kernel, do not use the baseline
unit unchanged. Use the dedicated RT template and the operator flow in
[PREEMPT_RT Deployment](preempt-rt.md).

## Related

- [Install On Target](install-on-target.md)
- [PREEMPT_RT Deployment](preempt-rt.md)
- [Backup And Restore](backup-and-restore.md)
- [Deploy And Rollback](deploy-rollback.md)
