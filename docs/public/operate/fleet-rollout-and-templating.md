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

## What Success Looks Like

- each site has one shared base profile plus only the overrides it needs
- rollout order, rollback owner, and rollback trigger are written before rollout
  starts
- at least one runtime is upgraded and verified before the fleet-wide step

## Related

- [Runtime Cloud Federation](../connect/runtime-to-runtime/runtime-cloud-federation.md)
- [Runtime Cloud](runtime-cloud.md)
- [Deploy And Rollback](deploy-rollback.md)
