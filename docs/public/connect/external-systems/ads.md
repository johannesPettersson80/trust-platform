# Beckhoff ADS

truST supports Beckhoff ADS in two directions:

- ADS client: truST connects to a Beckhoff TwinCAT PLC and imports TwinCAT
  symbols into generated ST globals.
- ADS server: truST exposes selected runtime globals as ADS symbols so
  TwinCAT, pyads, .NET ADS clients, and SCADA tools can browse, read,
  subscribe, and optionally write them.

Both directions start from the selected runtime, not from a second ADS-specific
runtime picker. That keeps routes, source IPs, and AMS Net IDs tied to the host
that will actually run the connection.

## Quick Start

Choose the direction first:

- Use **ADS client** when truST should read or write symbols from an existing
  TwinCAT PLC.
- Use **ADS server** when TwinCAT, an HMI, SCADA, pyads, or a .NET ADS client
  should browse and read values from truST.

### truST Connects To TwinCAT

This is the ADS client path. truST imports TwinCAT symbols and turns them into
reviewed ST globals.

1. Select the online runtime in the VS Code Runtime pane.
2. Open **Structured Text: Add Beckhoff ADS Device** or open the runtime-host
   setup page at `/setup/ads`.
3. Discover the TwinCAT target. Broadcast discovery is easiest when UDP 48899 is
   allowed; otherwise enter the TwinCAT IP and AMS Net ID manually.
4. In **Devices & Connections**, choose the logical **ADS port** before browsing
   symbols. Port `851` is the default PLC runtime; other examples include `301`
   for an I/O server, `501` for NC/Motion, and `852` or later for additional PLC
   runtimes. Each port is a separate ADS server with its own symbol namespace;
   this setting does not search all ports.
5. Add the route on the TwinCAT side so TwinCAT trusts the truST runtime host.
   The wizard can generate the exact PowerShell, XML, or manual route values.
6. Run the ADS Doctor from the runtime host.
7. Import TwinCAT symbols into `ads.toml`, a cached symbol snapshot, and the
   single generated ST file `src/generated/ads_generated.st`.
8. Deploy or reload the project bundle, then verify `ads.status` and generated
   value/`_quality` globals from the runtime.

The selected server must expose Beckhoff Online Symbolism/Symbol Upload. An I/O
or Motion port can be reachable without exposing a browseable symbol table; for
NC/Motion, TwinCAT symbol generation may need to be enabled for the relevant
axes. Ports `301` and `501` are examples, not hardware-validation claims for a
particular TwinCAT project.

### TwinCAT Connects To truST

This is the ADS server path. truST exposes selected runtime globals as ADS
symbols, and TwinCAT browses truST like an ADS target.

1. Enable `[runtime.ads_server]` in `runtime.toml`.
2. Set one concrete `listen` IP, an AMS Net ID, and logical `ads_port = 851`.
3. Add the globals to `expose`. Leave `writes_enabled = false` unless a specific
   write path is required.
4. Allowlist the TwinCAT client with its AMS Net ID and source IP or CIDR.
5. Start the truST runtime.
6. In TwinCAT, use **SYSTEM > Routes** or the Target Browser **Broadcast
   Search** to find the truST runtime and add the route to truST. truST
   acknowledges the Add Route handshake so TwinCAT can complete local route
   setup; the client still must be allowlisted in `[runtime.ads_server.clients]`
   before browse/read/write requests are accepted.
7. Browse the truST target on logical port `851`. Exposed globals appear with
   the `global.` prefix, for example `global.TankLevel` and `global.Setpoint`.
8. Read first. Test writes only against a guarded writable variable such as
   `global.Setpoint`.

### TwinCAT Setup Hints

- ADS uses TCP `48898` for router transport and UDP `48899` for discovery and
  route-related setup. The logical PLC ADS port is usually `851`, but the route
  transport and logical ADS server port are different settings.
- Route direction matters. In ADS client mode, TwinCAT needs a route back to the
  truST runtime host. In ADS server mode, the TwinCAT engineering station or
  client needs a route to the truST runtime host.
