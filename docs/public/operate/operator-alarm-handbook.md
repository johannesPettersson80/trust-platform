# Operator Alarm Handbook

Use this page for the generic operator alarm flow in truST. Site-specific alarm
causes, reset conditions, and escalation contacts still belong in the local
runbook.

## First Response

1. Open the alarms page first and identify the active alarm.
2. Read the alarm text, area, and timestamp before acknowledging anything.
3. Check the physical condition or process graphic that matches the alarm.
4. Follow the local procedure for acknowledge, silence, or hold-to-run actions.

## What Acknowledge Means

- Acknowledge records that an operator saw the alarm.
- Acknowledge does not prove the fault is cleared.
- The alarm remains active until the runtime condition returns to normal.

## Escalate Immediately When

- the alarm blocks a safety function
- the process condition is still present after acknowledge
- the same alarm reappears repeatedly in one shift
- the alarm text and the observed equipment state do not match

## Shift Handover Record

Record at least:

- the alarm name or code
- when it started
- whether it was acknowledged
- whether the condition cleared
- what was handed over to the next operator or technician

## Related

- [Operate In Browser HMI](../start/operate-in-browser.md)
- [Field Fault Procedures](field-fault-procedures.md)
- [Operator Shift Handover](operator-shift-handover.md)
- [Runbooks](../examples/runbooks.md)
