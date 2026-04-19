# Install On Target

Use this page when you need to put `trust-runtime` on a real machine and keep
it available after reboot.

## Typical Target Flow

1. install `trust-runtime` on the host
2. prepare `runtime.toml`, `io.toml`, and `program.stbc`
3. choose a supervision model such as `systemd`
4. verify restart and power-loss behavior

## Linux / Raspberry Pi

Recommended baseline:

```bash
cargo build -p trust-runtime --release
sudo install -m 0755 target/release/trust-runtime /usr/local/bin/trust-runtime
sudo trust-runtime setup --force
```

Then pair this with:

- [Supervision](supervision.md)
- [Deploy And Rollback](deploy-rollback.md)
- [Backup And Restore](backup-and-restore.md)

## System Service

The repo ships a service template:

- `docs/deploy/systemd/trust-runtime.service`

Use it together with:

- [Supervision](supervision.md)
- [Lifecycle](lifecycle.md)

## Related

- [Offline Install](offline-install.md)
- [Supervision](supervision.md)
- [Deploy And Rollback](deploy-rollback.md)
