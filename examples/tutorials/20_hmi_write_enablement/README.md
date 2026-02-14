# Tutorial 20: HMI Write Enablement with Guardrails

This tutorial starts from the read-only HMI tutorial and upgrades it to
controlled write mode for selected setpoints only.

## Why this tutorial exists

HMI writes are high-impact. The platform defaults to read-only for safety.
Production teams still need write capability, but only with explicit allowlists
and verification steps.

## What you will learn

- how to move from read-only to controlled write mode
- how to define a narrow `allow` list
- how to add writable slider controls in dashboard pages
- how to verify writes and roll back if needed

## Prerequisites

- complete Tutorial 12 first
- `trust-runtime` available

## Step 1: Copy the tutorial project

Why: keep the shipped example unchanged and create a writable lab copy.

```bash
rm -rf /tmp/trust-hmi-write
cp -R /home/johannes/projects/trust-platform/examples/tutorials/12_hmi_pid_process_dashboard /tmp/trust-hmi-write
cd /tmp/trust-hmi-write
```

## Step 2: Build and run baseline read-only project

Why: verify current behavior before introducing writes.

```bash
trust-runtime build --project . --sources src
trust-runtime run --project .
```

Open:
- `http://127.0.0.1:18082/hmi`

Expected result:
- pages render
- no writable controls are active

## Step 3: Enable writes with explicit allow list

Why: write enablement without target restriction is unsafe.

Edit `hmi/_config.toml`:

```toml
[write]
enabled = true
allow = [
  "P1.FeedLevelSetpointPct",
  "P1.ProductLevelSetpointPct"
]
```

Important:
- allow only the minimum set of variables you intend operators to change
- keep command/actuator writes out of this first pass

## Step 4: Add a dedicated control page with sliders

Why: separating write controls from overview pages reduces accidental changes.

Create `hmi/control.toml`:

```bash
cat > hmi/control.toml <<'TOML'
title = "Setpoint Control"
kind = "dashboard"
icon = "sliders"

[[section]]
title = "Authorized Setpoints"
span = 12

[[section.widget]]
type = "slider"
bind = "P1.FeedLevelSetpointPct"
label = "Feed Level SP"
unit = "%"
min = 20
max = 90
span = 6

[[section.widget]]
type = "slider"
bind = "P1.ProductLevelSetpointPct"
label = "Product Level SP"
unit = "%"
min = 20
max = 90
span = 6
TOML
```

Expected result:
- new control page appears in HMI navigation
- sliders are interactive when write policy and runtime auth allow it

## Step 5: Restart runtime and verify write path

Why: descriptor/config changes must be loaded by runtime.

Stop previous runtime and start again:

```bash
trust-runtime run --project .
```

In `/hmi`:
1. open `Setpoint Control`
2. move `Feed Level SP` slider
3. verify value updates on overview/process pages

Expected result:
- write action succeeds only for allowlisted setpoints

## Step 6: Verify guardrails by testing a non-allowlisted target

Why: successful writes are not enough; blocked writes prove policy is working.

Temporarily add a slider bound to a non-allowlisted path, for example
`P1.PressureBar`.

Expected result:
- write should be rejected or remain non-writable
- no unauthorized process variable write should be applied

## Step 7: Define rollback path

Why: if write behavior is not as expected, rollback must be immediate.

Rollback options:
- set `[write].enabled = false`
- remove `hmi/control.toml`
- restart runtime

Expected result:
- HMI returns to read-only behavior

## Common mistakes

- enabling writes with broad `allow` entries
- mixing operator setpoints and actuator commands in one allowlist
- forgetting to re-test blocked-write behavior after enabling writes

## Completion checklist

- [ ] read-only baseline verified
- [ ] write mode enabled with minimal `allow` list
- [ ] dedicated control page created
- [ ] allowlisted writes succeed
- [ ] non-allowlisted writes are blocked
- [ ] rollback path tested
