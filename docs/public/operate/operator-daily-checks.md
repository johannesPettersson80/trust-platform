# Operator Daily Checks

Per-shift checks cover runtime connection, freshness, alarm state, live
values, and the shift record.

## Per-Shift Checklist

| Check | Expected state | If not |
| --- | --- | --- |
| Runtime connection | Connected/healthy indicator visible; freshness keeps updating | Escalate to the technician or runtime owner |
| Active alarms | No unexpected alarms, or alarms match known maintenance work | Record the alarm and follow the site procedure |
| Live values | Process values update and stay within the expected band for the current shift | Compare with field indications, then escalate if the mismatch persists |
| Shift record | Anything abnormal is written to the site runbook or handover log | Do not leave the condition undocumented |

## Escalation Fields

Fill these with site-specific details before using this page in production.

| Role | Contact | Phone | Use when |
| --- | --- | --- | --- |
| Supervisor | fill in | fill in | operator cannot confirm the correct operating state |
| Technician on call | fill in | fill in | runtime disconnected, stale values, or I/O mismatch |
| Maintenance lead | fill in | fill in | repeated equipment alarms or manual intervention required |

## Site-Specific Procedure

Pair this generic page with a local runbook:

- [Runbooks](../examples/runbooks.md)

## Related

- [Operator Guide](operator-guide.md)
- [Operator Shift Handover](operator-shift-handover.md)
