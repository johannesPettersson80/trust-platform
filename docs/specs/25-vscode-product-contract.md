# VS Code Product Contract

Status: normative truST product specification for the shipped VS Code
extension.

This document owns the user-facing shell, Live Values, Devices & Connections,
onboarding, libraries, HMI preview, and visual-editor presentation contracts.
It is a truST product specification, not an IEC 61131-3 semantic rule.
Reviewed choices that resolve earlier product-plan conflicts are recorded in
`docs/PRODUCT_DECISIONS.md`.

The words MUST, MUST NOT, SHOULD, and MAY are normative. Static source-contract
tests establish behavior-lock wiring only. They do not by themselves prove
rendered layout, keyboard operation, accessibility, contrast, a produced VSIX,
or a live runtime journey; those claims require the corresponding rendered,
package, or end-to-end evidence.

## 1. Product shell and command exposure

### 1.1 Supported surfaces

- The user-facing runtime/device graph MUST be named
  `Devices & Connections`; its command title is
  `Open Devices & Connections`. User-facing command titles MUST NOT expose the
  internal name `Network Canvas`.
- The value surface and both active and compatibility HTML titles MUST be
  `Live Values`, not `Structured Text Runtime` or `Runtime Panel`.
- Palette-visible commands MUST use the common truST category and MUST NOT
  embed a `Structured Text:` prefix in the title.
- The namespace refactor MUST be named `Move Structured Text Namespace`. It
  MUST request and apply an LSP workspace edit, create a missing target file
  when needed, and remove an empty file it created if the operation fails.
- The native Testing welcome area MUST explain an empty Structured Text test
  workspace and direct the user to add a `TEST_PROGRAM` or
  `TEST_FUNCTION_BLOCK`.

### 1.2 Hidden and retired commands

- Commands whose intended route is another product surface MAY remain
  registered for internal invocation, but MUST be hidden from the command
  palette. This applies to opening Live Values, debug start/attach/
  configuration/reload, test execution, and raw HMI initialization/refresh.
- The legacy Communication and ADS panel commands MUST be absent from command
  contributions, activation events, menus, and the package manifest as a
  whole. No visible editor-title, view-title, or view-item menu may expose
  `Communication`, `Open Runtime Panel`, or legacy ADS panel commands.
- The retired VS Code 3D-twin surface MUST be absent from extension activation
  events, commands, menus, language-model tools, build inputs, compiled
  panel/tool output, and packaged media.
- Hiding a command MUST NOT remove the registered handler needed by F5,
  sidebar actions, native Testing, or adaptive HMI flows.

## 2. Discover

- `Search` runs one search (specification 11, "One search, device identity,
  and presence") over every source its origin can run. Each source shows
  `searching`, `{n} found`, `timed out`, or `skipped` with its reason, using the
  reason texts below. `Scan from` chooses this computer or a runtime. The
  per-type checkboxes become an optional `Only search` list, which keeps the
  remembered selection.
- Fieldbus scans (EtherCAT, Modbus RTU) are not part of the automatic search.
  Each runs only after the user confirms a scan of that bus, and the
  confirmation names the bus, the adapter or port, and what the scan does.
- Results appear while the search runs, one card per device with every protocol
  found on it (§5.14).
- Discover MUST offer separate Modbus operations for a known host/port and a
  caller-supplied CIDR subnet; they are optional targets of the search under
  `Known address or subnet`.
- A discovered runtime card MUST show one actionable host/port value, omit a
  visible `tcp://` prefix, and wrap long endpoints.
- Other candidate cards MUST show human-readable protocol, discovery-source,
  and confidence labels rather than backend identifiers.
- Discover MUST use the shared inspector, section, field, input, button,
  message, and disabled-control roles.
- Empty results MUST suggest concrete checks for device power, network
  reachability, port/firewall restrictions, and the supplied address or
  subnet.
- Discover copy MUST NOT use `Field devices`, `origin's local subnet`,
  `connect-only`, `Targeted (needs a host/subnet)`, `Runtime-only`, or
  `Discovery needs a runtime that serves it`. It MUST NOT use raw
  editor-hover-widget colors instead of shared product roles.
- Hardware-origin scans, including EtherCAT and GPIO, MUST remain visible but
  disabled with a reason until the selected runtime origin is attached or has
  `connected`, `running`, or `online` health. A disabled scan is neutral, not a
  primary action.
- A selected stopped runtime disables every runtime-origin scan. EtherCAT and
  GPIO use the reason
  `Start or connect a runtime before scanning EtherCAT or GPIO.`; every other
  scan from it uses `Start or connect {runtime} before scanning from it.`, with
  the runtime's name. With `This computer` selected, EtherCAT and GPIO are
  disabled with `Choose a running runtime for EtherCAT or GPIO scans.`
- Adopting a discovered runtime MUST retain its discovered label, refresh the
  fleet topology, and select and focus the adopted runtime node.

## 3. Examples, project scaffolding, and packaged tools

### 3.1 Curated gallery

- The packaged manifest MUST parse and contain: Empty simulator, Conveyor,
  TwinCAT ADS, Raspberry Pi, HMI starter, and PLCopen Motion single axis.
- The gallery MUST always expose search plus independent hardware and category
  filters. Filters may be combined. When no result matches, a visible action
  MUST clear the search and both filters.
- Gallery cards MUST display their hardware requirement. Canonical labels are
  `No hardware`, `Requires TwinCAT`, and `Requires Raspberry Pi`.
- Hardware-required badges use the shared warning role. Category identity uses
  stable internal IDs while displaying human-readable names such as `ADS` and
  `Raspberry Pi`.
- Card descriptions MUST contain no more than 80 characters.
- Normal copy flows MUST use native VS Code folder and name prompts. Only
  acceptance automation may override destination, name, and open-folder
  answers through `TRUST_UX_EXAMPLE_*`.

### 3.2 Runnable scaffold

- Every bundled entry MUST contain `trust-lsp.toml`, `runtime.toml`,
  `io.toml`, `.vscode/launch.json`, `.vscode/settings.json`, and
  `src/Main.st`.
- The launch file MUST contain a `structured-text` launch configuration named
  `truST Simulator`, use request `launch`, and target an existing source below
  `${workspaceFolder}/src/`.
- The settings file MUST set `debug.showInStatusBar` to `never`; the truST
  sidebar is the product's run-target surface.
- Create Project MUST produce that native simulator launch/settings contract,
  targeting `${workspaceFolder}/src/config.st`. The `network_canvas_demo`
  acceptance fixture MUST provide the same native simulator launch target.
- Each example `runtime.toml` MUST explicitly contain `[runtime.control]`,
  `[runtime.retain]`, `[runtime.watchdog]`, and `[runtime.fault]`.
- Each example MUST declare a `CONFIGURATION` and bind a program instance to a
  task using `PROGRAM <instance> WITH <task>`.
- Example fixtures MUST NOT contain populated authentication tokens,
  passwords, secrets, private keys, or API keys.

### 3.3 PLCopen Motion starter

- The motion starter MUST use a project-relative vendored
  `PLCopenMotionSingleAxis` dependency at version `0.1.0`, include that
  library's manifest, demonstrate `MC_MoveAbsolute`, and expose motion proof
  values.
- The starter MUST be portable outside the repository and MUST NOT depend on an
  unnecessary `Globals.st`.

### 3.4 VSIX and acceptance evidence

- Every Unix and Windows release VSIX MUST bundle `trust-lsp`, `trust-debug`,
  `trust-runtime`, and `trust-dev` under `editors/vscode/bin`; Windows artifacts
  use `.exe`. The bundled `trust-runtime` and `trust-dev` MUST come from the
  same target and release build so native Testing cannot fall back to a stale
  workbench binary from `PATH`.
- The acceptance-journey batch MUST remove helper-produced PNG files,
  including diagnostic `runner-output` copies, before validating the retained
  evidence tree.

## 4. Live Values

### 4.1 Ownership, placement, and capability

- Live Values owns runtime values, I/O, globals/memory, controlled Write,
  Force, Release, `Release all forces`, and operation feedback. It MUST NOT
  render Start, Stop, Connect, Disconnect, Local/External target selection,
  embedded compile diagnostics, pause/step controls, or another lifecycle
  selector.
- Opening Live Values from Devices & Connections MUST reuse the active editor
  group. Other launch routes use column two.
- Simulator I/O supports read, Write, Force, and Release. Attached managed or
  remote I/O supports those operations when reported capability and
  authorization allow them; a blanket remote-only disable gate is forbidden.
- Runtime status MUST propagate role-derived write, force, and release
  capabilities to the active session. Controls MUST disable before click with
  a visible reason when denied. A forced row MUST require Release before Write.
- Write, Force, and Release MUST use the attach-safe I/O request path for
  remote sessions.

### 4.2 Force safety and mutation feedback

- A simulator Force is one deliberate action. A connected managed or remote
  target requires an explicit first `Arm force` action; the second action may
  pin the value. The panel MUST explain this policy.
- Arming feedback remains in the sticky header, uses the shared quiet warning
  treatment rather than a filled action color, and remains explicit after a
  release.
- Every forced row MUST display a visible `FORCED` badge. The header MUST expose
  an accessible, pressed-state `Forced (N)` filter that shows only forced rows
  and suppresses empty groups.
- Row actions MUST spell out `Write`, `Force`, and `Release`; `W`, `R`, `F`,
  and `F*` abbreviations are forbidden.
- Program-driven outputs or memory that cannot be written MUST explain that
  Write is disabled and that Force is the override path when Force is
  available.
- When forces exist, `Release all forces` MUST be visible. The webview sends
  the forced addresses and the host processes the listed releases.
- Write, Force, Release, and Release-all feedback MUST remain visible in the
  sticky header. Failure uses error treatment; permission, armed force, and
  active-force state use warning treatment. Only bounded transient startup,
  unavailable, or success feedback may expire automatically.
- Successful row operations use the prefixes `I/O write queued for`,
  `I/O force active at`, and `I/O force released at`. Success feedback expires
  after exactly 5000 milliseconds only if it is still the current message.
  Standing active-force feedback MUST NOT auto-expire or be cleared by an
  ordinary value refresh.
- A mutation captures the preceding scan number and refreshes visible rows
  only after a newer runtime scan is observed.

### 4.3 Table and value presentation

- Every row MUST prefer the backend value type and only then infer from an I/O
  address. `Type` and `State` are visible columns, state uses badges, and source
  provenance is muted context beneath the name rather than another column.
- BOOL rows use a compact TRUE/FALSE chooser for Write and Force. A forced BOOL
  row shows Release, hides the editable value, and explains that release is
  required before Write. Row controls remain aligned.
- The header identifies the selected target, current scan, and DEC/HEX/BIN
  format. The table identifies `Name`, `Value`, `Type`, `State`, and `Actions`.
- Standard target labels are `Simulator`, `Connected runtime`,
  `Runtime at <endpoint>`, and `Local runtime (control socket)`. Scan labels
  use `scan #N`, and accessible context states
  `Rows are from runtime scan #N`.
- BYTE, WORD, and DWORD values use uppercase IEC-style `16#…` and padded
  `2#…` forms in hexadecimal and binary modes.
- Safety verbs MUST not wrap. Row actions are quiet secondary controls, not
  repeated filled-primary buttons, and the action column reserves stable
  width and spacing.
- The shared row grid reserves actions with `minmax(160px, max-content)`, a
  6-pixel column gap, and compact control widths of 46 and 62 pixels.
- Narrow panes retain identifiable columns with horizontal scrolling. Long
  names ellipsize without collapsing adjacent columns, expose the full name
  and address as accessible context, and allow source provenance to wrap.
- The five columns use minimum tracks of 116, 52, 38, 64, and 160 pixels for
  Name, Value, Type, State, and Actions respectively.
- Active and compatibility Live Values surfaces consume shared `--trust-*`
  roles and MUST NOT define or consume a private Live Values token layer.

### 4.4 Session truth

- No-session, stopped, disconnected, terminated, and cancelled-I/O paths MUST
  clear stale values through one unavailable-state route.
- A simulator with no session tells the user to Start. A selected unattached
  remote tells the user to Connect. Raw adapter no-session errors are
  normalized to those product states.
- An attached session is `Connected`; its endpoint comes from the debug
  session. A reachable online target without an attached session is
  `Not connected`, not `Running`.
- The lifecycle pill contains only lifecycle state and MUST NOT append ADS or
  other protocol commentary.
- Debug termination or active-session loss immediately clears stale rows,
  capabilities, operation banners, and Connected state. Termination handling
  MUST NOT immediately restore stale state from an older lifecycle snapshot.
- Structured Text Stop MUST track the relevant ST session even when VS Code has
  no active session, await its termination event, and wait the bounded UI
  settle interval before resolving.
- The contributed and dynamically provided debugger configurations MUST both
  expose `truST Simulator`.
- Live Values prefers the selected runtime's friendly label. Managed Start,
  lifecycle attach, sidebar Connect, and canvas Connect propagate that label;
  raw endpoint text is fallback only.
- Live Values subscribes to the shared lifecycle model for status but MUST NOT
  request another I/O snapshot for every lifecycle event.

## 5. Devices & Connections

### 5.1 Panel, target, and lifecycle

- Panel and HTML titles MUST be `Devices & Connections`; `Network Canvas` is
  permitted only as an internal identifier and MUST NOT appear in the built
  webview or runtime-rendered strings.
- Lifecycle ownership is exact: Simulator and managed-local targets use
  Start/Stop because the extension owns their processes; unowned remote targets
  use Connect/Disconnect because the extension owns only their sessions. Every
  runtime reports in its `status` what `shutdown` does to it, as
  `program_control`: `keeps_control` when it still answers after `shutdown`
  and `restart` starts its program again, as the truST firmware of a chip does
  (specification 34 STATE-08), and `process` when `shutdown` ends its process,
  as it ends trust-runtime on a computer. A runtime that does not report the
  field counts as `process`. This is a lifecycle fact of every runtime, not a
  board flag. A runtime that the extension does not launch and that reports
  `keeps_control` also offers Start/Stop for its program: Start, sent as
  `restart`, while it answers with the state `stopped`, and Stop, sent as
  `shutdown`, while it answers with another state. Start sends the mode
  `cold`, the mode a new trust-runtime process starts with (specification 11);
  Rust chooses it, never the extension. Start needs the Operator role and Stop
  the Admin role, as the runtime's operation registry states (specification
  11); below that role the action stays visible but disabled, and its reason
  names the role needed and the role of the connection. A `process` runtime
  keeps the rules above: one on another computer is never stopped from the
  panel, because nothing in the panel could start it again, and the Simulator
  and managed-local runtimes use their launchers.
  Transitional lifecycle state disables the primary action. An unreachable or
  endpoint-less remote keeps Connect visible but disabled with a recovery
  reason. A managed runtime whose status is unavailable keeps Start visible but
  disabled and MUST NOT be represented as stopped. The project's runtime and a
  managed runtime that have not answered before the panel's gather deadline
  are `Status unavailable` for that round, never unreachable or stopped; another
  configured runtime is drawn again from its next answer.
