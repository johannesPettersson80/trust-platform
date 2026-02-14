# Tutorial 14: Deploy and Roll Back a PLC Project

This tutorial teaches the operational release flow for PLC projects:

1. build and validate a known-good revision,
2. deploy it to a versioned deployment root,
3. deploy a newer revision,
4. roll back safely.

## Why this tutorial exists

Development success is not enough for production. You need a predictable way to
promote and revert versions when behavior changes or incidents happen.

## What you will learn

- when to use `build`, `validate`, `deploy`, and `rollback`
- how to keep deploy artifacts in a dedicated root
- how to verify which revision is currently active

## Prerequisites

- complete Tutorial 13 first
- `trust-runtime` available on `PATH`

## Step 1: Prepare a dedicated deploy playground

Why: never test deploy/rollback in your main project folder. Keep operations
state separate so history and rollback targets are explicit.

```bash
rm -rf /tmp/trust-tutorial-14-project /tmp/trust-tutorial-14-deploy
cp -R /tmp/trust-tutorial-13 /tmp/trust-tutorial-14-project
mkdir -p /tmp/trust-tutorial-14-deploy
cd /tmp/trust-tutorial-14-project
```

Expected result:
- working copy at `/tmp/trust-tutorial-14-project`
- deploy root at `/tmp/trust-tutorial-14-deploy`

## Step 2: Build and validate revision A

Why: deployment should only use validated artifacts. This is your preflight gate.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

Expected result:
- build and validation pass

## Step 3: Deploy revision A

Why: `deploy` creates a managed deployment record instead of ad hoc file copies.

```bash
trust-runtime deploy --project . --root /tmp/trust-tutorial-14-deploy
```

Then inspect deployment summary:

```bash
cat /tmp/trust-tutorial-14-deploy/deployments/last.txt
```

Expected result:
- summary file exists
- summary points to the active deployment entry

## Step 4: Run active deployment once

Why: post-deploy smoke test proves the deployed artifact is runnable.

Use the active deployment project path shown in `last.txt`:

```bash
trust-runtime --project <active-deployment-project-path>
```

Expected result:
- runtime starts normally from deployed artifact

## Step 5: Create revision B (intentional behavior change)

Why: rollback only matters when there is a meaningful delta between revisions.

Edit `/tmp/trust-tutorial-14-project/src/Main.st`:

```st
LampOut := NOT StartCmd;
```

Rebuild and validate:

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

Expected result:
- revision B compiles and validates

## Step 6: Deploy revision B

Why: this simulates a normal upgrade path.

```bash
trust-runtime deploy --project . --root /tmp/trust-tutorial-14-deploy
cat /tmp/trust-tutorial-14-deploy/deployments/last.txt
```

Expected result:
- `last.txt` now references the newer deployment

## Step 7: Roll back

Why: rollback is your safety action when the latest deployment misbehaves.

```bash
trust-runtime rollback --root /tmp/trust-tutorial-14-deploy
cat /tmp/trust-tutorial-14-deploy/deployments/last.txt
```

Expected result:
- active deployment moves back to the previous known-good revision

## Step 8: Verify rollback behavior

Why: command success alone is not enough; validate runtime behavior after rollback.

Run from the now-active deployment path and test `%IX0.0` -> `%QX0.0` logic.

- if revision A is active, output should match input
- if revision B is active, output should invert input

## Operational rules to keep

- always `build` + `validate` before deploy
- keep deploy root outside source tree
- keep smoke-test checklist for each deployment
- practice rollback before commissioning, not during incident response

## Common mistakes

- deploying directly from unvalidated edits
- mixing manual file copies with `trust-runtime deploy`
- running rollback without checking what is currently active

## Completion checklist

- [ ] revision A deployed and verified
- [ ] revision B deployed and verified
- [ ] rollback executed and verified
- [ ] active deployment confirmed via `deployments/last.txt`
