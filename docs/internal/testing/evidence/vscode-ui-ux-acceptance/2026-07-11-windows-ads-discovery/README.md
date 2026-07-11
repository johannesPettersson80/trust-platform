# Windows ADS discovery and local simulator acceptance

Status: accepted; `ux_accepted` is proven by the real VS Code journey,
device-in-loop TwinCAT checks, and the required GitHub Windows packaged-VSIX
lane.

## Captured visual evidence

The final candidate was exercised in the real VS Code Extension Development
Host across 11 journey states in Default Dark Modern, Default Light Modern,
and Default High Contrast: 33 captures total. The strict diagnostics at
`json/ads-windows-visual-diagnostics.json` report 33/33 captures, 581/581 DOM,
interaction, accessibility, and paint-integrity assertions passing, zero
failures, and every image accepted on its first capture attempt.

Review artifacts:

- `screenshots/`: 33 full-resolution source captures.
- `json/run-metadata.json`: exact source, bundle, extension-host, candidate
  binary, fixture, and VS Code hashes for the captured run.

The capture harness also generated 33 pixel-equivalent review proxies (`AE=0`
for every source/proxy pair), three complete theme matrices, and eleven
three-theme state comparisons. Those disposable review derivatives were
inspected locally; the full-resolution sources and structured diagnostics are
the retained release evidence.

PNG hygiene passed for all source captures, proxies, and contact sheets. Every
individual proxy and all state/theme comparisons were visually inspected. No
blocking clipping, overflow, missing paint, duplicate ADS journey, ambiguous
port control, or color-only status was found. The typed identity-failure state
clearly directs the user to enter the target AMS Net ID and keeps Advanced
visible; its manual-entry control is below the current viewport, which is a
non-blocking progressive-disclosure observation for a later refinement.

This visual run uses the deterministic ADS UI fixture and is not hardware
proof. The separate real-TwinCAT evidence remained present after the final
visual recapture in `json/real-twincat-service-probe-safety.json` and
`json/real-twincat-logical-services.json`.

`json/run-metadata.json` and `json/ads-windows-visual-diagnostics.json` retain
the controller hashes from the screenshot candidate they actually captured.
The later backend-only fail-closed hardening did not change the webview bundle
or any rendered state. The exact release-candidate controller was rebuilt and
rerun against the live TwinCAT reader; its current source and compiled hashes
are recorded independently in `json/real-twincat-service-probe-safety.json`.

## Root cause analysis

The customer report contained several failures that looked like one problem but
occurred at different layers.

### 1. The Windows simulator could not load the project

The generated `runtime.toml` selected a TCP control endpoint on Windows but did
not include the authentication token required by the runtime parser. The debug
adapter therefore rejected launch with
`runtime.control.auth_token required for tcp endpoint`. VS Code then waited for
the failed debug start and surfaced the secondary timeout as a stale runtime or
debug session, which obscured the actionable configuration error.

The fix gives every new Windows project a fresh file token and migrates only
older tokenless or known-placeholder local-loopback projects before launch.
This includes the reported `some-secret-value`; it is replaced with a random
token rather than treated as production authentication. The separate DAP
control endpoint uses a per-workspace endpoint and random in-memory workspace
token. Launch and attach credentials are redacted from session and DAP logs.

### 2. A discovery execution error became `0 found`

The extension's offline command wrapper discarded child-process stderr and
non-zero exit status. The controller therefore could not distinguish “the ADS
command failed” from “the command completed and found no TwinCAT computers.”
This converted validation, UDP, and runtime errors into a plausible but false
empty result.

The fix preserves command stderr, marks the progress row failed, and keeps the
error visible. Only a successful command with an empty candidate list may show
the genuine nothing-found guidance.

### 3. The product mixed three different meanings of “port”

The old UI exposed separate broadcast and known-address ADS choices without
explaining their relationship. Its generic Host field suggested `host:port`,
while TwinCAT computer identity actually uses UDP `48899`, AMS router transport
uses TCP `48898`, and PLC runtimes such as ADS `851` and `852` are logical
services inside the router. The user could neither tell which ADS choice to use
nor see which logical PLC runtime was usable.