- An invalid saved selection falls back to Simulator. Remote labels retain the
  endpoint port so same-host targets remain distinguishable. Lifecycle success
  requires the requested terminal state (`running` after Start, `stopped` after
  Stop); `starting`, `stopping`, missing, and unknown states are not success.
- Before React mounts or topology resolves, the webview MUST synchronously
  paint a themed loading shell saying `Loading your devices...` and
  `Reading the project's runtime and connections.` Refresh starts
  asynchronously. Any acceptance-only delay is explicit and capped at ten
  seconds.
- The sidebar and graph share one selected-runtime store. Workspace state is
  primary; workspace-keyed global state and an extension-global-storage file
  are durable fallbacks across real VS Code restart. Re-selecting an existing
  ID repairs missing fallback copies.
- Successful Connect also selects the runtime. `Set as run target` selects
  without connecting: the simulator as `simulator`, a managed runtime by its
  name, and another runtime by its control endpoint; a runtime without a
  control endpoint cannot be selected and says why.
- Managed Start/Stop uses fleet lifecycle. Successful managed Start attaches
  through the shared helper with the reached endpoint and friendly label,
  selects the target, and makes Live Values usable. A static fake local-runtime
  capability flag is forbidden.
- Runtime-node controls retain the same ownership rules. They offer
  `Set as run target` and Settings, offer Logs only when a log backend exists,
  and make credential recovery primary after an authentication failure without
  inventing remote process ownership. Their placement in the inspector is
  §5.12.

### 5.2 Setup tasks and stable panes

- The setup wizard offers Connect existing and local managed runtime in v1,
  and `Install truST on a chip` for every chip of truST's chip files (§5.12).
  SSH install and Docker remain visible but unavailable with reasons, and show
  their details `Install on another computer over SSH.` and
  `Run the runtime as a container.`. Wording is generic to another
  computer/controller and avoids IPC-only jargon.
- Setup slots read `Set up runtime`, `Add connection`, and `Add host`. Add host
  occupies the host body row without overlapping header/setup slots.
- Runtime setup uses shared inspector/header/section/button/help chrome and the
  breadcrumb `Devices & Connections / Runtime setup`.
- Connect-existing uses shared chrome, password-masked optional token input,
  plain `Runtime address` host:port guidance, a Discover recovery hint, and
  `Add runtime`. Host:port normalizes to a control endpoint.
- Connect tokens go to SecretStorage; probes read them from there; settings
  resolve against the active workspace. Legacy plaintext writes and
  result-obscuring global success toasts are forbidden.
- Endpoint edit breadcrumbs read
  `Devices & Connections / Edit <user-facing protocol>`, not raw role badges.
- Async refresh snapshots the current panel before awaiting, aborts when that
  panel was disposed/replaced, and posts only through the stable snapshot.
- Backend health, mode, state, detail, and schema identifiers MUST be
  translated to product labels. `configured_policy` renders `Configured`;
  lifecycle appears in one title-case `State` row. Schema summaries translate
  relevant fields to `Connection file`, `Polling`, and `Enabled`.
- Opening a new drawer or choosing a new protocol clears stale apply,
  validation, and fault results in host and webview.
- EtherCAT Browse remains a channel picker labelled `Add channels`, sends a
  dedicated add-EtherCAT-channels message, and persists `selected_channels`
  through `comm.apply`; it MUST NOT enter ADS tag import.

