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

- Discover MUST offer separate Modbus operations for a known host/port and a
  caller-supplied CIDR subnet.
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
  `Start or connect a runtime before scanning EtherCAT or GPIO.`
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
  and `trust-runtime` under `editors/vscode/bin`; Windows artifacts use `.exe`.
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
  use Connect/Disconnect because the extension owns only their sessions.
  Transitional lifecycle state disables the primary action. An unreachable or
  endpoint-less remote keeps Connect visible but disabled with a recovery
  reason. A managed runtime whose status is unavailable keeps Start visible but
  disabled and MUST NOT be represented as stopped.
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
  without connecting.
- Managed Start/Stop uses fleet lifecycle. Successful managed Start attaches
  through the shared helper with the reached endpoint and friendly label,
  selects the target, and makes Live Values usable. A static fake local-runtime
  capability flag is forbidden.
- Runtime-node controls retain the same ownership rules. They offer
  `Set as run target` and Settings, offer Logs only when a log backend exists,
  and make credential recovery primary after an authentication failure without
  inventing remote process ownership. At most two secondary controls are
  visible before the remainder moves behind `More actions`.

### 5.2 Setup tasks and stable panes

- The setup wizard offers Connect existing and local managed runtime in v1.
  SSH install and Docker remain visible but unavailable with reasons. Wording
  is generic to another computer/controller and avoids IPC-only jargon.
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
  referring to the retired ADS panel.
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
  attention, fault, warning, and error counts.
- Connector state, health, confidence, and point counts MUST survive topology
  projection into endpoint-node data. Product labels include `Ready`,
  `Needs attention`, `OK`, `Degraded`, `Port reachable only`, and
  `Known address`; raw values such as `port_reachable`, `tcp_connect`, and
  `tcp-only` MUST NOT be shown. Signal summaries report good points separately
  and combine degraded and unavailable points as `<N> need attention`, for
  example `1 good, 2 need attention`.
- A reachable local host is headed `This computer`; a reachable remote host is
  headed `Computer <IP>`. The raw hostname remains supporting detail. An
  unreachable configured peer keeps its configured label and MUST NOT be
  relabelled `This computer`.

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
  `(host_id, optional container_id, runtime_id)`. That tuple scopes the
  runtime's endpoints and its configured mesh externals. Each link endpoint is
  resolved independently; the normalized link identity includes the raw link
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
  nodes, redirect links, duplicate handles, or hide status/detail. Repeated
  snapshots from the same complete owner deduplicate. If a runtime or endpoint
  reference has more than one possible owner in one response, the extension
  retains the unambiguous host/runtime projection but omits every affected link
  instead of guessing. It also omits an affected configured-mesh external and
  removes an ambiguous runtime reference from shared-system `used_by`; unrelated
  links, external systems, and shared systems remain visible.
- Merging fleet snapshots uses the highest input schema version, unions hosts
  and runtimes by identity, deduplicates links and external systems by
  identity, and unions each shared system's `used_by` set. Configured and live
  endpoints for the same runtime remain on that runtime.
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
  leaks no socket path, and does not consume the fault channel. A running
  local simulator is connected.
- Live local-simulator topology replaces the matching stopped project overlay
  rather than creating a twin. Configured endpoints waiting for restart remain
  visible, duplicate live endpoints are removed, and raw Structured Text
  resource names do not replace the `Simulator` product identity even when
  runtime mode is absent.
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
  only while its `visible_when` predicate matches the selected backend.
- Runtime discovery is a separate action, not a protocol card. Empty picker
  groups are omitted. Unknown protocols are never dropped and appear last in
  an advanced `Other choices` group.
- The canonical nonempty group order is `Devices and I/O`,
  `Read tags from another PLC or server`, `Share truST values`,
  `Send and receive messages`, and `Advanced integrations`.
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