- Classic ADS is cleartext and route-based. Keep it on a private OT network and
  do not expose ADS ports through NAT or the public internet.
- On TwinCAT Usermode Runtime, static routes may live under
  `C:\ProgramData\Beckhoff\TwinCAT\3.1\Runtimes\UmRT_Default\3.1\`.
  Use the generated route artifact when possible so the correct Usermode paths
  are handled for you.

The CLI exists for scripting and for the VS Code/web front ends to call. It is
not the primary onboarding interface.

## Runtime Context

ADS onboarding is a child of the selected runtime. VS Code renders the wizard
and reads the Runtime pane context; network operations run on the runtime host
through the runtime control/web APIs. This prevents creating a route for the
developer laptop when the deployed runtime is a Raspberry Pi, IPC, or other
remote host.

The setup web page is the canonical production commissioning surface because it
runs on the runtime host. VS Code can still help with authoring-only symbol
imports from the developer laptop, but those imports are explicitly badged as
authoring-only until the runtime-host Doctor passes.

## First Things To Decide

- ADS client: which runtime host connects to the TwinCAT PLC, which symbols are
  imported, and which generated ST global names should be created?
- ADS server: which runtime host exposes truST variables, which globals are
  exposed, which external ADS clients are allowed, and whether any write-back
  symbols are enabled?
- Both directions: is classic plain ADS acceptable on this private network
  segment?

## VS Code Flow

Use these commands from VS Code:

| Command | Use |
| --- | --- |
| `Structured Text: Open ADS Devices` | View ADS connection state for the selected runtime. |
| `Structured Text: Add Beckhoff ADS Device` | Start the runtime-child onboarding wizard. |
| `Structured Text: Diagnose ADS Connection` | Run the ADS Doctor from the runtime host. |
| `Structured Text: Import TwinCAT Symbols` | Preview generated `ads.toml`, snapshot, and ST diffs before writing. |
| `Structured Text: Add ADS Route` | Open the route-planning path for the selected runtime. |

The ADS panel has no runtime selector. If no online runtime is selected, it
opens the Runtime pane instead of silently falling back to the laptop.
When the selected runtime uses remote plain TCP control, VS Code does not send
TwinCAT credentials over that channel. It fetches the runtime host identity with
`ads.identity`, then runs the local `trust-runtime ads add-route --password-stdin`
command so the password travels directly from the engineering computer to the
PLC for that one AddRoute action. The runtime-host Doctor must still pass after
that route is added before the device can be treated as production-ready.

## Setup Web Flow

Open `/setup/ads` on the runtime host for commissioning. The page can identify
the target, show the runtime host local IP and AMS Net ID, plan route artifacts,
run the Doctor, import symbols, and link back to `/ide` for deployment. The
production-ready state requires a deployed bundle plus healthy live ADS status;
a passing offline import alone is not enough.

## Discovery

Do not confuse ADS target discovery with truST runtime discovery:

- truST runtime discovery uses the runtime-to-runtime mDNS/pairing surfaces.
- ADS target discovery uses Beckhoff ADS UDP discovery and directed identify
  from the runtime host toward TwinCAT.

If broadcast discovery is blocked by the network, enter the TwinCAT host/IP and
AMS Net ID manually. The Doctor still proves whether the runtime host can reach
the PLC and whether the route back to the runtime host exists.

## Simulator Smoke

The pyads testserver is useful for a basic no-TwinCAT wire smoke. It can prove
that truST opens a real ADS TCP connection to `127.0.0.1:48898`, derives the
loopback AMS identity, verifies the route path, and reads the ADS runtime state.
It is not a substitute for the TwinCAT lab gate: the stock pyads advanced
handler does not expose a real TwinCAT symbol upload table, so truST cannot use
it to prove symbol upload, handle caching, notifications, online-change symbol
versioning, or guarded writes. Those remain real TwinCAT validation items.

## ADS Client: Symbol Import Model

`ads.toml` maps TwinCAT symbols to generated ST globals:

```toml
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
mode = "poll"