### 5.3 Canvas and endpoint lifecycle

- React Flow controls and summaries use shared low-prominence chrome.
  Protocol, topology role, disabled state, draft state, and grid lines use
  shared semantic roles. Draft topology has a distinct `DRAFT` marker and
  label knockouts that keep edges out of text.
- The graph reframes after structural topology changes, child endpoint
  appearance, selection/focus changes, drawer-width changes, window focus/
  resize/visibility changes, or detection of off-screen rendered nodes.
- Edit/setup slots hide while a right-side drawer is open. Connect-existing
  and Adopt success return to a clean graph without edit placeholders. The
  webview content MUST NOT repeat the `Devices & Connections` page title that
  is already present in the VS Code tab.
- `+ Add` is a first-class toolbar action and is named by empty runtime
  guidance. Hidden Edit mode is not a prerequisite.
- Endpoint removal is a two-step action with explanatory confirmation and
  Cancel.
- Identical topology or schema refreshes MUST NOT erase an in-progress edit.
  Reset decisions compare stable content signatures.
- Endpoint Disable/Enable is shown only when supported by backend schema,
  remains visible in the graph, and writes through offline `comm.apply`.

### 5.4 Add, edit, browse, and recovery

- Add, edit, node-summary, and protocol panes use shared inspector, section,
  field, input, button, and message chrome.
- Picker taxonomy is: `Add device or connection`,
  `Discover devices and runtimes`, `Devices and I/O`,
  `Read tags from another PLC or server`, `Share truST values`,
  `Send and receive messages`, and `Advanced integrations`.
- Schema `json_array` fields use list editors and serialize to arrays. Boolean
  fields use On/Off checkboxes, acronyms retain capitalization, exposed
  globals use product language, and secret help states saved secrets are not
  shown.
- Browse access labels spell out `read/write` and `read-only`.
- Add/write actions remain visible but neutral-disabled with a reason when
  there is no valid selection or a browse/route error blocks the operation.
- ADS route recovery stays inline in Browse, offers `Create route`, and
  explains TwinCAT administrator/manual PowerShell requirements without
  referring to the retired ADS panel. `Create route` creates the route (owner,
  16 October 2026), as TwinCAT's own Add Route does: it asks once for the
  TwinCAT computer's administrator login and sends it only to the guarded
  `ads.route_add` operation of specification 11, which never stores or echoes
  it; then Browse retries. The generated PowerShell route stays available as
  the alternative.
- OPC UA authentication failures offer `Edit credentials` and reopen the form
  with the failed target prefilled.
- ADS and OPC UA browse each receive one configured client connection, not the
  whole connection array.
- ADS tag import works for a stopped project through deterministic offline
  symbol import, refreshes the canvas, and opens generated Structured Text so
  diagnostics can refresh.
- Endpoint summaries use protocol-specific allowlists. ADS allowed clients use
  a humanized summary rather than raw JSON. Notifications use user-facing
  protocol names and correct singular/plural grammar.
- Schema refresh does not reset an active add form unless protocol or prefill
  changes. Successful Test feedback does not expose internal lifecycle tokens.
- Successful Save preserves its result and selects/focuses the saved node.
  When the backend omits an ID, topology matching is allowed only when
  unambiguous.
- Active form validation takes precedence in the header issue indicator, with
  concise text and complete recovery help. Filter wording is neutral and
  grammatically correct.

### 5.5 Graph authority, evidence, and identity

- The extension MUST contribute the internal command
  `trust-lsp.networkCanvas.open`; its user-facing title remains
  `Open Devices & Connections`. Devices & Connections owns communication
  setup in-canvas and MUST NOT import, invoke, or direct users to the retired
  Communication panel.
- Wizard-stage progression alone MUST NOT manufacture `running`,
  `connected`, or green state. A runtime becomes running/connected only from
  runtime lifecycle evidence. A field device becomes connected only after the
  runtime reports real I/O values.
- Runtime health MUST roll up raw endpoint health before any display search or
  protocol filter. Host health represents machine reachability and MUST NOT
  inherit a child endpoint's degraded state.
- Search is non-destructive: nonmatching endpoints, their external
  counterparts, and their wires are dimmed for presentation while their raw
  health and warnings remain in the runtime rollup. Protocol filtering MAY
  remove nodes from the visible count, but MUST report hidden endpoint,
  attention, fault, warning, and error counts. The filter hides the
  connections of the protocols it names, the devices on their buses, the links
  that reach a hidden item, the mesh bus without a shown connection, and the
  systems that only hidden links or a hidden mesh bus reach; it never removes
  an issue from the issues list (§5.11).
- Connector state, health, confidence, and point counts MUST survive topology
  projection into endpoint-node data. Product labels include `Ready`,
  `Needs attention`, `OK`, `Degraded`, `Port reachable only`, and
  `Known address`; raw values such as `port_reachable`, `tcp_connect`, and
  `tcp-only` MUST NOT be shown. Signal summaries report good points separately
  and combine degraded and unavailable points as `<N> need attention`, for
  example `1 good, 2 need attention`.
- A reachable local host is headed `This computer`; a reachable remote host is
  headed `Computer <IP>`. The local host is the host whose ID this computer's
  runtimes report (specification 11), else the host of the project's
  configuration, which is read on this computer, else the host that the
  simulator reports. A remote host is headed by its first address that is not
  a loopback address, and `Computer <host name>` when it reports none. The raw
  hostname remains supporting detail. An unreachable configured peer keeps its
  configured label and MUST NOT be relabelled `This computer`.

### 5.6 Graph projection, merge, and fallback

- Fleet projection MUST preserve host, runtime, endpoint, shared-system,
  external-system, and link identities. A runtime MUST NOT also appear as an
  external system, unrelated external systems remain visible, and every
  emitted link endpoint MUST name an emitted node.
- At the display ingress, the extension clones and normalizes every topology
  response without mutating the received payload. The same idempotent
  normalization applies whether the canvas receives one response or merges
  several responses; adding or removing an overlay or peer MUST NOT change the
  identity scheme. Each container is scoped by `(host_id, container_id)`, and
  every runtime is scoped by its lossless owner tuple
  `(host_id, optional container_id, runtime_id, optional instance)`. The
  instance tells apart the runtimes of one host that report the same runtime ID
  (specification 34 ARCH-15): it is the stable runtime ID that the runtime's
  report or probe carries, and without one the control endpoint through which
  the panel reached it; a runtime that no target reached, such as one that only
  the project's configuration or another runtime's discovery reports, has none.
  That tuple scopes the runtime's endpoints and its configured mesh externals.
  Each link endpoint is resolved independently; the normalized link identity
  includes the raw link
  identity and every uniquely resolved endpoint-owner tuple. Shared-system IDs
  remain global, while each `used_by` runtime reference is rewritten to its
  unique owner tuple. Tuple encoding MUST be injective; display sanitization,
  case folding, or a truncated hash cannot establish identity. The normalized
  clone replaces wire IDs only inside the merge/render model; names, addresses,
  state, detail, security, and other product values remain unchanged.
- This normalization applies to supported topology schema versions 2, 3, and
  4, including older payloads with unscoped `external:mesh:<index>` identities
  and links without IDs. Version 2 links receive a lossless scoped identity;
  version 3/4 link IDs are scoped rather than trusted as globally unique.
  Exact endpoint containment on either end of a link identifies that endpoint's
  owning runtime; `source` metadata may corroborate but cannot override
  containment. A global shared or external endpoint that is not contained by a
  runtime retains its global identity unless it is a configured mesh external
  paired with a uniquely owned runtime endpoint. A link whose two endpoint
  references resolve uniquely to different runtimes is valid and retains both
  owner tuples; ambiguity exists only when an individual reference has zero or
  multiple plausible owned targets where an owned target is required.
  Equal container names, resource names, endpoint IDs, link IDs, or
  configuration indexes under different hosts or containers MUST NOT collapse
  nodes, redirect links, duplicate handles, or hide status/detail. The
  complete owner of a runtime is its whole owner tuple, instance included.
  Repeated snapshots from the same complete owner deduplicate; two runtimes of
  one host that share a resource name but differ in stable runtime ID or in
  reached endpoint are two runtimes. If a runtime or endpoint reference has
  more than one possible owner in one response, the extension retains the
  unambiguous host/runtime projection but omits every affected link
  instead of guessing. It also omits an affected configured-mesh external and
  removes an ambiguous runtime reference from shared-system `used_by`; unrelated
  links, external systems, and shared systems remain visible.
- Merging fleet snapshots uses the highest input schema version, unions hosts
  and runtimes by identity, deduplicates links and external systems by
  identity, and unions each shared system's `used_by` set. A host keeps the
  facts of the first snapshot that names it; the project's own runtime and its
  configuration come before the other configured runtimes. Configured and live
  endpoints for the same runtime remain on that runtime: the project's
  configuration describes the project's own runtime when it answers under the
  configured runtime ID, and the simulator while it runs the project.
