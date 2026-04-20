# Backup And Restore

## What To Back Up

- project files
- `runtime.toml`
- `io.toml`
- `hmi/` if present
- deployment metadata or versioned bundle roots
- retain data if your process depends on it

## Safe Restore Order

1. restore the project or deployment bundle
2. restore config files
3. restore retain data only if the version/layout is compatible
4. start under supervision
5. verify with status, HMI, and one safe I/O proof

## Related

- [Install On Target](install-on-target.md)
- [Upgrade](upgrade.md)
- [Deploy And Rollback](deploy-rollback.md)