[[connections.points]]
symbol = "GVL.LineReady"
var = "line1_ready"
type = "BOOL"
mode = "notify"
notification_mode = "on_change"
```

Generate the reviewed ST interface from a live TwinCAT symbol import:

```bash
trust-runtime ads import-symbols \
  --target 192.168.10.5 \
  --target-net-id 5.23.91.12.1.1 \
  --connection line1 \
  --out ads.toml \
  --gen src/generated/ads_generated.st
```

VS Code uses `trust-runtime ads import-symbols --dry-run --json` for preview
mode. It opens diffs/previews first and writes files only after confirmation.

Validate the checked-in interface without a PLC connection:

```bash
trust-runtime ads validate --offline \
  --config ads.toml \
  --snapshot ads/snapshots/line1.symbols.json \
  --generated src/generated/ads_generated.st
```

The generated file is deterministic and should be reviewed. It is not hidden
build output. The generated globals are normal ST declarations, so they appear
on the same runtime surfaces as other globals: HMI schemas and values, debug and
control reads, historian sampling, and OPC UA export when the OPC UA server is
enabled.

Enable live runtime ADS with:

```toml
[runtime.ads]
enabled = true
config_path = "ads.toml"
worker_tick_interval_ms = 20
```

At runtime, ADS workers own the socket I/O. The scan cycle only copies the
latest cached read/notification values into generated globals at the input
phase and queues changed write values at the output phase.

## Routes And Security

Classic ADS over port 48898 is cleartext and relies on AMS route trust rather
than modern transport authentication. Plain ADS must be acknowledged explicitly
with `transport = "plain"` and `insecure_transport = true`. Keep it on a private
OT segment, default points to reads, and make write points explicit with
`access = "write"` or `access = "read_write"`.

Secure ADS is not supported in this release. Configure only classic plain ADS on
trusted OT networks; do not expose ADS ports to public or NAT-routed networks.

Route creation is explicit. The setup page, VS Code panel, and CLI can generate
PowerShell, StaticRoutes.xml, and manual GUI artifacts using the runtime host
identity. Automatic route-add sends TwinCAT credentials directly to the PLC for
that one action and never stores them in `ads.toml`, logs, settings, or
generated files. The CLI accepts the password only through `--password-stdin`
and rejects an empty password before sending any AddRoute packet.

For TwinCAT 3 Usermode Runtime, Beckhoff loads remote-access routes from the
runtime instance files, for example
`C:\ProgramData\Beckhoff\TwinCAT\3.1\Runtimes\UmRT_Default\3.1\StaticRoutes.xml`.
Some Usermode installations also display and retain the route from the sibling
`Target\StaticRoutes.xml`. The generated PowerShell searches and updates both
locations when they exist. For Linux/non-Windows ADS clients, generated static
routes use `<Flags>0</Flags>`.

## TwinCAT Usermode Troubleshooting

The following checks came from a real TwinCAT 3 Usermode Runtime validation run
with truST running as a Linux ADS client.

| Symptom | Cause | Fix |
| --- | --- | --- |
| TwinCAT cannot activate normal Run mode in a VM or Hyper-V laptop. | Kernel realtime runtime is blocked by the host environment. | Use TwinCAT Usermode Runtime/XAR user mode instead of forcing kernel Run mode. |
| `ping`/ADS cannot reach a `100.64.0.0/10` laptop address from Linux. | That address range overlaps Tailscale/CGNAT routing and may be filtered or routed differently. | Put both machines on a simple dedicated subnet, for example `192.168.77.10/24` for truST and `192.168.77.11/24` for TwinCAT, then use those addresses in ADS config and routes. |
| TCP `48898` connects, but ADS `read_state` times out with ADS error `1861`. | The TCP router accepted the socket, but TwinCAT did not have a usable route back to the Linux client identity loaded. | Add a static route for the runtime host IP/AMS Net ID, use `<Flags>0</Flags>`, restart TwinCAT Usermode Runtime, and reboot Windows if the router keeps stale route state. |
| Deleting the duplicate route file makes the route disappear from `SYSTEM > Routes`. | Some Usermode installations show routes from `Target\StaticRoutes.xml` even when remote access also needs the runtime-root file. | Keep both Usermode `StaticRoutes.xml` files aligned. The generated PowerShell updates both when present. |
| `SYSTEM > Routes` shows the route but ADS still times out. | TwinCAT may not have reloaded the static route contents into the router. | Restart the Usermode Runtime. If the first ADS request still times out after static-file edits, reboot Windows and start the runtime again. |
| Symbol upload fails on a library or struct metadata symbol such as `ST_LibVersion`. | Whole-STRUCT symbols are not bindable scalar ADS points. | Bind scalar leaf members. The live importer filters unsupported complex symbols and continues with compatible scalar symbols. |

Expected live Doctor proof after the route is correct:

```text
read_state      pass  PLC runtime responded with state 'run'
symbol_upload   pass  Uploaded compatible symbols
handle_resolve  pass  Resolved handle for the selected symbol
sumup_read      pass  Read value(s) with sum-up read
notification    pass  Notification subscription worked
symbol_version  pass  Symbol version returned
```

If the Doctor reports `partial` only because `write_guarded` is skipped, the
read/notification path is working; writes still need an explicit guarded write
test before claiming write-path validation for that PLC.

Run a guarded write only against a safe writable PLC variable. The Doctor
writes the probe value, reads it back, restores the original value, and reads
back the restored value:

```bash
trust-runtime ads doctor \
  --target 192.168.77.11 \
  --target-net-id 100.67.6.217.1.1 \
  --ams-port 851 \
  --write-symbol GVL.Setpoint \
  --write-type REAL \
  --write-value 12.5
