# Communication Example: Beckhoff ADS Symbol Import

This example shows the ADS customer workflow after the VS Code ADS panel or the
runtime-host `/setup/ads` wizard has imported symbols: a cached TwinCAT symbol
snapshot and `ads.toml` generate reviewed ST globals that compile without a PLC
connection.

## What you learn

- how TwinCAT symbols map to generated ST globals
- how cached symbol snapshots make CI/offline builds deterministic
- why plain ADS must be explicitly acknowledged before use
- how `[runtime.ads]` turns the reviewed `ads.toml` bindings into live runtime
  scan-boundary updates when the runtime is built with `ads-wire`
- why production-ready requires a deployed runtime-host Doctor/status proof,
  not just an offline generated file

## Files in this folder

- `ads.toml`: ADS connection and point bindings
- `ads/snapshots/line1.symbols.json`: cached TwinCAT symbol metadata
- `src/generated/ads_generated.st`: generated value and `_quality` globals
- `src/main.st`: sample logic using generated ADS globals
- `src/config.st`: task binding
- `runtime.toml`, `io.toml`, `trust-lsp.toml`: runtime/project defaults

## TwinCAT side checklist

Use this checklist when truST is the ADS client and TwinCAT is the PLC target:

1. Put the TwinCAT PLC runtime in Run mode.
2. Confirm the PLC logical ADS port is `851`.
3. Add a route on the TwinCAT side back to the truST runtime host. The route
   must use the truST runtime host IP and AMS Net ID, not the developer laptop
   unless the laptop is the runtime host.
4. Allow TCP `48898` and UDP `48899` between the TwinCAT machine and the truST
   runtime host.
5. Prefer a simple private subnet during first setup. The live lab used
   `192.168.77.10/24` for truST and `192.168.77.11/24` for TwinCAT.
6. Use scalar TwinCAT symbols or scalar leaf members for the first import.

If broadcast discovery is blocked, enter the TwinCAT IP and AMS Net ID manually
in the VS Code ADS panel, `/setup/ads`, or `trust-runtime ads doctor`.

## Step 1: Validate the generated interface offline

Why: prove the checked-in generated ST still matches `ads.toml` and the cached
symbol snapshot before connecting to plant hardware.

```bash
trust-runtime ads validate --offline \
  --config ads.toml \
  --snapshot ads/snapshots/line1.symbols.json \
  --generated src/generated/ads_generated.st
```

## Step 2: Build the project

Why: confirms the generated ADS globals and `ADS_QUALITY` type are valid ST.

```bash
trust-runtime build --project . --sources src
```

## Step 3: Run against a TwinCAT target

Why: `[runtime.ads]` in `runtime.toml` loads `ads.toml` at startup and starts a
background ADS worker per connection. The scan thread only copies cached values
into the generated globals and queues write intents back to the worker.

```bash
trust-runtime play --project . --no-console
```

Live ADS I/O requires a `trust-runtime` build with feature `ads-wire` and a
valid TwinCAT route. Without `ads-wire`, startup fails clearly instead of
silently ignoring ADS.

Before treating the target as ready, run the Doctor from the same runtime host
that will run ADS:

```bash
trust-runtime ads doctor \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --ams-port 851
```

For a healthy read/notification path, expect `read_state`, `symbol_upload`,
`handle_resolve`, `sumup_read`, `notification`, and `symbol_version` to pass.
The command may still report `partial` when guarded writes are disabled; that
means the read path is proven but the write path has not been validated.

To validate a safe write point, add an explicit guarded write probe. The Doctor
writes the probe value, reads it back, restores the original value, and verifies
the restore:

```bash
trust-runtime ads doctor \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --ams-port 851 \
  --write-symbol GVL.Setpoint \
  --write-type REAL \
  --write-value 12.5
```

Use a harmless setpoint or test variable for this probe.

## Step 4: Regenerate after a TwinCAT symbol change

Why: regenerated ST is intentionally git-reviewed, not hidden build output.
Run this after a live symbol import from the TwinCAT target; CI should still use
`validate --offline` against the cached snapshot.

```bash
trust-runtime ads import-symbols \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --connection line1 \
  --out ads.toml \
  --gen src/generated/ads_generated.st \
  --force
```

Use `--force` only when replacing a reviewed generated file with the new
deterministic output.

## TwinCAT Usermode Runtime Notes

The Windows/TwinCAT Usermode lab exposed several setup details that are easy to
miss:

- If normal Run mode is blocked by Hyper-V or a VM host, use TwinCAT Usermode
  Runtime/XAR user mode.
- If the PLC and runtime are on a `100.64.0.0/10` address, Linux may treat it as
  Tailscale/CGNAT traffic. A simple dedicated subnet such as
  `192.168.77.10/24` for truST and `192.168.77.11/24` for TwinCAT is easier to
  prove.
- For Linux/non-Windows ADS clients, static routes should use
  `<Flags>0</Flags>`.
- TwinCAT Usermode installations can keep two relevant route files:
  `...\Runtimes\UmRT_Default\3.1\StaticRoutes.xml` and
  `...\Runtimes\UmRT_Default\3.1\Target\StaticRoutes.xml`. Keep both aligned if
  the TwinCAT UI shows the route from one file while the router uses the other.
- If TCP `48898` connects but `read_state` times out with ADS error `1861`,
  restart the Usermode Runtime. If the router still behaves as if the old route
  is loaded, reboot Windows and start TwinCAT again.
- TwinCAT symbol tables contain library/STRUCT metadata such as
  `ST_LibVersion`. The importer skips unsupported complex symbols and imports
  compatible scalar symbols; bind leaf members instead of whole structs.

## Security notes

This fixture uses `transport = "plain"` and `insecure_transport = true` because
classic ADS is cleartext. Keep this traffic on a private OT segment and require
explicit write bindings (`access = "write"`) for any remote command path.
