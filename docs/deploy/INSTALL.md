# truST Runtime Installation

This guide covers production installation paths for Linux, macOS, Windows, and
Raspberry Pi. Choose the path that fits your environment.

## Linux / Raspberry Pi (recommended)

Build and install from source:

```
git clone <repo> trust-platform
cd trust-platform
cargo build -p trust-runtime --release
sudo install -m 0755 target/release/trust-runtime /usr/local/bin/trust-runtime
sudo install -m 0755 target/release/trust-dev /usr/local/bin/trust-dev
```

Optional system I/O setup (GPIO / hardware drivers):
```
sudo trust-runtime setup
```

Non-interactive system I/O setup (writes `/etc/trust/io.toml`):
```
sudo trust-runtime setup --force
```

Start the PLC (zero‑config):
```
trust-runtime
```

## macOS

Build and install:
```
cargo build -p trust-runtime --release
sudo install -m 0755 target/release/trust-runtime /usr/local/bin/trust-runtime
sudo install -m 0755 target/release/trust-dev /usr/local/bin/trust-dev
```

## Windows

Build and install:
```
cargo build -p trust-runtime --release
```

Copy `target\\release\\trust-runtime.exe` and `target\\release\\trust-dev.exe`
into a folder on your PATH.

## Offline Install (USB/SD Card)

1) Build `trust-runtime` on a connected machine.
2) Copy the binary and service template to removable media:
   - `target/release/trust-runtime`
   - `target/release/trust-dev`
   - `docs/deploy/systemd/trust-runtime.service`
3) On the target device:
   - Copy `trust-runtime` to `/usr/local/bin/`
   - Copy `trust-dev` to `/usr/local/bin/` if developer/workbench commands are
     needed on the target
   - Copy the service file to `/etc/systemd/system/`
   - Run `sudo trust-runtime setup --force`
   - Enable and start the service:
     ```
     sudo systemctl daemon-reload
     sudo systemctl enable trust-runtime
     sudo systemctl start trust-runtime
     ```

## Next Steps

- Build your project folder: `trust-runtime build --project /path/to/project`
- Deploy (versioned project folder): `trust-runtime deploy --project /path/to/project --root /opt/trust`
- Start runtime: `trust-runtime --project /opt/trust/current`
