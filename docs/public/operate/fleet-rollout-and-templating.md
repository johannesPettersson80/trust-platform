# Fleet Rollout And Templating

## What This Covers

- per-site overrides
- shared base config
- staged rollout
- rollback planning across more than one runtime

## Start Here

- [Runtime Cloud](runtime-cloud.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Upgrade](upgrade.md)

Create and inspect managed local runtime projects before planning a wider
rollout:

```bash
trust-runtime fleet runtime add --fleet-root ./fleet --name cell-1 --template simulate --json
trust-runtime fleet list --fleet-root ./fleet --json
```

The add command creates `./fleet/cell-1` and registers it in
`./fleet/fleet.toml`. Runtime names must be portable and unique. The command
allocates distinct available loopback control/web ports and a secure random
control credential; malformed hand-edited manifests are rejected before a
runtime is added or controlled.

Managed runtime lifecycle commands are:

```bash
trust-runtime fleet runtime start --fleet-root ./fleet --name cell-1 --json
trust-runtime fleet runtime status --fleet-root ./fleet --name cell-1 --json
trust-runtime fleet runtime logs --fleet-root ./fleet --name cell-1 --lines 100 --json
trust-runtime fleet runtime stop --fleet-root ./fleet --name cell-1 --json
```

Only a failed connection is reported as `stopped`. Invalid local credentials or
a connected endpoint that returns a rejected or malformed control response are
errors, preventing an uncertain state from being mistaken for a safe restart.

## What Success Looks Like

- each site has one shared base profile plus only the overrides it needs
- rollout order, rollback owner, and rollback trigger are written before rollout
  starts
- at least one runtime is upgraded and verified before the fleet-wide step

## Related

- [Runtime Cloud Federation](../connect/runtime-to-runtime/runtime-cloud-federation.md)
- [Runtime Cloud](runtime-cloud.md)
- [Deploy And Rollback](deploy-rollback.md)
