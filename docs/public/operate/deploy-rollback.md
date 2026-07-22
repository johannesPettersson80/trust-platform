# Deploy And Rollback

## Core commands

```bash
trust-runtime deploy --project ./my-plc --root ./deploy-root
trust-runtime rollback --root ./deploy-root
```

`deploy` writes a versioned deployment entry. `rollback` moves the active
deployment pointer back to the previous version.

Each successful deploy writes its change summary to
`deployments/<label>.txt` and copies that summary to `deployments/last.txt`.
The summary names the newly deployed bundle and the prior bundle and reports
changes to runtime configuration, I/O configuration, bytecode, and sources.
`last.txt` records the last deploy operation; it is not the active pointer and
is not rewritten by rollback. Inspect the `current` symlink to identify the
active bundle.

Deployment labels are single names, not paths; values containing path
separators, `.`/`..`, or absolute paths are rejected. The deployment root keeps
the newly active bundle and the immediately previous bundle. This remains true
when `--root` is relative, and a dangling `current` or `previous` symlink does
not block replacement when that pointer is updated.

![Deploy CLI surface](../assets/images/terminal/deploy.gif)

*Figure:* The `deploy` command surface and required flags. Use this to confirm
the exact CLI contract before you automate rollout on a target root.

![Rollback CLI surface](../assets/images/terminal/rollback.gif)

*Figure:* The `rollback` command surface. Keep this close to the deployment root
you operate so you know which flag set the runtime expects.

## Pre-deploy checklist

- build succeeded
- validate succeeded
- the target config and safe-state policy were reviewed
- the rollback root has at least one known-good prior deployment

## Good rollback scenarios

- bad bytecode bundle
- correct logic but wrong runtime config
- rollout succeeded technically but operator behavior regressed

## Worked tutorial

--8<-- "examples/tutorials/14_deploy_and_rollback/README.md:3"