The fix uses one **Find TwinCAT** journey. It discovers computer identity once,
then offers one confirmed check for PLC runtimes `851`-`854`, Additional task 1
at `301`, NC SAF at `501`, and at most four explicit custom services. It reports
every result separately and never sweeps all 65,535 logical ports.

### 4. Same-computer identity incorrectly depended on loopback UDP

The old same-computer path sent the ordinary UDP Identify request to loopback.
TwinCAT on Windows exposes its local AMS router on TCP `48898`, but its UDP
discovery service does not have to answer a loopback Identify request. A fully
running local TwinCAT installation could therefore be reported as no computers
found even though its router was reachable.

The same-computer path now performs the AMS router open-port handshake first and
uses the router-assigned AMS Net ID as observed identity. UDP remains the
fallback for non-router loopback fixtures. The UI labels this result **On the
discovery computer** and keeps `127.0.0.1` in technical details instead of
presenting loopback as the user's TwinCAT address.

### 5. Same-computer browsing used the wrong ADS source policy

For loopback targets the ADS client used an automatic or explicitly configured
source address. A local TwinCAT AMS router expects the client to request a
router-assigned source registration. Identity discovery could therefore appear
successful while the next browse operation failed on the same computer.

The transport now uses router-assigned `Source::Request` for every loopback
address and keeps the existing explicit/automatic policies for remote targets.
A bounded router probe falls back to a direct source for pyads-style loopback
servers that do not implement the AMS router handshake. Computer identity,
service probing, and transport-source selection have separate owners and
regression tests.

### 6. Unconfirmed service checks interrupted live ADS I/O

The ADS client backend documents that opening a second direct TCP/ADS
connection between the same two hosts can close the first. A real TwinCAT test
confirmed this: while a runtime polled ADS `851`, sequential checks of `851`,
`852`, `853`, `854`, `301`, and `501` moved the live connection from healthy to
reconnecting for about three seconds before it recovered.

Identity discovery remains safe because it uses UDP or, for same-computer
TwinCAT, a local AMS-router identity handshake that does not open a PLC service.
The product now requires fresh confirmation that other software on the named
discovery computer is not
reading TwinCAT before every service check or recheck, while explicitly telling
the user to leave TwinCAT and the PLC running. Closing or replacing a local
check kills its child process. A selected runtime is queried with `ads.status`
and fails closed when ADS I/O is active, recovering, or cannot be verified.
The post-fix device-in-loop proof used this exact controller source against a
runtime that continuously polled the real TwinCAT target. The proxy observed
one `ads.status` request and zero `comm.browse_symbols` requests; all 180
continuity samples stayed healthy and `last_good_value_ms` continued to advance.
The structured evidence is in
`json/real-twincat-service-probe-safety.json`.

After stopping that reader, the final `0.24.32` runtime binary checked the six
preset logical services against the same TwinCAT computer. ADS `851` returned
166 variables; `852`-`854`, `301`, and `501` each returned the structured
`ads_port_unavailable` result (`0x6`, target port not found). No ADS/TCP
connection remained after the checks. The bounded result is in
`json/real-twincat-logical-services.json`.

## Journey ADS-WIN-01

As a PLC engineer, I can find the TwinCAT computer, select an available PLC
runtime, and browse its variables without knowing ADS transport details or
starting the truST simulator. Advanced users can supply a known address, AMS
Net ID, or custom logical port without weakening the simple path.

### Product promise

The user is not asked to choose between two ADS implementations or understand
the difference between discovery transport and an ADS service. There is one
**Find TwinCAT** journey:

1. Say where TwinCAT is.
2. Find the TwinCAT computer.
3. Confirm that other software on the named discovery computer is not reading TwinCAT.
4. Check its logical ADS services.
5. Select a working runtime and browse variables.

The happy path uses the words **TwinCAT computer**, **PLC runtime**, and
**variables**. `AMS Net ID`, `ADS 851`, UDP `48899`, and TCP `48898` are shown
only as Advanced or technical details.

### Scenario A: TwinCAT is on this Windows computer

