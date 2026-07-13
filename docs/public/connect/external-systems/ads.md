# Beckhoff ADS

truST supports Beckhoff ADS in two directions:

- ADS client: truST connects to a Beckhoff TwinCAT PLC and imports TwinCAT
  symbols into generated ST globals.
- ADS server: truST exposes selected runtime globals as ADS symbols so
  TwinCAT, pyads, .NET ADS clients, and SCADA tools can browse, read,
  subscribe, and optionally write them.

ADS configuration belongs to the selected runtime, not to a second ADS-specific
runtime picker. Discovery in VS Code is simpler: one action searches this
computer and its local network. If production will run elsewhere, the runtime
host Doctor separately proves that host's route and connectivity.

## Quick Start

Choose the direction first:

- Use **ADS client** when truST should read or write symbols from an existing
  TwinCAT PLC.
- Use **ADS server** when TwinCAT, an HMI, SCADA, pyads, or a .NET ADS client
  should browse and read values from truST.

### truST Connects To An ADS Device

This is the ADS client path. truST imports symbols from an ADS device and turns
them into reviewed ST globals.

1. Open **Devices & Connections** and select **Discover ADS devices**. There is
   no location question and nothing to enter for the normal path.
2. truST searches this computer and the local network in the same operation. It
   identifies ADS devices, then checks logical ADS services `851`–`854`, `301`,
   and `501` on each device.
3. Read the device card. **Responding ADS services** lists every logical service
   that answered. Services that did not answer or could not be checked stay
   under **Technical details** instead of making the whole scan look like
   `0 found`.
4. If exactly one responding service exposes variables, truST selects it. If
   several expose variables, choose the service you want and select **Browse
   variables**. A service may answer without supporting Symbol Upload; that is
   shown as a responding service, but it cannot be used to browse variables.
5. Use **Advanced** only for recovery or a non-standard installation. It accepts
   a known Host/IP, an optional AMS Net ID, and up to four additional logical
   ADS service ports. A known address is added to the automatic local scan; it
   does not replace it. Enter a bare Host or IP, not `host:port`.
6. For an ADS device on another computer, set up the reciprocal ADS route when
   the result asks for it. The route must identify the computer or runtime that
   will make the production connection. A found device remains visible as
   **Route setup required** instead of disappearing.
7. For TwinCAT on the same Windows computer as truST, do not add a static route
   back to that computer. truST uses the installed native Windows ADS router and
   the router-assigned AMS identity; this path does not require a self-route.
8. Import the selected variables into `ads.toml`, a cached symbol snapshot, and
   `src/generated/ads_generated.st`, then restart or redeploy the runtime.
9. Open **Live Values** and expand **Connected variables → ADS** to verify the
   imported value, type, and quality reported by the running runtime.

The automatic port check is intentionally bounded; truST does not sweep all
`1..65535` logical services:

| Logical ADS port | Common use | What a result means |
| --- | --- | --- |
| `851`–`854` | TwinCAT PLC runtime 1–4 | The service must also support Symbol Upload before truST can browse its variables. |
| `301` | Common ADS service, often an additional task | It can respond without exposing a browsable variable table. |
| `501` | Common ADS service, often NC SAF | It can respond without exposing browsable axis variables. |
| Custom | Device-specific service entered under **Advanced** | truST reports it only after that logical service actually responds. |

**Unavailable** means that logical service did not answer or could not be
checked. It does not mean that the ADS device itself was not found, and it does
not hide another port that did respond.

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
- Route direction matters for different computers. In ADS client mode, the
  remote ADS router needs a route back to the truST runtime host. In ADS server
  mode, the external client needs a route to the truST runtime host.
- Do not create a self-route when truST and the target ADS runtime are on the
  same Windows computer. The native Windows ADS client registers with the local
  router and uses the source identity assigned by that router.
- Classic ADS is cleartext and route-based. Keep it on a private OT network and
  do not expose ADS ports through NAT or the public internet.
