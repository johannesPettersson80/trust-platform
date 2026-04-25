# HMI And Web UI

Operate and monitor a runtime-hosted HMI from the browser.

Authoring descriptor files belongs in [HMI Authoring](../develop/hmi-authoring.md).

## First Screen Checks

![Browser HMI overview](../assets/images/browser/hmi-home.png)

*Figure:* The runtime-hosted HMI dashboard. Look here first for connection
state, alarms, live values, and operator status.

1. Confirm the connection badge is healthy.
2. Confirm freshness is current enough for the task.
3. Open the overview page before changing pages.
4. Check alarms before forcing or acknowledging anything.
5. Compare suspicious values against trends, runtime panel, or field state.

## What This Page Is Not

This is not the HMI authoring workflow. Do not edit `hmi/*.toml` from an
operator session. If the page layout or bindings are wrong, hand the issue to
an engineering workflow and use [HMI Authoring](../develop/hmi-authoring.md).

## Success State

- the HMI URL opens
- connection and freshness indicators are visible
- operator status, alarms, and live values are readable
- the local runbook explains what the operator may acknowledge or change

## Related

- [Operate In Browser HMI](../start/operate-in-browser.md)
- [HMI Authoring](../develop/hmi-authoring.md)
- [Runtime UI And Control](runtime-ui-and-control.md)
- [HMI examples](../examples/hmi.md)
