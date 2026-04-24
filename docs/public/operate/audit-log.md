# Audit Log

## What To Treat As Auditable

- control actions
- protected runtime-cloud actions
- operator write actions that matter to plant history
- deployment and restart actions

## Important Boundary

truST can provide runtime-side evidence, but plant-grade compliance usually also
needs:

- site policy
- operator identity model
- external retention policy
- local runbook and escalation rules

## Verification

After enabling audit capture, perform one harmless control action, one operator
acknowledgement, and one deployment or restart action. Confirm each event appears
with time, actor or source, target, and result before relying on the log for
site review.

## Related

- [Runtime Cloud](runtime-cloud.md)
- [Runtime UI And Control](runtime-ui-and-control.md)
- [Runbooks](../examples/runbooks.md)
