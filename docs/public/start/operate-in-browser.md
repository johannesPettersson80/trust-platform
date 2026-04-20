# Operate In Browser HMI

Use this page when your administrator gave you an HMI URL and you only need to
operate or inspect. This page is not for setup or deployment.

## What Your URL Usually Looks Like

- `http://<host>:<port>/hmi`
- or a site-specific hostname your administrator or supervisor gave you

If the page is unreachable:

1. confirm you have the right URL
2. record the time and any error text
3. use your local runbook or escalation contact

## First Success Workflow

1. open `/hmi`
2. read the overview page first
3. open the process page
4. check trends if values look suspicious
5. open alarms before trying operator actions

![Browser HMI](../assets/images/browser/hmi-home.png)

*Figure:* The HMI overview page. Check overall state, alarms, and live values
here before you open deeper operator pages.

## What Acknowledge Does And Does Not Mean

- acknowledge clears the banner state for the operator workflow
- acknowledge does not guarantee the physical cause is gone
- if the alarm returns, follow your site procedure and escalation path

## What Not To Click Blindly

- do not force outputs on a live machine without procedure approval
- do not assume clearing an alarm fixes the plant
- do not edit project files from an operator-only session

## Local Runbook

truST gives you the generic operator surface. Your plant still needs a
site-specific runbook with:

- the real URL
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