Given TwinCAT and VS Code are running on the same Windows computer, when the
user selects **On the discovery computer** and chooses **Find TwinCAT**, then
truST asks the local AMS router for its identity and shows the computer once,
without depending on loopback UDP. The found card explains the temporary-client
safety rule; after the user confirms other
software there is not reading TwinCAT, truST uses the local AMS router to check
the six default services and selects the only usable one. TwinCAT and the PLC
remain running. The user never enters `localhost`,
`127.0.0.1`, an AMS Net ID, or an ADS router port.

### Scenario B: TwinCAT is on the local network

Given one or more TwinCAT computers are on the same private network, when the
user keeps the recommended **On the discovery computer's network** choice, then
every identified computer is shown once. After explicit connection-safety
confirmation, the UI shows the bounded service checks separately, for example:

- **PLC runtime 1** — Available — 166 variables (`ADS 851`)
- **PLC runtime 2** — Not running (`ADS 852`)
- **PLC runtime 3** — Symbol browsing not enabled (`ADS 853`)

If exactly one runtime can browse variables it is selected automatically. If
several can, the user makes one explicit selection before continuing.

### Scenario C: broadcast discovery is blocked

Given UDP discovery is blocked, when the user selects **At known address**, then
the basic form asks only for **Host or IP**. It rejects `host:port` before any
network request and explains that runtime ports are selected separately. If a
directed identify still cannot obtain the AMS identity, the result says what is
missing and reveals **AMS Net ID** in Advanced; it must never report a successful
scan with `0 found` after an execution error. Supplying Host/IP plus AMS Net ID
creates a declared target and proceeds to the same explicit service-check
confirmation; it does not claim UDP discovery succeeded.

### Scenario D: a non-standard ADS service is required

Given the PLC uses a logical ADS service outside the common PLC runtime range,
when the user opens Advanced, then they can add explicit logical ports to the
confirmed `851`–`854`, `301`, and `501` checks. Inputs are deduplicated,
validated in `1..65535`, and capped at four additional services and ten total
checks. The UI never scans all ports and never implies that these are TCP or
UDP socket ports. Changing the service list immediately makes earlier results
read-only and disables **Browse variables** until a fresh confirmed check
finishes. Invalid edited port text also invalidates earlier results, so a
removed custom service can never remain browseable through stale state.

### Scenario E: the computer is found but the route is missing

Given TwinCAT identity discovery succeeds but TwinCAT has no route back to the
computer running discovery, then the TwinCAT computer remains visible and the
PLC runtime area says **Route setup required**. The primary recovery action
continues into the existing generated route instructions. A missing route is
not presented as `0 found`, `Not running`, or an empty symbol table.

### Progressive disclosure contract

| Level | Visible settings |
| --- | --- |
| Default | Discovery runs from, Where is TwinCAT?, Find TwinCAT |
| Computer found | Plain temporary-connection explanation, named-origin safety confirmation, Check services |
| Known address | Host or IP |
| Advanced | AMS Net ID, additional logical ADS ports, technical transport details |

The scan origin answers “which computer performs the network operation?” The
target location answers “where is TwinCAT?” Those two questions must not share
one ambiguous label. **On the discovery computer** is intentionally relative to
the selected origin, so it remains truthful when discovery runs on a connected
remote runtime rather than the VS Code workstation.

Surfaces:

- VS Code **Devices & Connections** Discover pane.
- ADS discovered-target card and logical-port selector.
- truST Simulator start flow on Windows.

Preconditions:

- New Structured Text project scaffold.
- Local truST ADS responder for deterministic discovery proof.
- Reachable TwinCAT target at `192.168.77.11` for device-in-loop proof.

Acceptance checks:

