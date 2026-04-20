# Upgrade

## Safe Upgrade Pattern

1. build and validate the new project version first
2. keep the current deployment available
3. deploy into a versioned root
4. restart in the intended mode
5. verify with status, HMI, and one safe process proof
6. keep rollback ready until the upgrade is accepted

## Rollback Rule

If the new version is not clearly healthy, roll back quickly:

- [Deploy And Rollback](deploy-rollback.md)

## Before You Upgrade

- read [Version History](../reference/version-history.md)
- read [Changelog](../changelog.md)
- confirm your supervision and backup paths are working

## Related

- [Deploy And Rollback](deploy-rollback.md)
- [Backup And Restore](backup-and-restore.md)
- [Version History](../reference/version-history.md)
