# Install On Target

Use this page when you need `trust-runtime` running as a service on a real
Linux host such as a Raspberry Pi.

This is the Raspberry Pi install page for the public docs: use it when the
question is "how do I install truST on a Pi or other small Linux target?"

## Raspberry Pi Quick Path

Use this section when the question is exactly "how do I install truST on a
Raspberry Pi?" The supported public-doc path is: download the latest release,
install `trust-runtime`, install the service unit, reboot, and confirm the Pi
auto-starts the runtime.

## 1. Download The Released Runtime Bundle

Start from the latest GitHub release:

<https://github.com/johannesPettersson80/trust-platform/releases/latest>

For Raspberry Pi and other ARM64 Linux targets, download the Linux ARM64
runtime bundle. Until a dedicated Pi image or `.deb` package is published, the
release bundle is the supported install path.

## 2. Install The Runtime

Copy the downloaded bundle to the target and unpack it:

```bash
mkdir -p /opt/trust/releases/runtime
tar -xzf trust-runtime-linux-arm64.tar.gz -C /opt/trust/releases/runtime
```

Install the runtime binary somewhere stable on `PATH`:

```bash
sudo install -m 0755 /opt/trust/releases/runtime/trust-runtime /usr/local/bin/trust-runtime
sudo install -m 0755 /opt/trust/releases/runtime/trust-bundle-gen /usr/local/bin/trust-bundle-gen
```

## 3. Install Your Project

Create the deployment root and place your project there:

```bash
sudo mkdir -p /opt/trust/current
sudo chown -R "$USER":"$USER" /opt/trust
```

Your deployed project should contain at least:

- `runtime.toml`
- `io.toml`
- `program.stbc`

## 4. Install The systemd Unit

The repo ships a service file at
[`docs/deploy/systemd/trust-runtime.service`](https://github.com/johannesPettersson80/trust-platform/blob/main/docs/deploy/systemd/trust-runtime.service).

Install it:

```bash
sudo cp docs/deploy/systemd/trust-runtime.service /etc/systemd/system/trust-runtime.service
sudo systemctl daemon-reload
sudo systemctl enable trust-runtime
sudo systemctl start trust-runtime
```

## 5. Verify Boot And Runtime Health

Confirm the service is up now:

```bash
systemctl status trust-runtime --no-pager
journalctl -u trust-runtime -n 100 --no-pager
```

Then reboot the target and verify it auto-starts again:

```bash
sudo reboot
```

After the host comes back:

```bash
systemctl status trust-runtime --no-pager
```

## Log Location

- systemd journal: `journalctl -u trust-runtime`
- your project/runtime logs, if configured, follow the paths from `runtime.toml`

## Next

- [Lifecycle](lifecycle.md)
- [Supervision](supervision.md)
- [Deploy And Rollback](deploy-rollback.md)