- [x] ADS known-address input is a bare Host/IP field, not `host:port`.
- [x] Discover presents one TwinCAT (ADS) flow, not separate broadcast and known-device choices.
- [x] The one flow offers Same computer, Local network, and Known address modes and shows only the fields that mode needs.
- [x] The scan-origin label explains where discovery executes and is not confused with the TwinCAT target location.
- [x] AMS Net ID and logical ADS port are separate fields with concise protocol help.
- [x] Identity discovery opens no ADS/TCP client; every service check or recheck names its origin, tells the user to leave TwinCAT and the PLC running, and requires fresh confirmation that other software there is not reading TwinCAT.
- [x] After confirmation, users see PLC runtimes 851-854, Additional task 1 at 301, and NC SAF at 501, and can add four explicit services without rescanning identity.
- [x] Changing the logical service plan marks prior results stale, disables their radio controls and Browse variables, resets any earlier confirmation, and requires **Check updated ADS services**.
- [x] A selected runtime with active, recovering, or unverifiable ADS I/O fails closed before service checks.
- [x] Canceling a local service check terminates its child process before the UI reports it canceled.
- [x] Each checked logical port reports Available with symbols, Available without Symbol Upload, Empty, or Not running; the UI never calls these network ports.
- [x] Port checks are bounded to six preset services plus four explicit user choices and never sweep all 65535 logical ports.
- [x] The primary happy-path terms are TwinCAT computer, PLC runtime, and Browse variables; ADS transport details stay in technical details/Advanced.
- [x] If exactly one PLC runtime is available it is selected automatically; multiple available runtimes require one clear selection.
- [x] A host containing an ADS port is rejected inline before a scan starts.
- [x] A supplied Host/IP + AMS Net ID creates a declared candidate when UDP Identify is blocked.
- [x] Backend/CLI discovery failures are visible and never collapse into `0 found`.
- [x] Port `851` browse succeeds against the configured TwinCAT target; unavailable ports are reported separately.
- [x] Same-host loopback ADS transport requests a source port from the local AMS router.
- [x] Same-computer identity comes from the local AMS router when loopback UDP does not answer.
- [x] A direct loopback pyads-style server falls back within a bounded time when it does not answer the router handshake.
- [x] Real TwinCAT safety proof shows unconfirmed checks never start and selected-runtime checks never interrupt live ADS I/O.
- [x] A newly scaffolded Windows project starts the simulator without hand-editing `runtime.toml`.
- [x] Dark, Light+, and Dark High Contrast screenshots cover success, validation, and manual-fallback states.
- [x] Extension tests, packaged-Windows executable proof, remote-builder gates, and TwinCAT device proof pass.

GitHub Windows packaged-VSIX proof:

- Exact candidate commit: `639ad4a48909ff2ca1b7b5e8d083926a589d44c6`.
- CI run: [29159934997](https://github.com/johannesPettersson80/trust-platform/actions/runs/29159934997),
  `Windows VSIX ADS Smoke` job
  [86563165876](https://github.com/johannesPettersson80/trust-platform/actions/runs/29159934997/job/86563165876),
  completed successfully on `windows-latest`.
- Focused Extension Host result: 47 passing, zero failing, including the real
  Windows simulator migration/start journey and fail-closed ADS service-probe
  safety contracts.
- Packaged runtime proof: `trust-runtime 0.24.32`, `win32-x64`; all seven
  phases passed (`runtime_version`, manual port 852, host/port rejection, UDP
  Identify, local-router identity, router-assigned loopback source, and bounded
  direct-loopback fallback).
- Packaged VSIX SHA-256:
  `631ea3746a7ddf99769d4fca7096b0e26f61f00ec325ae62ccd44143f3cd7f57`.
- The staged release and packaged `trust-debug.exe` are byte-identical at
  SHA-256 `31f11a09b3cb8846b6945d30ea6748a8d71e23852ac0042154a78db3a0a4827f`.
- The evidence JSON reports `status=pass`, every phase `pass`, and no errors;
  the uploaded build/test logs contain no known placeholder token or exposed
  control credential.

Evidence:

- Automated extension and runtime regression tests.
- CDP journey diagnostics and screenshots for all three themes.
- Windows packaged-executable loopback responder artifact.
- TwinCAT discovery, Doctor, and symbol-browse JSON.

Implementer: Codex. Visual/evidence reviewer: independent Codex capture lane;
GitHub Windows packaged-VSIX evidence reviewed and accepted from exact
candidate run 29159934997.
