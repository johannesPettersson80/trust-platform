# Operate In Browser HMI

Open `/hmi` to inspect or operate a running project.

## What Your URL Usually Looks Like

- `http://<host>:<port>/hmi`
- or a site-specific hostname your administrator or supervisor gave you

If the page is unreachable:

1. confirm you have the right URL
2. record the time and any error text
3. use your local runbook or escalation contact

## Quick start

1. Open `/hmi`.
2. Read the overview page first.
3. Open the process page.
4. Check trends if values look suspicious.
5. Open alarms before trying operator actions.

![Browser HMI](../assets/images/browser/hmi-home.png)

*Figure:* The HMI overview page. Check overall state, alarms, and live values
here before you open deeper operator pages.

## What Acknowledge means

- Acknowledge clears the banner state for the operator workflow.
- It does not guarantee that the physical cause is gone.
- If the alarm returns, follow your site procedure and escalation path.

## Operator restrictions

- Do not force outputs on a live machine without procedure approval.
- Do not assume clearing an alarm fixes the plant.
- Do not edit project files from an operator-only session.

## Local Runbook

truST gives you the generic operator UI. Your plant still needs a
site-specific runbook with:

- the HMI URL
- allowed usernames or access path
- escalation contact
- alarm actions
- start-of-shift checks

Template:

- [Runbook Template](../examples/runbooks.md)

## Next

- [Operator Guide](../operate/operator-guide.md)
- [Operator Daily Checks](../operate/operator-daily-checks.md)
- [Operator Alarm Handbook](../operate/operator-alarm-handbook.md)
- [Technician I/O Diagnosis](../operate/technician-io-diagnosis.md)