```

The `write_guarded` step must pass before claiming live write-path validation.
The CLI rejects partial write-probe arguments; `--write-symbol`,
`--write-type`, and `--write-value` must be supplied together.

## ADS Server: Expose truST To ADS Clients

Enable the ADS server when external Beckhoff tooling needs to browse or read
truST runtime variables through ADS. This is the opposite direction from the ADS
client import path: the external client connects to truST, and truST owns the
symbol table.

The server does not use `ads.toml`. It is configured in `runtime.toml`:

```toml
[runtime.ads_server]
enabled = true
listen = "192.168.77.10"
ams_net_id = "192.168.77.10.1.1"
ads_port = 851
insecure_transport = true
writes_enabled = false
expose = ["global.TankLevel", "global.PumpRunning", "global.StatusWord"]
writable = []

[[runtime.ads_server.clients]]
ams_net_id = "192.168.77.20.1.1"
source_ip = "192.168.77.20"
```

Fail-closed defaults matter:

- `enabled = false` when the section is omitted.
- `listen` is required when enabled and must be one concrete IP address; `0.0.0.0`
  is rejected.
- plain ADS requires `insecure_transport = true`.
- `expose = []` publishes no symbols.
- no external client can connect until it is allowlisted by AMS Net ID and
  source IP or CIDR.
- `writes_enabled = false` by default, and write-back also requires the symbol
  to match `writable`.

The ADS server listens on ADS router TCP port `48898`. The configured
`ads_port` is the logical AMS target port that external ADS clients use; `851`
is the normal TwinCAT PLC runtime port and is also the default truST ADS server
logical port.

TwinCAT Broadcast Search can discover the ADS server on UDP `48899` and its
Add Route action receives a successful compatibility acknowledgement from
truST. That acknowledgement does not mutate server access policy. The TwinCAT
client AMS Net ID and source IP or CIDR still need to match
`[[runtime.ads_server.clients]]` before TCP `48898` ADS requests are served.

Generate route instructions for an external ADS client or TwinCAT engineering
station with:

```bash
trust-runtime ads server route-script \
  --route-name trust-runtime-line1 \
  --server-ip 192.168.77.10 \
  --server-net-id 192.168.77.10.1.1 \
  --format powershell