- Each runtime's connector reports attach to the endpoints of that runtime's
  report with the same protocol; an ADS report of a supervisory server describes
  `ads_server`, and an OPC UA report of a supervisory client describes
  `opcua_client`. A report attaches to at most one endpoint, and a stronger
  match comes first for every endpoint, so a weaker match never takes a report
  that a stronger one claims: the report whose connection name (`display_name`)
  equals the endpoint's name, then the report whose target equals, contains, or
  is contained in the endpoint's address or its `endpoint_url`, `broker`,
  `host`, `listen`, or `interface` parameter, then the first unused report of
  the protocol. A disabled endpoint runs no connection and takes no report in
  any of these steps, so a report goes to an enabled endpoint. A report with an
  unknown or missing state, health, or confidence attaches nothing to the
  endpoint it claims and is shown as an error on that endpoint, or on its
  runtime when it claims none (specification 11, "Connector status and discovery
  truth contract").
- A configured endpoint overlaid on a running simulator retains the running
  runtime state and explains that restart is required; it MUST NOT report the
  runtime as stopped.
- A configured but unreachable peer is synthesized as `unknown`, never green
  or assumed stopped. Authentication failure is an error with distinct
  missing-token and rejected-token recovery text. A reachable peer uses its
  real topology instead of a synthetic node, and a target without an endpoint
  produces no peer node.
- Synthetic authentication-error nodes retain their control endpoint for
  inspector actions. Peer topology failure remains an inline error while the
  local view stays visible.
- A new project without a configured runtime shows one neutral local
  `Simulator` node. A stopped local control socket is a neutral stopped state,
  leaks no socket path, and does not consume the fault channel: a start failure
  of kind `stale_runtime` whose message says that the local simulator or
  runtime is stopped, or that a runtime at a local socket is not reachable, is
  that state. A running local simulator is connected.
- While the project runs in the simulator (the `simulate` run mode), the
  project's configured runtime is the simulator's node until the simulator
  reports itself: it is named `Simulator`, shows the simulator's lifecycle,
  offers its Start and Stop, and carries its session and run-target marks.
- Live local-simulator topology replaces the matching stopped project overlay
  rather than creating a twin. Configured endpoints waiting for restart remain
  visible, duplicate live endpoints are removed, and raw Structured Text
  resource names do not replace the `Simulator` product identity even when
  runtime mode is absent. A configured endpoint duplicates the live endpoint
  that has its protocol and its place in the configuration, which both
  topologies number in configuration order (specification 11, "Offline
  communication operation and fleet-topology truth").
- Managed local runtimes share the existing `This computer` host. Selection is
  projected as one run-target flag. A managed runtime already present in fleet
  topology is not duplicated and retains managed ownership, lifecycle, and
  log controls. A different live managed runtime does not erase the stopped
  project runtime.

### 5.7 Link and counterpart truth

- Dashing means live topology is not yet proven. Only an established
  `connected`, `degraded`, or `error` status proves a point-to-point link and
  renders it solid; every other status, including an unrecognized future
  status, fails closed and renders dashed. A proven link expresses health
  through its distinct status tone and detail.
- A mesh fabric remains draft and dashed unless every peer has an established
  `connected`, `degraded`, or `error` status. Every other peer status,
  including an unrecognized future status, fails closed as unproven. A
  multi-peer fabric shows its shared-bus label; a single-peer fabric suppresses
  the redundant label and centers its endpoint handle.
- External nodes use product protocol names rather than driver identifiers.
  Client links name the remote server (`ADS server`, `OPC UA server`); server
  links name the remote client (`OPC UA client`). Local simulated and loopback
  endpoint titles are `Simulated I/O` and `Loopback I/O`, leaving the I/O role
  to the node band.

### 5.8 Server configuration and evidence

- Offline ADS tag import MUST enable `[runtime.ads]`, set
  `config_path = "ads.toml"`, and use
  `worker_tick_interval_ms = 20`.
- Server summaries answer where the server listens and what it exposes. ADS
  summaries include endpoint, AMS Net ID, ADS port, connected-client count,
  and whether evidence is only self-test or independently verified.
- Live ADS server evidence, including connected-client and verifier fields,
  MUST survive topology, canvas, and rendered endpoint-node projection.
- Re-applying ADS or OPC UA server configuration MUST remove topology-only and
  secret-presence evidence such as `clients_count`, `clients_summary`, and
  `username_set`; it preserves configured exposure and adds reviewed writable
  names only.

### 5.9 Forms and add-picker taxonomy

- Editing a rejected add form hides only its stale apply fault; unrelated
  live faults remain visible.
- A conditional schema field is rendered, required, validated, and serialized
  only while its `visible_when` predicate matches the selected backend. Rust
  declares the predicate with the field; the panel uses it only to show or
  hide the field while the user types, and on submit Rust evaluates it again
  and ignores the values of hidden fields.
- Rust converts each submitted value by its field's schema type. A value that
  does not fit the type, such as text in a number field or a list that is not
  valid JSON, is a field error of that field; it is never sent as text. The
  items of a list that the catalog describes (specification 11) are checked
  the same way: a required property left empty, a value outside a choice's
  options, an item that sets none or more than one of the properties of which
  it must set exactly one, and an entry of a number list that is not a whole
  number of 0 or more are field errors of the list. An empty property that is
  not required is left out of the item. The list editor offers each such
  property beside the one the item starts with, as `Source CIDR` beside
  `Source IP` for an allowed ADS client.
- Runtime discovery is a separate action, not a protocol card. Empty picker
  groups are omitted. Unknown protocols are never dropped and appear last in
  an advanced `Other choices` group.
- The canonical nonempty group order is `Runtimes` (§5.14), `Devices and I/O`,
  `Read tags from another PLC or server`, `Share truST values`,
  `Send and receive messages`, and `Advanced integrations`. Entry titles,
  badges, and purposes stay as listed here; new entries (chips, `Modbus RTU`,
  an EtherCAT device from its ESI file) are added beside them, never in their
  place. Picker and form texts both come from the Rust catalog.
- Within the tested canonical taxonomy, `Devices and I/O` orders Modbus TCP
  before GPIO; client choices belong to `Read tags from another PLC or
  server`, server choices belong to `Share truST values`, MQTT belongs to
  `Send and receive messages`, and Mesh / Zenoh belongs to
  `Advanced integrations`.
- OPC UA server/client cards use `UA OUT`/`UA IN`; ADS server/client cards use
  `ADS OUT`/`ADS IN`. Direction copy explains sharing values versus reading
  tags.
- Advanced entries use the product titles `Mesh / Zenoh`, `OpenOT`,
  `Realtime T0`, and `Runtime cloud`, with badges `MESH`, `OT`, `RT`, and
  `CLOUD`. Their purpose copy respectively explains peer networking, OpenOT
  evidence, deterministic exchange, and federation without backend-review
  language.
- Canvas protocol names spell out `ADS client` and `ADS server`. Equivalent
  benign ADS and OPC UA server endpoints share a non-alarm accent.

### 5.10 Failure and recovery truth

- Runtime start failure preserves the graph, marks the runtime node as error,
  and exposes an inline retry action rather than replacing the surface with a
  failure screen.
- The banner shows the simulator's failed start with `Retry` and `Open logs`;
  else the reports that cannot be read, those of the other runtimes after
  `Peer topology degraded:`; else, while the panel draws the simulator for the
  project, `Simulator is stopped.` with `Start simulator`, or
  `Simulator running.` with `Stop simulator`.
- Failure or malformed output from managed-runtime status discovery is
  `Status unavailable`, never `Stopped`. Sidebar and graph lifecycle actions
  remain disabled until an authoritative status refresh succeeds.
- Start failures classify missing executable, address/port conflict,
  workspace permission, stale runtime/timeout, and other spawn failure as
  `missing_binary`, `port_conflict`, `workspace_permission`,
  `stale_runtime`, and `failed_spawn` respectively.
- An unreachable OPC UA client test reports that the OPC UA server is not
  reachable and MUST NOT expose raw backend status tokens such as
  `BadNotConnected`.

### 5.11 Layout of the redesigned panel

The owner approved this design on 16 October 2026 from a mockup, with the
condition that no information is lost (§5.16): "im happy with it, but you need
tomake sure we dont loose any information".

- The panel keeps its tab title `Devices & Connections`; Live Values chooses
  its column by that title (§4.1).
- A toolbar runs across the top: the project name; search with `Clear search`;
  `Filter`; the issues button; a `Topology`/`List` switch; `Discover`;
  `+ Add`; and `Edit`/`Done`. The `truST` wordmark is not repeated.
- The project name lists the truST projects of the workspace; choosing one
  shows that project. Rust uses the chosen project for the topology, the
  configuration, and the runtimes this computer manages, so they can no longer
  come from different folders. The list holds the projects of every workspace
  folder: the folder itself when it holds a `runtime.toml`, and each direct
  subfolder that holds one; until a project is chosen, or when the chosen one
  is no longer listed, the first is shown. The chosen project's folder is its
  fleet root: its `fleet.toml`, when present, lists the runtimes this computer
  manages, and otherwise the fleet is the project alone; `Add a runtime`
  writes under the chosen project, not under the first workspace folder.
- Search covers computers, runtimes, connections, links, devices, signals, and
  issues, with one dimming rule decided in Rust (§5.5); its placeholder is
  `Search devices, connections, signals, issues`. Ignoring case, an item
  matches when its cells in `List` (§5.14) other than its kind, the texts of
  its issues, or, for a connection or a device, its inspector rows (its
  settings and signals) contain the search; a connection also matches by its
  protocol identifier. A runtime matches when its computer matches, a
  connection when its runtime matches, and a device on a bus when its
  connection matches. A link is lit when it or one of its ends matches; an
  item is lit when it matches or a lit link reaches it; a computer or a
  runtime is lit when something it holds is lit. Every item that is not lit
  is dimmed, and the search reports how many lit items the filter shows.
- The summary strip on the canvas keeps the computer, runtime, and connection
  counts and adds the running, fault, and warning counts. It counts the
  connections that the filter shows, the runtimes whose program runs (also
  with a failing connection), and the faults and warnings of the issues list.
- Right-click on a runtime opens a small menu: `Add device or connection…`,
  `Set as run target`, `Settings`, and `Focus`. Edit slots hide while the
  inspector or another panel is open, as today (§5.3).
- The canvas fills the left. It groups computers by network, draws each
  computer as a frame holding its runtimes, and draws devices, links, the mesh
  bus, the summary strip, the banner, and the zoom controls. Below the network
  a tray lists the boards found on this computer's USB (§5.12).
- A computer belongs to the network band of the IPv4 subnet of its first
  address that is not a loopback address, from the addresses it reports or else
  its runtimes' control endpoints, with the prefix its runtime reports or else
  `/24`; the band is named `Network · <subnet>`, such as
  `Network · 192.168.10.0/24`. A computer without such an address belongs to no
  band.
- One inspector panel, 380 px wide, fills the right while an item is selected.
  It holds every fact and every action of that item. Only one right-hand panel
  is open at a time; opening Add, Discover, Filter, a browse panel, or the
  inspector closes the panel that was open, so panels never cover each other or
  the canvas.
- Colours come from the shared `--trust-*` roles and follow the VS Code dark,
  light, and high-contrast themes. The dark rendering of the approved mockup is
  the reference look. Every status colour comes with a word.
- A fault appears once on its item and once in the issues list. The issues
  button shows the number of faults and warnings and opens a list of all of
  them, grouped by item, each with `Show` and its recovery action; it never
  shows only the first fault. Lifecycle failures also keep their banner with
  `Retry` and `Open logs` (§5.10). A connection or a device on a fieldbus whose
  health is `error` is a fault, and one whose health is `degraded` a warning,
  reading `<name>: <detail>`. A runtime's own failure is a fault: a failed
  start (with `Retry`), a refused or missing token (with `Set auth token`), a
  status that cannot be read, a faulted program, or an error that the runtime
  reports itself; a running runtime whose connection fails raises no issue of
  its own. A report that cannot be read is a fault of the item it names, or of
  the group `General` for no item, which comes first.
- `Show` and `Focus` zoom to the item. Tab reaches every item, Enter opens its
  inspector, and Escape closes the open panel. A click on the empty canvas
  closes the inspector. Dragged positions are kept for the project when the
  panel is closed and opened again.
- The canvas text and every inspector row stay readable at 100 % zoom in a
  1280 × 800 window; long names wrap or end with an ellipsis and show the full
  text on hover.

### 5.12 Runtimes, computers, and chips

Every runtime is drawn and handled the same way, whether it is the Simulator, a
runtime this computer runs, a trust-runtime on another computer, or the truST
firmware of a chip (specification 34 ARCH-11 and ARCH-14). Nothing in the panel
is specific to boards.

- Every host, a computer or a chip, can hold several runtimes (owner, 17
  October 2026: "every host shall be able to have several runtimes"). Its
  frame holds all of them side by side; nothing in the panel assumes one
  runtime per host, and the `Runtimes` group of `+ Add` (§5.14) offers another
  runtime on any host shown (specification 34 ARCH-15).
- A computer frame is headed `This computer` or `Computer <IP>` (§5.5), or the
  chip name and address for a chip, and shows the platform (board, operating
  system and architecture, or chip) and a `Reachable`/`Unreachable` pill. Its
  inspector shows the address, state, runtimes, connections, host name, board,
  operating system and architecture, temperature, up time, and load when the
  runtime reports them. A host is a chip when one of its runtimes reports, as
  the `platform` of its `status`, the name of a chip of truST's chip files
  (specification 34 TARGET-03): the frame is then headed by the chip's model
  and address, such as `STM32F407 192.168.10.31`, its platform reads
  `STM32F407 chip`, and its runtimes have the chip icon.
- A container that holds one runtime is drawn as a chip on that runtime's
  card; only a container with several runtimes is drawn as a frame. Both keep
  their inspector.
- A runtime card shows its name, a kind icon (Simulator, computer, or chip), its
  state with the current cycle time, its link and control address (host and
  port, §5.1), the `Run target` badge when selected, and the pending or `DRAFT`
  look of §5.3. The state is one title-case word of the runtime's lifecycle in
  the tone of its rolled-up health (§5.5): `Running`, `Paused`, `Faulted`,
  `Stopped`, `Ready`, `Starting`, `Stopping`, `Status unavailable`, `Unknown`
  for a runtime that does not answer, or `Authentication failed`; a running
  runtime whose connections report an error or degradation reads `Error` or
  `Degraded`. Only a running runtime is green, and a state that nothing proves
  yet is drawn dashed. A local control socket's path is never shown as its
  address.
  A card without connections shows `No devices yet` with the `+ Add` and
  `Discover` hint, on the simulator and on a runtime that answers and runs; a
  stopped runtime may have connections that it shows once it runs.
- The runtime inspector starts with its actions: the lifecycle action of §5.1
  (a runtime that the extension does not launch offers `Connect`/`Disconnect`
  for the session and, when it reports `keeps_control`, `Start`/`Stop` for its
  program, as the owner's plan of 16 October 2026 states for a chip),
  `Live Values`, `Debug`, and `Deploy` (disabled with the §6.1 text until the
  backend exists). `Live Values` opens the values of the runtime the extension
  is connected to; for another runtime it is disabled and says to start the
  simulator, start the managed runtime, or connect (§4.4). `Debug` launches the
  simulator under the debugger and attaches to another runtime that answers
  with its debugging on; otherwise it is disabled with its reason.
  `Set as run target`, `Settings`, `Logs` (only with a log backend: the
  simulator's debug log and a managed runtime's log), and `Focus` follow as
  smaller buttons. Credential recovery becomes the primary action after an
  authentication failure.
- The runtime inspector then shows: state and detail; run target; mode; cycle
  time and its maximum; link and control address; truST version and the update
  it needs, if any (specification 34 TARGET-05); up time and fault count; the
  access role with the write and force rights; and the connections count. A
  chip adds its clock and its flash and RAM in use. `On this chip` lists the
  components of its firmware from its chip file and greys out a component the
  chip does not hold, naming it (specification 34 ARCH-14). Recent events come
  from the runtime's event log (TARGET-07).
- The USB tray, headed `Found on this computer (USB)`, lists the chips, debug
  probes, and USB-serial adapters that the USB and serial sources find on this
  computer (specification 11, one search); nothing else on USB is listed. A
  boot loader or a chip's USB controller is named as such, such as
  `STM32 boot loader (DFU)`, and lists the supported chips whose chip files
  name its IDs, with the note that the chip is confirmed when connecting and
  before any installation; a boot loader shows `No truST yet`. While only one
  chip file names a controller's IDs, the item is that chip, with its name and
  its flash and RAM from its chip file: the ESP32-C6 shows
  `ESP32-C6, truST not confirmed` until a connection confirms it. A runtime
  shows its truST version, and a probe or adapter shows what it is and its
  serial ports. Without an item the tray reads
  `No chips, debug probes, or USB-serial adapters on this computer's USB.`
  Nothing is written to a board until the user starts an installation.
- Rust decides each tray item's actions. A debug probe offers `Install truST`,
  and so does a chip's boot loader while a chip file names its IDs. A truST
  runtime that the panel does not show offers
  `Connect`, and a chip whose truST is not confirmed offers `Connect` and
  `Install truST`; `Connect` connects at `usb://<serial number>`, and it stays
  disabled, with its reason, for a device without a serial number and while
  this version of truST cannot connect over USB. A runtime that the panel
  shows offers `Show`: the runtime the panel reaches at that `usb://`
  endpoint, or the one whose runtime ID the device carries. The item then
  names that runtime and the network address where it is also reachable: the
  address of its network find (a device of the search with the runtime's ID),
  or the address the panel reaches it at. It offers `Update truST` first when
  the runtime's truST version differs from this tool's (specification 34
  TARGET-05). A USB-serial adapter offers nothing.
- `Install truST` opens the installation in the inspector (specification 34
  TARGET-02, TARGET-04, TARGET-05, TARGET-09), in four steps: the chip found,
  with its identity; installing the release firmware, with progress and the
  checked SHA-256 sum; setting up the device settings of `device.toml` while it
  installs (name, each network interface with DHCP or a fixed address, Wi-Fi
  network and password where the chip has Wi-Fi, Bluetooth where it has it,
  the time server, and the detected crystal and Ethernet PHY); and ready, after
  which the chip appears as a runtime. Rust performs every step; the panel shows
  progress and results.

### 5.13 Connections and devices

- A connection (an endpoint of §5.5) is a pill on its runtime that shows the
  protocol and the connection's own name, a status dot, the role word of today's
  tiles (`CLIENT`, `SERVER`, `MASTER`, `PUB/SUB`, `PEER`, `ADVERTISE`,
  `SAME-HOST`, `POLICY`, `I/O`), and the `DRAFT` and `Disabled` marks. A
  connection is marked `DRAFT` while it is a draft of the add flow, or while an
  advanced integration (Mesh, OpenOT, Realtime, Runtime cloud) has no evidence
  yet (`pending`, `configured_policy`, `not_configured`, or `unknown`), and
  `Disabled` while its health is `disabled`.
- Devices and systems at the other end (external and shared systems, and
  EtherCAT SubDevices) are cards that show their name, what they present (for
  example `MQTT broker`), their status, and, when the runtime reports them, the
  last time they were seen and their response time. They can be dragged. A
  system's status is that of the link that names what it presents, and its
  last time seen and response time are those that the connection at the
  link's other end reports; a SubDevice shows its own health.
- Links keep §5.7: dashed means not yet proven, a proven link is solid with the
  tone of its status, and an arrow shows the direction. A secure link shows a
  lock. Each link is labelled with its protocol, and its status and detail show
  on hover and keyboard focus. The arrow points at the counterpart for
  `outbound` traffic and at the runtime's end for `inbound` traffic; a link of
  another direction, such as `publish_subscribe`, `observed`, or `policy`, has
  none.
- The connection inspector is titled with the connection's name and keeps the
  summaries of §5.4 and §5.8. It adds the connector rows `Connection`,
  `Health`, `Verification`, and `Signals` (§5.5) and, when reported, live
  values, last seen, and response time. Its live values are those its runtime
  reports, and for the simulator's simulated and loopback I/O the first twelve
  I/O values of the simulator. Summaries show a switch as `On` or `Off`, omit a
  field that its `visible_when` hides (§5.9), and show of a secret only whether
  it is set. `Test` stays visible but disabled, with its reason, while the
  connection's runtime does not answer; an OPC UA client with several
  connections offers one `Test` for each, naming it.
- The edit view keeps §5.4 and §5.9, shows advanced fields under an `Advanced`
  heading (not collapsed), and keeps an edit in progress through live refreshes
  (§5.3).
- The runtime discovery settings (enable, service name, advertise, interfaces,
  host group) are edited from the `Runtime discovery` connection of the
  runtime, like any other connection.
- An ADS connection offers `Test` and `Diagnose`; both run the ADS checks of
  the ADS Doctor in Rust (UDP identify, TCP 48898, route, and read state). The
  ADS browser shows the port suggestions (301 `I/O`, 501 `Motion`, 851
  `PLC runtime 1`, 852 `PLC runtime 2`) and its help text under
  `Advanced: browse another ADS port`, and the TwinCAT route help texts in the
  card of a port whose route is missing (§5.4).
- An OPC UA client connection is edited with all its fields, described by the
  Rust schema: endpoint URL, security policy and mode, authentication, user
  name, password, and certificate trust. `Edit credentials` opens this editor.
- The EtherCAT bus keeps all its fields (adapter, timeout, cycle warning, on
  error, modules, mock input frames, selected channels). A drive or terminal
  is added to a configured bus from its ESI file.
- Results of an action started in the panel appear in the pane that started
  it; a failure may also raise a notification (§5.2).

### 5.14 Add, Discover, issues, and the list

- `+ Add` opens one dialog. It adds to the selected runtime, else to the
  Simulator, else to the first runtime shown; opened from a runtime's menu or
  its `Add connection` slot, it adds to that runtime. Its catalog always comes
  from Rust, from the runtime when one serves it and otherwise from
  `trust-runtime comm schema`, so it works before anything runs; a runtime
  whose catalog predates the picker words of specification 11 gets the words
  of the same protocol from the tool's own catalog. `Catalog unavailable` is
  reserved for a missing `trust-runtime`. The groups are §5.9's, preceded by
  `Runtimes`: `Computer`
  with `Connect existing runtime` and `Run a runtime on this computer`
  (`Install over SSH` and `Run in Docker` stay visible as later, §5.2), and one
  entry per chip of truST's chip files, which leads to `Install truST`. Results
  of the last search appear at the top as `Found on the network`, each with its
  protocol, source, and confidence labels (§2), its details (for example the
  AMS Net ID and host), whether the project already uses it, its host and port,
  and `Add`, or `Adopt` for a runtime. `Discover devices and runtimes` and its
  help `Scan first when hardware is already on the network.` stay at the top.
  When a runtime has no connections yet, the dialog also offers the quick
  starts the extension already defines: `Local simulated counter`,
  `Loopback sanity check`, `Modbus drive or meter`, and `MQTT broker`.
- The form column keeps §5.4 and §5.9. `Save` writes to the runtime chosen in
  the form, and `Test` and `Apply to running runtime` act on that runtime.
- `Discover` runs one search (§2). Each device card shows every protocol found
  on it, its identity, its source and confidence labels, and when it was last
  seen, and offers what fits it: `Add`, `Adopt` for a runtime, `Browse tags`,
  `Identify` for a runtime, `Copy address`, and `Ignore`. A card's warnings
  from the search are shown on it. `Compare with project` lists configured
  devices not found, found devices the project does not use, and devices whose
  identity differs.
- The issues list (§5.11) is also reachable from the summary strip.
- `List` shows the same items as the canvas in one table: computers, runtimes,
  connections, and devices, with every field the canvas and inspector show plus
  the live values, last seen, response time, and identity. It is sortable and
  searchable; selecting a row opens the same inspector.

### 5.15 Where the panel's logic lives

The panel only draws. Rust returns, in one request, the whole graph with its
product labels, states, the actions each item allows with the reasons for the
disabled ones, the faults, and the summaries; every action is a Rust command.
The extension keeps no copy of these rules (AGENTS.md, "all logic is in Rust";
the owner's decision of 16 October 2026). This includes today's TypeScript
topology merging and identity (§5.6), health and status rules, the lifecycle
button rules of §5.1, form validation, the discovery and browse rules, and the
extension's own edits of `runtime.toml` and `ads.toml`.

The extension reaches that engine through five language-server commands, which
the language server advertises and the extension never registers:
`trust-lsp.devicesView` returns the whole panel, `trust-lsp.devicesAct`
performs one action a view offered, `trust-lsp.devicesBrowse` opens or
refreshes a browse pane, `trust-lsp.devicesSearch` runs the one search of §2
with the verb `start`, `status` or `cancel`, and `trust-lsp.devicesClose` ends
a panel's session and the searches it started. A session that stays idle for
ten minutes is closed.

Each command waits at most 25 ms for its own work. Slow work continues on a
thread of its own. It holds no lock that another command of the panel needs,
so the other commands answer meanwhile:

- A `devicesView` answers the panel's last projection at once and starts the
  next gather in the background. The first view of a session is the loading
  shell.
- A `devicesBrowse` answers the loading pane until its tree is read.
- A `devicesAct` whose action is not done within 25 ms answers `ok` true,
  `refresh` `soon`, and `pending` with three fields: `operation` (such as
  `op-7`), `pane` (the pane that started the action, §5.13) and `text`. The
  `text` is `Starting…` for a managed Start, `Stopping…` for a managed Stop,
  and `Working…` for any other action.
- A `devicesSearch` with origin `runtime` waits at most 25 ms for that runtime.
  If the runtime has not answered, the answer is the error `runtime_pending`,
  and the panel asks again. An answer that comes later is still kept, so that
  `devicesClose` cancels a search the runtime started.

The final reply of a pending action arrives once, in `operations_done` of a
later `devicesView` reply, with the fields of an on-time reply: `notice`
(absent when the action reported none), `effects`, `close_pane`, `refresh` and
`form` (set only by `form.open`). The view that delivers it applies `refresh`:
`now` makes that reply's `next_refresh_ms` 0 and starts a gather of the new
state as soon as none is running; `soon` caps `next_refresh_ms` at 500 ms;
`none` changes nothing. A result is never delivered twice.

### 5.16 Nothing is lost

Every piece of information and every action of the panel before this redesign
keeps a place in it. The table below names each family and its new place; the
item-by-item mapping of the four inventories of 16 October 2026 (1,403 items,
with file and line evidence) is kept with the release evidence, and a VS Code
test checks each family.

| Before the redesign | In the redesigned panel |
|---|---|
| Panel title, loading texts, the open command, sidebar entry points | Unchanged (§1.1, §5.1) |
| `truST` wordmark in the header | The project name and its project list (§5.11) |
| Search and `Clear search` | Toolbar search over more kinds of items (§5.11) |
| Filter panel: protocol choices, `Show all`, hidden counts and hidden issues | Toolbar `Filter`, unchanged contents |
| Field-issue pill and fault pill (only the first fault's text) | Issues button and a list of every fault (§5.11) |
| `+ Add`, `Discover`, `Edit`/`Done` and the Edit slots | Toolbar and canvas, unchanged (§5.11) |
| Banner: `Start simulator`, `Stop simulator`, `Retry`, `Open logs`, topology errors | Unchanged (§5.10, §5.11) |
| Summary chip with host, runtime and endpoint counts | Summary strip with these and the running, fault and warning counts |
| Zoom, fit, pan, dragged positions | Unchanged; `Focus` zooms to the item; positions persist |
| Host card: headline, reachability, hover card; host inspector | Computer frame and inspector, with the computer's facts (§5.12) |
| Container frame or chip, and its inspector | Unchanged |
| Runtime card: name, state, `Run target`, container chip, empty hint, hover card | Runtime card with cycle time and address (§5.12) |
| Runtime inspector: state, run target, mode, connections, lifecycle and secondary actions | The same, with `Live Values`, `Debug`, `Deploy`, facts, chip, components, events (§5.12) |
| Endpoint tile: protocol, status, role, `DRAFT`, `Disabled`, EtherCAT SubDevices, connector hover card | Connection pill with its own name; SubDevices as device cards; connector rows also in the inspector (§5.13) |
| External system card and inspector | Device card with live information (§5.13) |
| Mesh fabric bus | Unchanged |
| Links: lanes, colours, dashed when unproven, arrows, hover detail | The same, with a protocol label and a lock when secure (§5.13) |
| Endpoint inspector: summaries, server endpoint, clients, verification, fields, `Test`, browse, `Focus` | The same, titled with the connection's name (§5.13) |
| ADS client view | The same, with `Test` and `Diagnose` (§5.13) |
| Edit view: fields, `Save`/`Enable`, `Cancel`, `Test`, `Disable`, `Remove` with its confirmation, `Apply to running runtime` | The same; advanced fields under `Advanced` (§5.13) |
| Add picker: groups, entries, badges, purposes, the advanced toggle | Add dialog with `Runtimes`, found devices and quick starts (§5.9, §5.14) |
| Add form: protocol, purpose, fields, result, `Save`, `Cancel`, `Test`, `Apply to running runtime` | Form column; `Save` writes to the runtime chosen in it (§5.14) |
| `Set up runtime`, `Connect existing runtime`, `Add a runtime`, `Add host` | `Runtimes` group, with `Install truST on a chip` (§5.2, §5.14) |
| Discover: scan types, address or subnet, `Scan from`, ADS ports setting, progress, results and their actions | One search with progress per source (§2) and device cards (§5.14) |
| Browse panes: OPC UA nodes, EtherCAT channels, `Choose globals`, ADS tags per port | Unchanged, with the ADS port suggestions and route help (§5.13) |
| Notifications about actions started in the panel | Results in the pane that started the action (§5.13) |
| Data that reached the extension but was never shown: cycle time, computer facts, live values, last seen, response time, the secure flag, access rights, service states, the quick-start templates | Shown (§5.12 to §5.14) |
| Rules computed in the extension: topology merge and identity, health, button rules, validation, discovery rules, file edits | Rust (§5.15) |

## 6. Sidebar actions, Compile, update, and authentication

### 6.1 Fixed action row

- The fixed row contains Compile, runtime action, Debug, and Deploy. Each
  button is projected from one typed state function.
- The status bar is passive, follows the shared target selection, reveals the
  sidebar, and shows a neutral no-project state. It MUST NOT provide a second
  lifecycle action, and editor-title menus MUST NOT provide Run/Stop lifecycle
  controls.
- Only enabled Start or Connect is filled-primary. Stop and Disconnect are
  neutral-outline. All four use Codicons rather than emoji/text glyphs.
- Compile state uses icon plus semantic tone: clean is check/neutral/outline,
  dirty is warning, and failure is error/danger. Clean diagnostics MUST NOT
  claim `Build OK`, `Build succeeded`, or equivalent.
- At widths at or below 245 px, labels hide and controls retain a compact
  minimum height of 32 px instead of wrapping.
- Deploy remains visible but disabled with
  `Deploy is not available for this target yet.` until a real backend exists.
  No palette deploy command exists during that state, and `Send to PLC` is not
  displayed.
- Live Values MUST NOT retain a compile-diagnostics card or synthetic
  `No compile run yet` state. Details stay hidden until a real result exists.

### 6.2 Compile and project identity

- `Compile` is a fixed sidebar action and palette escape hatch invoking
  `trust-lsp.checkProgram`; it is not nested in a retired Project menu.
- Success is `Compile passed — N source(s), no errors.` Failure is
  `Compile failed — N error(s), M warning(s).` Correct singular/plural and the
  effective diagnostic totals are required.
- The project name is a non-interactive identity row with the open-folder
  Codicon, `Current truST project` help, visible 600-weight name, and theme
  roles. Project switching MUST NOT hide behind that label.
- The truST view title exposes Settings as its sole visible navigation icon;
  `New diagram` remains in overflow.
- Native Settings are titled `truST`; keys use `trust.*`, not `trust-lsp.*`,
  and every title uses product language. Executable-path titles are
  `Runtime executable path`, `Debug adapter path`, and `Test runner path`.

### 6.3 Update running simulation

- Update eligibility requires simulator target, running state, and a real
  saved Structured Text change.
- Update invokes `trust-lsp.debug.reload`, publishes compact success/failure,
  summarizes Problems-oriented compiler failures rather than raw paths, clears
  pending state only after success, and leaves retry visible after failure.
- The language-model reload tool inspects the structured command result and
  reports `Failed to update running simulation:` when `ok=false`; invocation
  alone is not success.

### 6.4 Runtime authentication

- Token reads use the per-endpoint SecretStorage-backed API. A nonblank secure
  value wins; when absent or blank, a trimmed nonblank
  `trust.runtime.authTokenFallback` MAY be used.
- Runtime-target, lifecycle, and Live Values code MUST NOT directly read a raw
  plaintext token setting.
- Managed-runtime token import reads the runtime project's `runtime.toml` and
  imports the value to SecretStorage before attach, never to plaintext
  settings.
- The retired `trust-lsp.runtime.controlAuthToken` key is not contributed.
  The fallback key remains only with UI text that explicitly says legacy,
  fallback, and OS secret store.
- Remote Connect verifies control authentication and `debug_enabled` before
  opening an attach session. Managed token parsing accepts only root
  `runtime.control`, including its root dotted form; an identically named key
  below another table does not supply runtime-control authority.

### 6.5 Lifecycle execution and bounded recovery

- Managed-runtime list/status projection preserves the authoritative
  `running`, `stopped`, `starting`, and `stopping` states. A missing or unknown
  status is `Status unavailable`, never `Stopped`; the sidebar keeps Start
  visible but disabled until a successful status refresh.
- Start completes Compile successfully before launch dispatch. A failed
  Compile prevents launch. Successful simulator Start also requires the
  post-launch I/O acceptance probe; background polling failures after an
  accepted Start do not become persistent Start failures.
- Stop is idempotent when the matching session has already disappeared,
  publishes the fresh lifecycle state only after termination, and managed Stop
  disconnects the matching Live Values session.
- Start and Update requests are bounded. Timeout and failure text is
  human-facing and MUST NOT expose raw local paths, adapter JSON, or debug
  console noise. Managed logs likewise render readable event summaries rather
  than raw JSON records.

## 7. Libraries

- Libraries is reachable from the project sidebar and `Open Libraries`.
- It loads and packages the shared truST theme, uses shared buttons, and MUST
  NOT establish a private token layer.
- OSCAT and PLCopen Motion are curated. Catalog installation remains hidden
  until its backend exists. Empty copy is
  `No libraries added. Add OSCAT, PLCopen Motion, or your own.`
- Failed add remains an alert with `Fix and retry`; success clears stale error.
- Installed/update state comes from the project's vendored copy, preferring its
  package version over stale project information.
- Counts use correct singular/plural grammar.
- The symbol browser supports complete-collection search and pagination,
  declaration detail, copy, and insertion into a visible Structured Text
  `VAR` block.
- Read-only files use `View source`; updates name the target version.

## 8. Shared HMI and visual-editor contract

### 8.1 Shared theme and runtime placement

- SFC, Statechart, Ladder, and Blockly use the truST sidebar for lifecycle and
  Live Values for values. They MUST NOT embed runtime, I/O, runtime-settings,
  or compile-diagnostics panels.
- They retain the shared Structured Text generation, launch, attach, and debug
  command route from `17-visual-editors-runtime-unification.md` sections 1-4.
- Devices & Connections and all four editors consume
  `src/webview/theme.css`; React/inline styles consume shared
  `src/webview/theme.ts` roles. Parallel private theme layers are forbidden.
- Shared protocol roles are `protocolBlue`, `protocolOrange`,
  `protocolGreen`, `protocolCyan`, `protocolRed`, `protocolPurple`, and
  `protocolMuted`. Shared topology roles are `roleHostBg`,
  `roleHostBorder`, `roleRuntimeBg`, `roleRuntimeBorder`, `roleEndpointBg`,
  `roleExternalBg`, and `roleExternalBorder`. The CSS layer MUST expose the
  corresponding `--trust-protocol-*` and `--trust-role-*` tokens.
- Filled primary actions use VS Code button background, hover, and foreground
  roles, never focus/accent as the fill.
- Shared theme defines dark-high-contrast, light-high-contrast, and
  forced-colors behavior for surfaces, inputs, borders, identity, primary
  actions, and focus outlines.
- Default minimap/statistics overlays MUST NOT obscure the program.

### 8.2 HMI preview

- HMI Preview uses shared canvas, surface, text, border, selection, input,
  accent, card, tab, button, status, and empty-state roles.
- With no runtime, it clears stale live presentation and directs the user to
  Start in the truST sidebar.
- Process bindings resolve against the complete HMI schema, not only visible
  widgets. Embedded process SVG is normalized to shared roles.
- BOOL displays as `TRUE`/`FALSE`; REAL/LREAL retains a decimal component.
  Existing lowercase boolean process-map keys remain compatible.
- Descriptor refresh is debounced after relevant text edits, saves, descriptor
  or SVG changes, and view-file watcher events.

### 8.3 Common authoring shell

- All four editors share product header, workspace, canvas, inspector, section,
  button, and input treatment. Inspectors name their actual surface.
- Right-pane order is `Tools`, `Edit`, `View`, with one Fit View placement.
- Preview ST does not write; Generate ST writes or offers to save the companion.
- Invalid SFC, Statechart, and Blockly models offer `Open as text` in VS Code's
  default text editor for the same file.
- Ladder contacts/coils resolve mapped variables and show symbol and address
  separately. Edit strokes are neutral; live power flow owns live wire color.
- The EtherCAT ladder fixture maps `%MX1.0` to the symbol `Step0Active`.
- Dashed treatment denotes missing live topology proof. Devices & Connections
  follows section 5.7; an established degraded or error link remains solid and
  communicates health through semantic status treatment.
- Blockly derives workspace, toolbox, flyout, block, and category presentation
  from shared theme and counts all live workspace blocks.

### 8.4 SFC, Statechart, and visual hygiene

- SFC Add Step, Split, and Join place nodes relative to the graph and reframe
  after mutation commits.
- SFC transitions use an IEC-style perpendicular bar and offset condition
  label. Backward/skip routes use side handles; initial steps are distinct.
- Statechart import, Add State, and Auto Layout reframe after mutation.
  Spacing and side routes leave room for labels.
- Product/semantic colors outside theme modules use shared CSS/React roles;
  private hex/RGB colors are forbidden.
- Network, SFC, and Statechart grids use the shared grid-line role.
- Blockly toolbox labels use normal foreground. Generated-code actions use
  shared buttons and no emoji; layout CSS does not override button chrome.
- Parse failures use user-facing `Could not open…` recovery, not
  `Editor Error`.

## 9. Tooling and example truth

- Extension-test and development binary resolution MUST honor
  `CARGO_TARGET_DIR` before the repository `target` directory. This is a
  developer-workflow contract, not a visual claim.
- OPC UA and ADS server examples expose scan-driven globals: `TankLevel`
  increments by `1.0` during continued ST execution. OPC UA derives
  `PumpRunning` from `TankLevel > 50.0`; ADS derives it from
  `TankLevel > 40.0`.
- When optional local runners are present, they MUST prove a before/after
  change to `TankLevel`, not merely read its static initializer. Their absence
  is not evidence that a runner executed.

## 10. Proof limits and remaining authority gaps

- Source-contract tests do not prove rendered visibility, focus order,
  keyboard accessibility, responsive behavior, or contrast ratios.
- Workflow-copy assertions do not prove produced VSIX contents.
- Selection among multiple configured ADS or OPC UA client connections remains
  unspecified; this batch requires only one configured connection to browse.
- HMI formatting outside BOOL and REAL/LREAL remains outside this batch.
- Positive focus/invocation of shared Run and Live Values from every visual
  editor requires a rendered journey; removal of duplicate panes is not that
  proof.

## 11. Existing authoring workflow contracts

### 11.1 Native Testing and snippets

- Native Testing MUST discover `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK`
  declarations, run all or one selected test through the registered test
  command, and project pass/fail results into the test-controller state.
- Refresh MUST remove results for declarations that disappeared and preserve
  results for declarations that remain.
- The snippet contribution MUST be registered under identifier-friendly
  aliases. Its JSON MUST parse and contain the required declaration patterns.
- A snippet syntax check proves validity only after affirmative parser/LSP
  analysis completes. Observing an initially empty diagnostic collection is
  not proof of valid Structured Text.

### 11.2 OPC UA browse-to-save identity

- A browsed OPC UA leaf's raw `node_id` is its protocol identity. Display
  labels, sanitized identifiers, and tree paths MUST NOT replace or merge that
  identity.
- React/tree keys prefer raw `node_id`, then the source node ID, then the path.
  Leaves with colliding sanitized IDs or equal paths but different raw NodeIds
  remain distinct and preserve browse order.
- A leaf without a raw NodeId cannot be saved. Saved points round-trip the raw
  NodeId and the apply-ready data type; generated variable names are
  deterministic and valid identifiers.
- Write access requires both server-writable evidence and explicit user
  opt-in. Username mode carries credentials; anonymous mode does not.
- Connection assembly requires a usable endpoint and at least one usable
  point. Browse failures use one product recovery action and human-facing text,
  never raw backend status tokens.
- Browse follows the server's continuation points up to the configured depth and
  node limits. When a limit stops the tree, the browse result carries a warning
  naming that limit, and no truncated tree looks complete.

### 11.3 PLCopen import and export workflow

- Import and export cancellation is side-effect free. Missing input, missing
  project, missing runtime binary, malformed XML, and command failure produce
  actionable, operation-specific errors.
- Import into a nonempty target and export over an existing file require
  explicit confirmation before mutation.
- Successful import and export preserve the selected project/file identity and
  complete atomically from the extension user's perspective.
- Ladder interchange accepts schema-v2 nodes, rejects malformed enum values
  without coercion, and reports unsupported constructs.
- A claimed Ladder round trip MUST compare semantic node attributes, operands,
  topology, and connections. Equal node and edge counts alone are not
  sufficient proof.

### 11.4 Visual-model transformations and generated execution

- Ladder edits preserve deterministic IDs, ordering, declaration
  reconciliation, branch topology, and auto-route geometry. Invalid shortcut
  or topology requests fail without partial mutation.
- Blockly lowering preserves connected statement order, control-input slots,
  variable identity, and deterministic inference for supported untyped numeric
  variables.
- SFC topology validation accepts a well-formed parallel split/join and rejects
  missing continuations or fewer than two branches.
- Ladder, Blockly, Statechart, and SFC execution authority is the generated
  Structured Text companion and runtime wrapper. Companion generation
  preserves declared symbols, addressed globals, topology, and event/action
  inputs.
- Retained editor-specific execution engines are non-primary component models.
  They MUST NOT weaken generated Structured Text semantics, including
  divide-by-zero faults.

#### 11.4.1 Retained component-model locks

The following tests may remain as explicitly non-primary component or
historical behavior locks:

- the retained Ladder engine locks deterministic component scan order,
  buffering, topology rejection, supported logical nodes, symbol resolution,
  and its UI timer/counter projection;
- the retired embedded Ladder I/O projection locks deterministic symbolic
  qualification, direct-address canonicalization, pending confirmation, and
  row replacement;
- the retained Statechart component locks awaited hardware actions,
  fail-closed invalid or unreadable guards, bounded request-listener cleanup,
  and editor-session cleanup;
- the retired visual runtime controller and panel bridge lock only their
  message schema, mode/status projection, and action routing.

These locks MUST be labeled as component evidence. They do not authorize a
visible embedded runtime panel, establish IEC/runtime semantics, or replace
generated Structured Text execution and rendered extension evidence.

### 11.5 HMI language-model tools and evidence

- HMI language-model tools validate inputs and cancellation before committing
  final artifacts. Cancellation MUST NOT be reported as a completed workflow.
- Candidate generation, intent planning, validation locks, scenario traces,
  viewport snapshots, journey results, and provenance explanations are
  deterministic for identical reviewed inputs.
- Validation writes bounded lock evidence and applies the documented retention
  policy. Trace and preview operations preserve their scenario and viewport
  identity.
- Binding lookup routes through the registered workspace command and validates
  its input shape. Layout snapshots and dry-run patches report conflicts
  without applying them.
- HMI page enumeration excludes scene-view payload TOML that is not an HMI
  page.

### 11.6 Libraries, project creation, persistence, and focused recovery

- Library dependency edits preserve unrelated manifest structure. Path and Git
  dependencies parse deterministically; a Git dependency has exactly one
  reviewed pin selector; update and removal mutate only the selected entry.
- Library code actions are offered only for known OSCAT or PLCopen Motion
  symbols when the corresponding dependency is absent.
- Create Project is cancellation-safe at every prompt, requires confirmation
  before using a nonempty target, produces parseable/buildable ST and TOML, and
  resolves single-root and multi-root workspace targets deterministically.
- Visual right-pane width persistence uses stable per-editor keys, clamps
  configured bounds, prefers VS Code webview state, falls back to local
  storage, and falls back again to the default for invalid values. It governs
  authoring panes only.
- Compile reports actionable missing-runtime and report-version mismatch
  recovery. Runtime source options preserve explicit include globs and apply
  documented defaults only when they are absent.
- Live Values MAY retry a bounded transient-busy mutation. The retry does not
  authorize immediate success before a newer runtime state confirms the
  mutation.