- On TwinCAT Usermode Runtime, static routes may live under
  `C:\ProgramData\Beckhoff\TwinCAT\3.1\Runtimes\UmRT_Default\3.1\`.
  Use the generated route artifact when possible so the correct Usermode paths
  are handled for you.

The CLI exists for scripting and for the VS Code/web front ends to call. It is
not the primary onboarding interface.

## Discovery Computer And Runtime Host

**Discover ADS devices** in VS Code searches the computer running the extension
and that computer's local network. It does not ask the user to choose among
three target locations.

Discovery and production commissioning are different scopes. If the deployed
truST runtime runs on another machine, that runtime host still needs network
reachability and its own reciprocal ADS route. Run the ADS Doctor from the
runtime host before treating an authoring-time import from VS Code as
production-ready. The `/setup/ads` page remains available on that runtime host
for route planning and deployment checks.

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
- ADS device discovery searches this computer and its local network in one
  operation. LAN identity uses Beckhoff ADS discovery. On Windows,
  same-computer identity and service checks use the installed native ADS router
  API; loopback UDP does not have to answer and truST does not open a competing
  raw TCP connection to the local router.

If automatic identity discovery is blocked by the network, open **Advanced**
and enter the known Host/IP and, when known, its AMS Net ID. Manual identity is
shown as declared until an ADS service actually responds. The Doctor still
proves whether a remote runtime host can reach the device and whether a
reciprocal route exists.

Identity discovery and PLC runtime checks are separate operations:

- The installed native ADS router identifies and connects to same-computer ADS
  services on Windows. It uses the router-assigned source AMS identity and does
  not require a static route from the computer back to itself.
- UDP discovery identifies LAN ADS devices. A known address in **Advanced** is
  an additive directed recovery attempt.
- TCP `48898` carries AMS router traffic.
- ADS `851`–`854`, `301`, `501`, and custom values select logical services
  inside that router; they are not TCP or UDP socket ports.

Devices & Connections keeps an identified ADS device visible even when a
logical service is stopped, Symbol Upload is unavailable, or route setup is
still required. Execution errors are shown as errors and are never converted to
a successful `0 found` result.

## Test Servers And Real Windows Proof

A protocol test server can exercise parts of the cross-platform ADS wire path,
but it is not proof of Windows same-computer behavior. The Windows path must use
the installed native ADS router and a real running ADS service; it deliberately
does not fall back to a raw loopback AMS/TCP client or ask for a self-route.

A stock pyads test server also does not expose a real TwinCAT Symbol Upload
table. It cannot prove symbol import, handle caching, notifications,
online-change symbol versioning, or guarded writes. Use a real ADS device for
those validation steps.

### Accept A Packaged Windows Candidate

This is a release/lab check for a candidate VSIX, not an extra step in normal
ADS onboarding. Run it on the Windows computer that hosts the intended ADS
runtime. Keep the local ADS router and runtime running, and do not add a route
from that computer back to itself.

Use the fetched candidate directory. It contains the exact `win32-x64` VSIX,
its manifest, the API-verified provenance JSON, and the acceptance scripts. Run
one command from that directory:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\accept_windows_twincat_ads.ps1 `
  -CandidateManifestPath .\windows-ads-msvc-candidate.json `
  -ExpectedTargetNetId (Read-Host 'Expected target AMS Net ID') `
  -EvidencePath .\windows-ads-laptop-result.json
```

The default run exercises one unavailable custom port through **Advanced**. If
the lab has another non-standard logical ADS service, pass one to four approved
values with `-CustomAdsPorts`. Normal discovery always checks `851`–`854`,
`301`, and `501`. Do not supply a router source AMS Net ID for normal
acceptance. The prompted AMS Net ID is a fail-closed expected result, not an
address entered into discovery; the packaged UI still starts with one zero-input
**Discover ADS devices** action. The harness observes the native router-assigned
source AMS address; `-ExpectedRouterSourceNetId` is only an optional additional
assertion when the lab already controls that identity.

The harness extracts and executes the binaries from that exact VSIX in an
isolated VS Code profile. It verifies the Simulator lifecycle without opening
Live Values, performs the one-action zero-input ADS discovery, compares the UI
and CLI results for built-in and Advanced ports, browses ADS `851`, imports one
read-only symbol, restarts the Simulator, and verifies that symbol in Live
Values. On same-computer Windows it requires the installed native ADS client,
records the full source and target AMS addresses (Net ID and port), and rejects
raw-loopback or self-route fallback.

A pass also requires the candidate version and hashes to match the manifest,
the relevant TwinCAT static-route files to remain byte-identical, and the final
structured evidence to report `pass`. Screenshots are written beside the
evidence JSON. Treat a missing evidence file, missing screenshot, changed route
file, unavailable ADS `851` symbol table, or failed assertion as a failed
candidate—not as proof that the Windows journey works.

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

Route creation for a remote ADS device is explicit. The setup page, VS Code
panel, and CLI can generate PowerShell, StaticRoutes.xml, and manual GUI
artifacts using the runtime host identity. Automatic route-add sends TwinCAT
credentials directly to the PLC for that one action and never stores them in
`ads.toml`, logs, settings, or generated files. The CLI accepts the password
only through `--password-stdin` and rejects an empty password before sending any
AddRoute packet. Same-computer Windows ADS bypasses this route-creation flow; a
static self-route is neither required nor recommended.

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
| The runtime host can reach one Windows address but ADS replies use another interface or route. | The selected addresses do not share a direct, reciprocal network path. | Put both machines on the same dedicated subnet, use those interface addresses in ADS config, and create the reciprocal route for the runtime host. |
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
  --target 192.168.50.42 \
  --target-net-id 10.20.30.40.1.1 \
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