```

For server mode, the route is added on the external ADS client side so it can
reach the truST ADS target. This is the inverse of ADS client onboarding, where
the TwinCAT PLC needs a route back to the truST runtime host. The server route
artifact does not use the TwinCAT "get fingerprint of remote system" path that
caused ADS 1861 during Linux-client route setup.

Run the status and doctor commands against the running runtime control endpoint:

```bash
trust-runtime ads server status --project examples/communication/ads_server_basic
trust-runtime ads server symbols --project examples/communication/ads_server_basic
trust-runtime ads server doctor --project examples/communication/ads_server_basic
```

The loopback self-test proves the server can answer its own basic ADS requests,
but it is not external-client proof. pyads proof is stronger because it uses an
independent ADS client implementation, but it still does not replace the real
TwinCAT merge gate. Production validation for server mode is:

1. loopback self-test passes,
2. an independent external client such as pyads browses, reads, subscribes, and
   performs any guarded write that is enabled,
3. a real TwinCAT engineering station or runtime browses truST, reads symbols,
   validates sum-up read/write/read-write requests, subscribes to notifications,
   and validates any guarded write path.

Start the TwinCAT side with Target Browser, not Scope. The customer path is:
discover or add the route, open `truST-ADS -> 851`, browse the `global.*`
symbols, read them, and perform a guarded write to a safe writable symbol such
as `global.Setpoint`. Target Browser value preview can be used as a quick
live-value sanity check. TwinCAT Scope View is optional interop only; it adds a
separate Scope Server recording workflow and is not required to prove the ADS
server browse/read/write path. For a PLC-side TwinCAT smoke program, use
`examples/communication/ads_server_basic/README.md`; it shows the required
`Tc2_DataExchange` library and edge-triggered `FB_ReadAdsSymByName` /
`FB_WriteAdsSymByName` calls.

The included pyads smoke script exercises the independent-client step:

```bash
python3 scripts/ads_server_pyads_smoke.py \
  --target-ip 127.0.0.1 \
  --target-net-id 127.0.0.1.1.1 \
  --local-net-id 127.0.0.1.1.100 \
  --read global.TankLevel:REAL \
  --read global.PumpRunning:BOOL \
  --notification global.PumpRunning:BOOL \
  --write global.Setpoint:REAL:12.5 \
  --doctor-endpoint unix:///tmp/trust-runtime-ads-server-basic.sock \
  --doctor-token ads-server-smoke-token \
  --trust-runtime target/debug/trust-runtime
```

The script report explicitly keeps `"twinCAT_merge_gate_satisfied": false`;
that field prevents pyads evidence from being confused with real TwinCAT lab
evidence.

Writes from ADS clients are queued through the ADS server write port and
applied at the runtime boundary. Accepted and rejected writes emit
`ads.server.write` audit records. Keep `writes_enabled = false` unless a
specific external client and symbol need a write path, and use a safe variable
such as `global.Setpoint` for guarded validation.

## ADS Server Example

--8<-- "examples/communication/ads_server_basic/README.md:3"

## Quality Variables

Each ADS value gets a sibling quality variable:

```iecst
TYPE
    ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);
END_TYPE

VAR_GLOBAL
    line1_temp : REAL;
    line1_temp_quality : ADS_QUALITY := Stale;
END_VAR
```

Use the value and quality together in logic and operator views. A generated
quality starts as `Stale`, moves to `Good` after a successful read, and moves to
`Error` when the ADS point reports a point-level error. The runtime updates
these quality globals at scan boundaries alongside the ADS values.

## ADS To OPC UA Re-Export

ADS-imported globals can be re-exported through OPC UA because they are ordinary
declared globals after generation:

```toml
[runtime.opcua]
enabled = true
endpoint = "opc.tcp://0.0.0.0:4840"
namespace_uri = "urn:trust:line1"
expose = ["line1_temp", "line1_temp_quality", "line1_ready"]
```

Wire-level OPC UA serving still requires a runtime built with `opcua-wire`.

## Example

--8<-- "examples/communication/ads_line1/README.md:3"

## Related

- [`ads.toml`](../../reference/config/ads-toml.md)
- [`runtime.toml`](../../reference/config/runtime-toml.md#runtimeads_server)
- [`trust-runtime ads`](../../reference/cli/trust-runtime.md#ads)
- [OPC UA](opc-ua.md)
- [Protocol Matrix](../protocol-matrix.md)
