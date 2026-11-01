# VS Code Product Contract

Status: normative truST product specification for the shipped VS Code
extension.

This document owns the user-facing shell, Live Values, Devices & Connections,
onboarding, libraries, Control Studio placement, and visual-editor
presentation contracts. The HMI Builder is specified in
`docs/specs/39-hmi-builder.md`.
It is a truST product specification, not an IEC 61131-3 semantic rule.
Reviewed choices that resolve earlier product-plan conflicts are recorded in
`docs/PRODUCT_DECISIONS.md`.

The behavior for creating, configuring, and observing simulated plants inside
Devices & Connections is owned by
[`47-vscode-plant-simulation-ui.md`](47-vscode-plant-simulation-ui.md).
The loop-specific monitoring, experiment, identification, tuning, and
validation behavior of Control Studio is owned by
[`35-vscode-control-studio.md`](35-vscode-control-studio.md).

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
- The loop engineering surface MUST be named `Control Studio`; its command
  title is `Open Control Studio`. It is a full editor surface and MUST NOT be
  embedded in Live Values or the Devices & Connections inspector.
- The truST sidebar MUST expose the project destinations in this order:
  `Devices & Connections`, `Control Studio`, `Live Values`, and
  `Libraries`. Control Studio is a permanent first-class destination, not a
  command-palette-only feature. With no open project it remains visible but
  disabled and explains `Open a project to use this.`
- Activating the sidebar destination with an open project MUST invoke the same
  `Open Control Studio` command used by contextual plant and loop handoffs.
  Contextual `Open in Control Studio` actions MUST remain available because
  they preserve the selected plant or loop identity.
- Palette-visible commands MUST use the common truST category and MUST NOT
  embed a `Structured Text:` prefix in the title.
- The namespace refactor MUST be named `Move Structured Text Namespace`. It
  MUST request and apply an LSP workspace edit, create a missing target file
  when needed, and remove an empty file it created if the operation fails.
- The language server decides whether the cursor's line is a `NAMESPACE` or
  `USING` line (the keyword alone, or followed by a space, after the
  indentation, in any letter case), which dotted name a move starts from (the
  one under the cursor, else the line's first), whether a new namespace path
  can be taken (identifiers joined by dots, none a keyword) and its default
  file (`<last part>.st` in one folder per earlier part below the first
  workspace folder), and it offers the quick fix
  `Move Structured Text Namespace` on such a line (`trust-lsp.namespaceTarget`).
- The native Testing welcome area MUST explain an empty Structured Text test
  workspace and direct the user to add a `TEST_PROGRAM` or
  `TEST_FUNCTION_BLOCK`.

### 1.2 Hidden and retired commands

- Commands whose intended route is another product surface MAY remain
  registered for internal invocation, but MUST be hidden from the command
  palette. This applies to opening Live Values, debug start/attach/
  configuration/reload, and test execution.
- The legacy Communication and ADS panel commands MUST be absent from command
  contributions, activation events, menus, and the package manifest as a
  whole. No visible editor-title, view-title, or view-item menu may expose
  `Communication`, `Open Runtime Panel`, or legacy ADS panel commands.
- The retired VS Code 3D-twin surface MUST be absent from extension activation
  events, commands, menus, language-model tools, build inputs, compiled
  panel/tool output, and packaged media.
- Hiding a command MUST NOT remove the registered handler needed by F5,
  sidebar actions, or native Testing.

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

- The pane's own actions are the view's `actions` (the old pane's
  `networkCanvas/webview/DiscoverPane.tsx` line numbers are given):
  - `Search` (old `Scan`, line 425; this name is the one this section gives the
    action) starts one search with the origin, the ticked sources and the typed
    known address and subnet. While a scan runs it is disabled with the reason
    `A scan is running. Wait for it to end, or cancel it.` (the old pane gave no
    reason) and reads `Scanning…` (line 425). With no ticked source that the
    origin can scan it is disabled with `Select at least one available scan
    type.` (line 421).
  - `Cancel` (new: the old pane had no cancel) shows only while a scan runs, and
    ends it.
  - `Compare with project` (new) lists what the comparison of a finished search
    with the project says, and reads `Hide comparison` while the lists show (the
    rules below).
  - `Configure ADS ports to scan` (line 295) opens VS Code's settings at
    `trust.ads.discoveryPorts` (`networkCanvasPanel.ts` lines 661-665).
  - Search, Cancel and `Compare with project` are the `search` operation
    (`op.kind` `search`), not a `devicesAct` action. The host posts the operation's
    `request` to `trust-lsp.devicesSearch` with the panel's session. Search adds
    the typed targets as `targets`, and both texts are sent as typed:
    - Search: `{"verb": "start", "origin": <origin>, "params": {"only": [<the
      ticked source IDs>]}, "targets": {"hosts": <the Known address text>, "cidr":
      <the Subnet text>}}`.
    - Cancel: `{"verb": "cancel", "origin": <origin>, "search_id": <the running
      search>}`.
    - Compare with project: `{"verb": "status", "origin": <origin>, "search_id":
      <the finished search>, "change": {"kind": "compare", "shown": <bool>}}`.
    - `<origin>` is `"this_host"` or `{"runtime": <the runtime's item ID>}`, which
      is how `trust-lsp.devicesSearch` reads an origin.

    A blank text is no target. `targets.hosts` holds the Known address as one
    entry, and `targets.cidr` holds the subnet; each follows the rule of its
    `comm.search.start` member (specification 11, "Requests").
  - Before the first search, and after the search is cleared, the pane shows the
    idle view. The host asks `trust-lsp.devicesSearch` with the verb `status` and
    no `search_id`, and the answer's `discover` is that view: the origins, every
    scan choice with its reason (the scan rules above), empty `Known address or
    subnet` inputs, the Search action, and no sources or cards. On the first run
    the choice `ADS and TwinCAT` is ticked where the origin can run it, as the old
    pane's first-run default was (`networkCanvas/webview/DiscoverPane.tsx` lines
    136-150); after that the project's remembered ticks apply (the rules below).
    Where the origin cannot run it, Search is disabled with `Select at least one
    available scan type.`
  - A search that fails at a runtime answers its error reply and also the view's
    `error_text`. The texts are the old pane's, word for word:
    - `unknown_runtime` and `runtime_without_endpoint`, when the origin is no
      longer reachable: `The selected runtime is no longer reachable. Start or
      reconnect it, then scan again.` (`networkCanvas/discoveryController.ts`
      line 126).
    - `runtime_unreachable` with a message that names authentication: `The
      selected runtime rejected authentication for truST runtime discovery. Check
      its auth token. Reconnect or start the selected runtime, then scan again.`
      (`networkCanvas/discoveryErrors.ts` line 8).
    - `runtime_unreachable` with a message that names a timeout: `truST runtime
      discovery timed out on the selected runtime. Reconnect or start the selected
      runtime, then scan again.` (`discoveryErrors.ts` line 11).
    - any other `runtime_unreachable`: `The selected runtime could not complete
      truST runtime discovery. Reconnect or start the selected runtime, then scan
      again.` (`discoveryErrors.ts` line 13).

    The old texts name one protocol, and a search has several, so the runtime's
    own discovery carries the name the old pane gave it, `truST runtime`
    (`discoveryErrors.ts` line 21). A failure that the old pane had no text for,
    such as `search_busy`, keeps only its error reply.
- The headings and labels: `Devices & Connections` (the breadcrumb, line 230),
  `Close` (the close button's name, line 233), `What do you want to find?` (line
  262), `Scan from` (line 275), `Known address or subnet` (line 300), `Advanced
  scan settings` (line 271) and `Runtime hardware scans ⚠` (line 306).
- The help texts: `No discovery is available yet. Load a project or connect a
  runtime that advertises discoverable protocols.` (lines 251-252) when no source
  is listed, and `Choose what to find, then scan. Only the selected device types
  are searched.` (line 257) above the scan choices.
- The notes under the source names (lines 33-49): `network broadcast` (ADS),
  `mDNS` (truST runtimes), `local network scan` (Modbus TCP), `at host` (MQTT and
  OPC UA), `bus enumeration` (EtherCAT) and `local pins on the runtime` (GPIO).
  Serial, USB and Bluetooth had no row in the old pane and have no note. The
  choices sit in the groups of the old pane: `What do you want to find?`,
  `Known address or subnet` (MQTT, OPC UA) and `Runtime hardware scans ⚠`
  (EtherCAT, GPIO). The note `discoverable` belongs to a protocol that a runtime
  advertises beyond these ten sources; the view lists it (the rules below).
- A card that is not a truST runtime shows its `Host` (line 379).
- A simulated find carries the mark `Simulated` (new; specification 11,
  "Confidence").
- An empty result is drawn only when a source ran. A search in which every source
  was skipped, such as the search of an older runtime, shows the reason of each
  source and no empty text, so no hardware scan is claimed.
- A status that names a source or a protocol this build does not know leaves that
  part out, leaves out a device whose protocols are all unknown, and shows the
  line `This search reports a source or protocol that this version of truST does
  not know. Update truST to see its results.` (new; the old pane had no such
  case). A find whose confidence this build does not know is left out in the same
  way, and shows the same line.
- The Bluetooth reason is the one that trust-runtime's search catalog gives
  (`trust_runtime::search::BLUETOOTH_UNAVAILABLE`), read from that catalog rather
  than copied.
- The remembered ticks and the Ignore list are per project, in
  `<project>/.trust-runtime/devices.toml` (decision O14), under `[discover]`:
  `protocols` lists the ticked source IDs, and `ignored` lists the device IDs
  that the project ignores. Rust reads and writes the file through one reader,
  which `trust-runtime discover --project` uses too, so the CLI reads the same
  choices (specification 11, "One search"). Without a remembered `protocols`, the
  first run ticks `ADS and TwinCAT` where the origin can run it (old lines 136-150
  and 512-523). A file
  that cannot be read or saved leaves the pane with its text, `The project's
  devices.toml could not be read: {reason}.` or `The project's devices.toml could
  not be saved: {reason}.` (new). A tick or an Ignore with no open project is
  refused with `Open a truST project, so that this choice is remembered.` (new).
- A tick, an Ignore, `Show ignored` and `Compare with project` are each a status request
  of the search with a `change` member, and the answer is the new view: `{"kind": "select",
  "source": <source ID>, "checked": <bool>}`, `{"kind": "ignore", "device":
  <card ID>, "ignored": <bool>}`, `{"kind": "show_ignored", "shown": <bool>}`, and
  `{"kind": "compare", "shown": <bool>}`.
  A tick that cannot run is disabled, drawn unticked and carries no change (old
  `DiscoverPane.tsx` line 191: `checked && !disabledReason`). On a status, a choice
  is ticked when the status names its source; on the idle view the first run ticks
  ADS only. Once the project remembers its ticks, the remembered list decides both.
- Every card offers `Ignore` after its own actions (decision O14; spec §5.14). An
  ignored find leaves the found list until it is undone: with `Show ignored`
  ticked, its card is shown and offers `Stop ignoring` instead. The toggle is
  named `Show ignored`. Decision O14 calls it the toolbar filter; it sits in this
  pane, because the cards it changes are this pane's. The toggle is not
  remembered: it is the panel's own, and every view carries it, unticked until
  the user ticks it.
- The EtherCAT choice shows ⚠ with the title `Can disturb a live bus, so confirm
  before scanning` (old line 204). A Search with EtherCAT ticked asks first:
  `EtherCAT scans the SubDevices on {adapter}. It reads each SubDevice's identity
  in PRE-OP with its process data mapped, and it never brings the bus to SAFE-OP
  or OP. It can disturb a live bus.` (new; the effect is spec 11's source table).
  `{adapter}` is `the adapter {name}` from the origin runtime's EtherCAT
  connection, else `the adapter of the project's EtherCAT driver`. The
  confirmation's buttons are `Search` and `Cancel`. Only that confirmed Search
  posts `confirm_fieldbus_scan: true`, and its `params` keep the `only` list.
  EtherCAT needs the Engineer role (decision O12): from a runtime connection with
  another role the choice is disabled with `Scanning EtherCAT needs the Engineer
  role; this connection has the Viewer role.` (new; the lifecycle's role reasons
  give the same words, `role_needed`).
- An ADS find with no responding port is left out of the cards (old lines
  131-134). When TwinCAT identity was found and no port answered, then at the end of
  the search the ADS source row reads `0 found` (the devices that answered, as
  the old pane counted them, `verified.length`) and carries `TwinCAT identity was
  found, but none of the configured ADS ports ({ports}) responded.` (`discoveryController.ts` lines 237-241). `{ports}` are the
  `trust.ads.discoveryPorts` ports, else `301, 501, 851, 852, 853, 854`
  (`adsDiscoveryPorts.ts`, the default list). Configured ports are sent to the
  search as `ads_ports`.
- An ADS card (old lines 326-373 and 400-408) shows `{n} responding ADS port`
  (singular for one, `ports` for more; old line 345), one button per responding
  port that browses that port's tags of the found device (old lines 358-373 showed
  the ports as chips; they now act), and its primary action: `Add to canvas`
  (old line 407), which adds one ADS connection per responding port to the owning
  runtime, or `Manage tags` (old line 406) once the device is on the canvas. The
  device is on the canvas when a connection of a known runtime has the find's AMS
  Net ID; its mark `On canvas` (old line 356) is the project mark below. Adding is
  disabled with `Add a runtime first, so that the device has a runtime to connect
  through.` when no runtime owns the search.
  A successful `Add to canvas` is followed by the tag browser of the found device,
  as the old pane opened it after the add (old `useDiscoverPane.ts` lines 98-109).
  The view carries that browse as the action's follow-up: the action's `op` is
  `followed`, whose `first` is the `connection.ads_add` action and whose `then` is
  the browse that `Manage tags` opens. The webview runs `then` only when the final
  reply to `first` is `ok` (a `pending` reply is not final, specification 25 §5.13);
  a failed add opens nothing.
- The project mark (specification 11, "Compare with the project"). A card carries the
  project's view of its device: `project`, a word with its tone, and `differs`, one
  row for each fact that differs. The view compares the finds of the status with the
  connections that the canvas holds, those of the runtimes the panel shows that are
  not disabled, by the function that `comm.compare` runs; the panel reads no project
  file of its own. A connection whose settings name no device the way its driver
  reads them (specification 11, "The project's devices") names none. Only a card with
  a result of Modbus TCP, EtherCAT, MQTT, OPC UA or ADS is marked. Every other card,
  every card while the panel shows no runtime, and a result that the comparison cannot
  judge (a connection named by a host name that no result carries; specification 11,
  "Host names") carry no mark. A mark is not an action: a TwinCAT card offers
  `Manage tags` when the canvas holds the device, as above. An ignored find (decision
  O14) keeps its mark. The marks:
  - `In the project`, tone ok: a connection names the device. A TwinCAT card that is
    in the project and does not differ reads `On canvas` (old line 356) instead.
  - `Not in the project`, tone neutral: no connection names the device.
  - `Differs from the project`, tone warning: the device is in the project and its
    identity differs. One row for each differing fact. Its label is the fact's name,
    `Vendor`, `Product code`, `Revision`, `Position`, `Model` or `Security`, and its
    value is `The project has {configured}; the device reports {found}.`, with
    `nothing` for a fact that the device did not report.
- `Compare with project` (specification 11, "Compare with the project"). The pane's own
  action `compare`, after `Search` and `Cancel` and before `Configure ADS ports to scan`.
  It lists what the comparison of the finished search with the project says. It is a
  status request of the search the pane shows with the change `{"kind": "compare",
  "shown": true}`, and the answer is the view with `compare`, the lists, below the cards
  and the empty text. While the lists show, the action reads `Hide comparison` and asks
  for `"shown": false`; otherwise it reads `Compare with project`. The choice is the
  panel's own, as `Show ignored` is: it is not remembered in the project, and a new
  search clears it, because the lists belong to the search they were asked for. The
  panel compares the finds of the status with the connections that the canvas holds, by
  the function that the marks use, so a list and a mark never disagree; the command line
  and `comm.compare` run the same function on the devices of the project's files. The
  action is disabled, with its reason, in this order: `The panel shows no runtime, so
  there is no project to compare with.` (new) while the panel shows no runtime; `Search
  first, then compare the result with the project.` (new) before any search; `Wait for
  the search to end, then compare it with the project.` (new) while the search runs; and
  `A cancelled search has not looked everywhere. Search again to compare it with the
  project.` (new) after a cancelled search, because a device that a search did not look
  for is not a device that is missing (specification 11). The lists have the heading
  `Compared with the project`, then four lists in this order. Each has a heading, rows,
  and a text that shows in place of the rows when it has none. A row is a label and a
  value:

  | List | Heading | Row label | Row value | Without rows |
  |---|---|---|---|---|
  | Missing | `Configured devices not found` | the protocol: `Modbus TCP`, `EtherCAT`, `MQTT`, `OPC UA` or `ADS and TwinCAT` | the device in words: `{address}, unit {unit ID}` for Modbus TCP, `{model} at position {n} on {adapter}` for EtherCAT, `{host}:{port}` for MQTT and OPC UA, `{AMS Net ID}` with ` at {host}` when the connection names a host for ADS | `Every configured device was found.` |
  | Unused | `Found devices the project does not use` | the title of the device's card | its protocols, joined by `, `, followed by ` at {place}`: `position {n} on {adapter}` for an EtherCAT SubDevice, else the device's first address when its title is not that address | `Every found device is in the project.` |
  | Differs | `Devices that differ from the project` | the title of the device's card; one row for each fact that differs | `{Fact}: The project has {configured}; the device reports {found}.`, with the fact named and `nothing` worded as on the card's own rows (the project mark above) | `No device differs from the project.` |
  | Not compared | `Configured devices not compared` | the protocol, as in the first list | the device in words, then the reason in brackets: `its source did not run in this search`, `its host name is not looked up` or `a search cannot probe this unit ID` | `Every configured device was compared.` |

  An ignored find (decision O14) is never in the second list, and its difference is in the
  third even while its card is hidden. The reasons in the last list are the words of
  `trust-runtime discover --compare` (specification 11); the pane and the command line
  take them from one definition.
- A card's disabled reasons (old lines 334-340 and 385-389, `discoverySession.ts`),
  in this order: `Discovery context changed. Scan again before adding this
  result.` when the search was not started by this panel session; the stale-runtime
  text of spec §2 (`The runtime that found this result is no longer available. Scan
  again before adding it.`) when the origin runtime is no longer shown; and `This
  discovery action is no longer available. Refresh the runtime and scan again.`
  when the origin runtime's `comm.schema` does not advertise the find's protocol
  (only for the protocols that `comm.schema` lists with `discover`).
- A protocol that a runtime advertises beyond the ten sources (its `comm.schema`
  lists `discover` for it) is an `Only search` choice under `Known address or
  subnet`, named by its advertised label, with the note `discoverable` (old line 94).
  It is ticked, remembered and disabled by the rules of the other choices, so it is
  enabled wherever a scan from that runtime can run. Search names it in `only` as it
  names the ten sources, but the panel does not send that name to
  `comm.search.start`, which names only the sources of its table (specification 11,
  "One search"). The panel runs it through the origin runtime's `comm.discover` for
  that protocol: once for each Known address, once for the subnet, or once with no
  target when neither is typed (`scope.host` and `scope.cidr`, specification 11,
  `comm.discover`). Its finds join the same search by the rule of specification 11
  ("Devices"): a find whose `params.host` is an address of a device of the search
  joins that device's card, which keeps its ID, and any other find is a card of its
  own, named `dev:ip:{host}` when it has a host. Its source row reads `searching`,
  `{n} found` or `timed out` as the other rows do, and its protocol line on a card
  reads the advertised label. A request that the runtime refuses leaves the row `skipped` with
  the runtime's own error text. A request that cannot reach the runtime leaves the
  row `skipped` with the failure text of this section, with the advertised label in
  place of `truST runtime`: `The selected runtime rejected authentication for {label}
  discovery. Check its auth token. Reconnect or start the selected runtime, then scan
  again.`, `{label} discovery timed out on the selected runtime. Reconnect or start
  the selected runtime, then scan again.`, or `The selected runtime could not complete
  {label} discovery. Reconnect or start the selected runtime, then scan again.`. A
  status that names an unknown source or protocol is still left out, with the update
  line above.

## 3. Examples, project scaffolding, and packaged tools

### 3.1 Curated gallery

- The packaged manifest MUST parse and contain: Empty simulator, Conveyor,
  TwinCAT ADS, Raspberry Pi, and PLCopen Motion single axis.
- A valid top-level manifest MUST retain every valid entry when an individual
  control lesson's `lesson.json` is missing, unreadable, invalid JSON, or fails
  lesson-schema validation. Such an entry remains visible as unavailable with
  a concrete reason and cannot be offered as runnable. A malformed top-level
  manifest remains a gallery load failure; entry-level isolation MUST NOT hide
  catalog corruption.
- The language server decides the gallery (`trust-lsp.examplesCatalog`): it
  reads the packaged `manifest.json` and every control lesson's `lesson.json`,
  checks each lesson's engineering contract (a schema 2 lesson keeps its place
  in the course but is not runnable; schemas 3 and 4 must hold their complete
  contract), and answers each entry's availability with its reason, whether
  the entry can be copied (a starter always, a lesson only while available),
  and which stored lesson ids the learner's progress keeps (every id the
  gallery still lists with a lesson; ids it no longer lists are dropped). The
  gallery shows that answer.
- Copying a control lesson adapts the copy's README through the language
  server (`trust-lsp.copyExample`): a link whose relative target resolves to a
  lesson of the manifest keeps its text and is followed by
  `(to open it, use truST > Start from example and select Lesson N)`, where N
  is that lesson's `sequence`, else the number of a `Lesson N` link text;
  images, anchors, absolute paths, links with a URI scheme and links to folders
  the manifest does not list stay as written.
- The gallery MUST always expose search plus independent hardware and category
  filters. Filters may be combined. When no result matches, a visible action
  MUST clear the search and both filters.
- Hardware and category filters shall be labelled native select controls, not
  a full wall of tag buttons. They shall retain stable values, combine with
  search, expose a visible result count and remain usable at narrow widths.
  Typing shall preserve search focus; changing a filter shall retain focus on
  that filter. The empty-result reset shall clear all criteria and return focus
  to search so keyboard users can immediately try another query.
- Gallery cards MUST distinguish default execution mode from optional physical
  integration. The six bundled non-control starters ship simulated I/O defaults
  and MUST display `Runs in simulator`. ADS and Raspberry Pi additionally show
  `Hardware optional: TwinCAT` and `Hardware optional: Raspberry Pi` respectively;
  the other four show `No hardware needed`. These labels describe the packaged
  default and MUST NOT imply physical commissioning or a connected device.
  Existing hardware filter IDs remain stable; optional integration MUST NOT be
  labelled as a prerequisite for starting the simulated example.
- Actual hardware-required badges use the shared warning role. Category identity uses
  stable internal IDs while displaying human-readable names such as `ADS` and
  `Raspberry Pi`.
- Card descriptions MUST contain no more than 80 characters.
- Lesson details MUST retain the full catalog description and the lesson's
  declared objective and expected observation. Their preview MUST contain
  lesson-specific explanatory paragraphs from canonical lesson metadata, not
  only a generic list of section headings. Compact card descriptions shall
  not replace full detail text. Legacy metadata without a specific expected
  observation must be labelled accordingly, not presented as measured evidence.
- Normal copy flows MUST use native VS Code folder and name prompts. Only
  acceptance automation may override destination, name, and open-folder
  answers through `TRUST_UX_EXAMPLE_*`.
- The name of a copy's folder follows the rule of a new project's name (§3.2),
  checked by the language server (`trust-lsp.copyExample`, action `name`); the
  name box shows its reason.
- The repository's catalog (`examples/catalog.json`), which the extension's
  build copies into its package (`npm run build:examples`), passes a packaging
  check that trust-lsp's tests run in CI and in the release guard: every
  catalog entry has a nonblank `id`, `title`, `description`, `path` and
  `hardware`; its path stays inside the examples folder and names a folder; no
  id or path repeats; a control lesson (an id starting with `control-`, or the
  tag `control`) has a `lesson.json` that holds its contract; and the lessons,
  in catalog order, have the sequences 1, 2, 3 … with every prerequisite an
  earlier lesson. The gallery itself keeps a broken lesson visible as
  unavailable (above); the check keeps it out of a release.

### 3.2 Runnable scaffold

- Project creation MUST establish one exclusive activation landing destination
  before opening the destination folder: creating a starter schedules its
  `src/Main.st` and clears any earlier example README landing. Copying a control
  lesson schedules that lesson's README and clears earlier Main focus. An
  unrelated workspace activation MUST NOT consume this pending destination.
- Pending project landing MUST have one activation dispatcher and one current
  structured intent carrying a unique identity, destination project, landing
  kind and destination file, plus example identity for copied examples. A newer
  intent supersedes an older one. Concurrent activation requests for the same
  intent MUST NOT open it twice. A missing destination shall clear only the
  matching intent and report recovery guidance; an open failure shall retain
  the matching intent for retry. Completing an older request MUST NOT clear a
  newer intent. Existing legacy Main/README keys may be read for migration, but
  newly produced intents shall have one authoritative stored representation.
- The unified dispatcher shall migrate legacy handoffs only when their
  destination belongs to the active workspace. A lesson README's parent is
  its project; a starter `src/Main.st` belongs to the parent of `src`. If both
  legacy destinations match, their relative age cannot be recovered: clear
  those ambiguous pending actions, leave all project files untouched, and
  explain how to reopen the desired source or lesson instead of guessing.
- If opening a newly created project folder fails, retain its files and pending
  landing intent. Report that the project was created, name its exact location,
  and offer an explicit Retry opening action. Dismissing recovery MUST NOT retry,
  recopy, overwrite, delete the project, or report that it opened successfully.
  Retrying opens that same folder without repeating project creation.

- Every bundled entry MUST contain `trust-lsp.toml`, `runtime.toml`,
  `io.toml`, `.vscode/launch.json`, `.vscode/settings.json`, and
  `src/Main.st`.
- Each non-control starter MUST also include a source-owned README that is copied
  by the existing example packager. It shall state the first VS Code action,
  expected observable result, simulation mode, physical-hardware boundary and
  relevant existing truST surface. Hardware support claims require separate
  hardware proof; a simulated starter must not imply hardware commissioning.
- The launch file MUST contain a `structured-text` launch configuration named
  `truST Simulator`, use request `launch`, and target an existing source below
  `${workspaceFolder}/src/`.
- The settings file MUST set `debug.showInStatusBar` to `never`; the truST
  sidebar is the product's run-target surface.
- Create Project MUST produce that native simulator launch/settings contract,
  targeting `${workspaceFolder}/src/config.st`. `trust-runtime new --parent
  <folder> --name <name> [--force] --json` writes the project, and the
  extension only prompts for the folder and the name, opens the folder and
  focuses `src/Main.st`. The name box opens with the suggestion
  `my-st-project` selected and without a message; the extension does not check
  the name, and shows the command's message when the command refuses it. The
  command trims the name and checks that it is one
  plain folder name on every platform, so the project folder is always inside
  the parent folder: not empty, not `.` or `..`, no path separator, no drive
  prefix or other character that Windows refuses in a name (`: * ? " < > |`
  and control characters, so `C:x` is refused), no final dot, and not a device
  name that Windows reserves (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`,
  `LPT1` to `LPT9`, in any case and with or without an extension). It writes
  `src/Main.st` (an empty `PROGRAM
  Main`), `src/config.st` (`CONFIGURATION Config`, `RESOURCE MainRes ON PLC`,
  `TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1)`, `PROGRAM MainInstance
  WITH MainTask : Main`), `trust-lsp.toml` (`include_paths = ["src"]`),
  `runtime.toml` (resource `MainRes`, the resource of `src/config.st`, which a
  build requires, specification 11 §6.8.1d; 10 ms cycle; the web server on for
  this computer at `127.0.0.1:8080`, so a deployed HMI opens at once; discovery,
  mesh and observability disabled and the mesh without a token; RETAIN values
  kept in `retain.bin`; watchdog disabled; fault policy `halt`; and
  `[runtime.control]` without an endpoint, specification 11 §6.8.0a), `io.toml`
  (the `simulated` driver), `.vscode/launch.json`,
  `.vscode/settings.json` and a `.gitignore` that lists `/.trust-runtime/`.
  With `--force` the other files are overwritten, but an existing
  `.gitignore` is kept and only gains the `/.trust-runtime/` line when no line
  ignores that folder yet.
  It prints `{"project","focus"}`, or `{"error":{"code","message"}}` with a
  non-zero exit, where `code` is `name`, `exists` (an existing non-empty
  folder without `--force`; the extension asks before running again with
  `--force`), `not-directory` or `io`. The `network_canvas_demo`
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

### 3.5 Example READMEs

Every example teaches one thing, step by step (owner, 30 October 2026). The
`README.md` of an example has these sections, in this order:

1. The title, then one or two sentences: what you do and what it teaches.
2. `## What you learn`: two to four points.
3. `## What you need`: truST in VS Code, any hardware, and how long it takes.
4. `## Step by step`: numbered steps. Each step says what to do, with the exact
   name of the command, button or field, and what you see, with the values of
   a real run of the packaged extension.
5. `## What happened`: the idea behind the result, as a definition, then in
   words, then its formula with every symbol named where it has one, then
   worked with the numbers of the run.
6. `## Try it yourself`: changes to try, each with what to expect.
7. `## Reference`: the files, the signals and their addresses, and how to adapt
   the example. A control example also states what the control platform
   purpose §3 requires: the plant assumptions, the applicability limits and
   the validation needed before use on another plant.

The control course lessons follow their own template
(`docs/specs/control-course-lesson-template.md`). A folder README that only
lists the examples beneath it is an index: its title, one or two sentences,
and a table of its examples with what each teaches.

A walkthrough's values come from a run of the packaged extension by a journey
of the acceptance harness, so a README cannot drift from the product. The check
`scripts/check_example_readmes.py`, which CI runs, requires the sections in
this order in every README under `examples/` except the catalog
(`examples/README.md`), the course, the archive and the listed indexes. Its
pending list names the READMEs not yet converted and may only shrink: a listed
README that passes fails the check until it is taken off the list.

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
- What a row may do follows its section and its state: `live`, `forced` (a
  forced row is released, never written or forced again) or `readOnly` (an ADS
  tag added without writes is neither written nor forced). Each operation that
  is not allowed carries its reason, the row's state first, then its section,
  then the session's access. When the runtime reports no access, everything is
  allowed in simulate mode and nothing in online mode, with
  `Write/force permissions are unknown — reconnect with an engineer token.`
  while a session is attached or running, else
  `Connect with an engineer token to write or force.` (§6.9).
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

- The debug adapter writes each entry as the table shows it: `TRUE` or `FALSE`
  for a truth value, the decimal digits of a number of any integer type
  (`USINT` included), the value type in capitals; an entry without an address
  or a value is left out, and the globals that an ADS tag binds form their own
  group (trust-debug `adapter/io_values.rs`).
- The value type is the runtime's; when the runtime names none, the debug
  adapter names the type that the entry's direct address names by its size
  (IEC 61131-3 §6.5.5, Table 16: `X` or no size prefix `BOOL`, `B` `BYTE`, `W`
  `WORD`, `D` `DWORD`, `L` `LWORD`). The table shows that type and infers none.
  `Type` and `State` are visible columns, state uses badges, and source
  provenance is muted context beneath the name rather than another column.
- BOOL rows use a compact TRUE/FALSE chooser for Write and Force. A forced BOOL
  row shows Release, hides the editable value, and explains that release is
  required before Write. Row controls remain aligned.
- A numeric row's write box starts empty: the operator types the value to write
  or force, as the prepared value of a CODESYS or TwinCAT watch list, and the
  current value stays in the Value column. Write or Force with an empty box
  sends nothing and asks for a value.
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
  `Not connected`, not `Running`. When that runtime answered Connect but keeps
  debugging disabled, Live Values' state is `Debugging disabled` and the status
  bar reads `<runtime> · debugging disabled`, so neither says `connected` while
  the other says it is not.
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
  Transitional lifecycle state disables the primary action. A remote is
  unreachable only when its probe in that refresh failed; an unreachable or
  endpoint-less remote keeps Connect visible but disabled with a recovery
  reason. A remote that no probe asked in that refresh (its endpoint switched
  off, the simulate mode, or a session running) is `Not connected` with Connect
  enabled, since Connect probes it first. A managed runtime whose status is unavailable keeps Start visible but
  disabled and MUST NOT be represented as stopped. The project's runtime and a
  managed runtime that have not answered before the panel's gather deadline
  are `Status unavailable` for that round, never unreachable or stopped; another
  configured runtime is drawn again from its next answer.
- An invalid saved selection falls back to Simulator. Remote labels retain the
  endpoint port so same-host targets remain distinguishable. Lifecycle success
  requires the requested terminal state (`running` after Start, `stopped` after
  Stop); `starting`, `stopping`, missing, and unknown states are not success.
  Stopping a managed runtime ends the extension's attached session when that
  session is online and connected to the stopped runtime, known by its control
  endpoint (surrounding spaces aside) or by its name; a blank endpoint or name
  matches nothing, and a session with another runtime or the simulator stays.
  The language server decides it (`trust-lsp.runTargets` action
  `managedStopEndsSession`).
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
- Connect changes nothing until it succeeds. It probes the runtime's control
  endpoint and requests `status` with the token stored for that endpoint; it
  succeeds when the runtime answers and then either opens an attach session or
  reports debugging disabled. Only a successful Connect points the runtime
  settings at the runtime and selects it. A failed Connect (runtime not
  reachable, missing or rejected token, failed status request, failed attach)
  leaves the previous target, runtime settings and status bar as they were.
- Start in online mode with an empty `runtime.controlEndpoint` connects to the
  workspace project's own runtime, whose endpoint and token `trust-runtime ctl
  --project <dir> endpoint --json` reports at each attempt (specification 11
  §6.8.0a), and uses that token. Its successful Connect leaves
  `runtime.controlEndpoint` empty, so the next attempt asks again and follows a
  runtime that restarted on another port. A runtime that this command reports as
  not running fails the Connect with the command's message.
- The Target picker offers the workspace project's own runtime as `This project's
  runtime` while it runs outside VS Code, for example started from the command
  line: `trust-runtime ctl --project <dir> endpoint --json` reports its endpoint
  (specification 11 §6.8.0a), and it answers there. Selecting it puts the runtime
  settings online with an empty `runtime.controlEndpoint`, so that each connection
  asks for the endpoint again, as Start in online mode does (above), and follows
  the runtime when it restarts on another port. It is a runtime the extension
  connects to: Connect and Disconnect, never Start or Stop. It stays in the list
  while it is selected; when its runtime does not answer, its status says that it
  is not running and Connect is disabled with that reason. It is that runtime's
  only entry: being online with the project's endpoint adds no second one, and a
  Connect to that endpoint, also from Devices & Connections, selects this entry,
  so the sidebar Target, its Target picker and the status bar name it `This
  project's runtime` (the owner, 29 October 2026).
- After a successful Connect the sidebar Target, its Target picker and the
  status bar name that runtime, also when it is not a configured additional
  endpoint. Its node in the graph offers Disconnect and `Set as run target`
  for that endpoint: a runtime does not know the address it was reached at, so
  the graph gives its live node the endpoint that answered. `Set as run target`
  on a remote node without a control endpoint selects nothing and says why; it
  never selects the Simulator.
- A failed Connect keeps the runtime's node in the graph and shows the failure
  on that node. After a missing or rejected token the node's primary control
  stores a token for that endpoint, and Connect stays available on the node as
  the retry.
- The language server names a failed Connect's remedy (`trust-lsp.runTargets`,
  action `connect`): `openDevices` when the runtime accepts no connection or
  its `status` request gets no answer, `setAuthToken` when the runtime refuses
  a missing or rejected token by its code, and none for any other failure. The
  sidebar's notice of the failure offers `Open Devices & Connections` for the
  first, saying that the runtime is not reachable, and `Set auth token` for the
  second. The extension never reads the failure's text to choose.
- A runtime that answers Connect but reports debugging disabled becomes the
  selected target without an attach session. Live Values then says that
  debugging is disabled on that runtime and names `debug_enabled` under
  `[runtime.control]` in its `runtime.toml`; the graph's issue gives the same
  reason and does not direct the user to Devices & Connections.
- Selecting a run target, in the sidebar Target or with `Set as run target`,
  makes it the target that Start, the status bar, Live Values and HMI Live act
  on. A session with another target ends first, as Disconnect or Stop ends it,
  and a failure reported for another target is cleared. The runtime settings
  then name the selected runtime's control endpoint in online mode, or leave
  online mode for a target without one (the Simulator, a stopped managed
  runtime). Selecting opens no session. When the previous session does not end,
  the selection stays as it was and a warning says why.
- Stop or Disconnect while the extension holds no session leaves a runtime it
  is connected to by turning the control endpoint off
  (`runtime.controlEndpointEnabled` false, `Runtime endpoint disabled.`); with
  no runtime connected it changes nothing and succeeds (`Runtime already
  stopped.`). The language server answers which (`trust-lsp.runTargets`, action
  `stop`).
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
  `Add runtime`. Host:port normalizes to a control endpoint; a `tcp://` or
  `unix://` endpoint is kept as typed. Any other text is no control endpoint:
  it is refused with the address help and is not added to the fleet
  (26 October 2026; the canvas used to add such text as an endpoint). An
  endpoint already in the fleet is not added twice. A runtime that the canvas
  creates joins the fleet by the same rule: Rust's `add_host` answers the
  fleet's endpoints with the new runtime's control endpoint.
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
- Picker taxonomy is: `Add device, connection, or simulated plant`,
  `Discover devices and runtimes`, `Devices and I/O`,
  `Read tags from another PLC or server`, `Share truST values`,
  `Send and receive messages`, `Simulation`, and `Advanced integrations`.
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
  unambiguous: the saved connection is the one connection of its protocol in
  the project's configuration whose saved parameters agree with every
  submitted value they hold, else the only connection of that protocol; two
  candidates name none.
- The ADS port typed into a browse pane is read by Rust as the user types: an
  empty field is port 851, and any other text must be a whole number from 1 to
  65535; otherwise the pane shows why and offers no browse.
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
  inherit a child endpoint's degraded state. The roll-up is `error` when the
  runtime or any of its connections reports `error`, else `degraded` when any
  reports `degraded` or `runtime_unreachable`, else the runtime's own state
  when its own word names one (`connected`, `simulate`, `stopped`, `pending`
  or `unknown`), else `connected`, `simulate`, `stopped` (for
  `configured_policy`) or `pending`, the first that a connection reports
  (26 October 2026: the canvas used to let a connected connection outrank the
  runtime's own state; a running simulator stays `Running`).
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
  offers its Start and Stop, and carries its session and run-target marks. A
  Connect to that runtime refused for its token makes it the runtime connected
  to, under its own name: `Set auth token` first, Connect as the retry (no
  Start fixes a refused token).
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
- The add form applies `add` to a protocol that holds several instances and
  can be added to, and `upsert` to any other protocol; an edit applies the
  action it names.
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
- `Test` of an OPC UA client tests its first connection, whose parameters
  `comm.test` reads, from the summary and from the edit form alike
  (26 October 2026: the edit form used to send the whole list of
  connections, which the test could not read).

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
  and address, such as `STM32U5A9 192.168.10.31`, its platform reads
  `STM32U5A9 chip`, and its runtimes have the chip icon.
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
  `Set as run target`, `Settings`, `Identify`, `Logs` (only with a log backend:
  the simulator's debug log and a managed runtime's log), and `Focus` follow as
  smaller buttons. Credential recovery becomes the primary action after an
  authentication failure.
- `Identify` asks the runtime to show where it is (specification 11,
  "Identify"): the panel sends `identify` with `{"duration_s": 10}`, ten
  seconds, over the runtime's control endpoint, with the token the extension
  stored for it, and shows the runtime's answer as the notice of the runtime's
  inspector (§5.13). An accepted request reads `Identifying <runtime>: its
  identify output blinks for 10 seconds.`, with the seconds the runtime reports.
  A refusal shows the runtime's own text as it is, for example `identify is not
  configured: set [runtime.identify] output in runtime.toml` for a runtime
  without the setting and for a chip. The runtime decides every state of the
  resource: running, paused, stopped and faulted all accept it. The button is
  visible for every runtime. It is disabled, with its reason, while the runtime
  did not answer the last round (`Identify is unavailable until the runtime
  answers.`) and while the connection has a role below Operator, the role the
  operation registry names (`Identify needs the Operator role; this connection
  has the Viewer role.`). The right-click menu of §5.11 keeps its four entries.
- The runtime inspector then shows: state and detail; run target; mode; cycle
  time and its maximum; link and control address; truST version and the update
  it needs, if any (specification 34 TARGET-05): `Update this runtime to truST
  <version>` for a runtime older than this tool or whose version cannot be
  compared, `Update truST on this computer to <version>` for a newer one, in
  the warning tone; up time and fault count; the
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
  the runtime's truST version is older than this tool's or cannot be compared
  with it (specification 34 TARGET-05); a runtime newer than this tool is never
  offered this tool's older truST, and its inspector names the update of this
  computer's truST instead. A USB-serial adapter offers nothing.
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
  `Identify` for a runtime, `Copy address`, and `Ignore` (the rules of the TwinCAT
  cards, of `Ignore` and of the remembered ticks are in §2). A card's warnings
  from the search are shown on it. `Compare with project` lists configured
  devices not found, found devices the project does not use, devices whose
  identity differs, and configured devices that were not compared (§2).
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

Today's canvas (`editors/vscode/src/networkCanvas`), which the redesigned panel
replaces, reaches the same rules through two more commands (26 October 2026);
the canvas only gathers the runtimes' answers, draws Rust's answer and shows its
notices:

- `trust-lsp.devicesTopology` takes one round of answers: the open project's
  configuration (`trust-runtime comm topology`), the runtime that serves the
  canvas's live data with its `fleet.topology` and `connectors.status` answers,
  every other configured runtime with its probe result as
  `trust-lsp.runTargets` reports it (`status` and `auth_failure_kind`, which
  Rust reads as the probe of §5.1) and its answers, the
  endpoint of the extension's own simulator while it serves the live data, the
  run mode, the search, and the candidates of the last search; and the
  extension's own facts: the simulator's lifecycle (running, starting, whether
  it reports I/O values), the last start or Connect failure, the catalog
  (`comm.schema`) and `comm.capabilities` of the runtime that serves the live
  data, the managed runtimes with their state, the session (the attached
  endpoint and the selected run target), and the add flow (its step, the
  protocol of the open form, whether a first device was asked for, and the
  form's last result). The canvas asks each time it draws, so it draws the
  latest facts. The answer is the project's own runtime merged with its
  configuration (§5.6), and the graph the canvas draws, by the merge of §5.6:
  the drawn simulator while nothing reports the project's runtime (also beside
  other configured runtimes) with the catalog's local connections, the
  project's configured runtime standing for the simulator in simulate mode, the
  managed runtimes on this computer and the session's marks (§5.1, §6.5); a
  failed Connect on the runtime it was for (§5.1); on the drawn simulator, the
  draft of the open add form and the first device asked for; each computer's
  heading, detail and reachability; each runtime's product name, rolled-up
  health, control endpoint, managed state and whether `Identify` can be asked
  (§5.12); each connection's product name, health, whether it needs attention,
  whether the search dims it, the target its browse pane opens with, and an ADS
  client's summary (§5.13); the links, the shared and external systems; the
  faults, one for a blocking start or Connect failure and one for the form's
  refused result, which belongs to the form and is hidden while the user edits
  it (§5.9); the banner and the summary. It also answers, for each result of
  the last search, whether a configured ADS connection already serves it, which
  of its tags are imported, and why it can no longer be added (§2).
- `trust-lsp.devicesCanvasAct` performs one action: `comm_apply`, `comm_test`
  and `comm_save` (a form applied or tested at the runtime that serves the live
  data, or saved into the project's files, its values converted by their
  fields' schema types, §5.9; a save answers, as `node_id`, the node that draws
  the saved connection under the runtime it saved to, which the canvas
  selects), `ads_add_device`, `ads_tags_add`, `ads_tags_import` and
  `ads_tag_remove` (§5.4 and §5.8), `ads_port` (§5.4), `expose`,
  `ethercat_channels` and `opcua_add` (§5.8), `add_host` (§5.2), `identify`
  (a runtime node's `Identify`, §5.12, through the node's control endpoint or,
  for the simulator, the simulator's own), `browse_error` (the recovery of a
  failed browse, §5.4 and §11.2), and `discover_label` and `discover_failure`
  (a search's progress label and the text of a search that failed at the
  selected runtime, §2). Its reply says whether the action changed anything,
  the notice to show (its tone, detail, whether it waits for the user, and the
  actions it offers), whether the canvas reads its data again, the generated
  documents to open (§5.4), and the action's own result for the pane that
  asked.
- The canvas's local connections follow §5.7 (a simulated connection is titled
  `Simulated I/O`, also the first device) and §5.5 (the search dims a local
  connection and keeps its health), and an ADS client's summary reads its
  runtime's own state, so a configured client of a running runtime is never
  called stopped (26 October 2026: the canvas used to relabel a local connection
  that the search did not match, draw a start failure twice, and mark the
  simulator for a failed Connect to another runtime).
- The fleet view gives each runtime node its controls, the primary one first:
  the simulator and a managed runtime are started and stopped by their health
  (`Starting…`/`Updating…` while pending, a disabled Start while a managed
  runtime's state is unknown), any other runtime is connected and disconnected
  and never stopped, Connect needs a control endpoint, a Connect refused for a
  missing or rejected token (told by its code) makes `Set auth token` the
  primary control with Connect kept as the retry, every node offers `Set as
  run target` and `Settings`, and `Logs` exists for the simulator and managed
  runtimes. Each runtime node also says whether a search can start from it:
  from a runtime that runs or that the extension is attached to, else
  `Start or connect <name> before scanning from it.` Each result of the last
  search names the runtime a found device is added to (the runtime that found
  it while it is drawn, else the first runtime drawn, none when that runtime is
  gone) and, for a found runtime that announced a control endpoint, the host
  the canvas adds (its label, else its name). The webview lays the controls out
  and shows these answers (26 October 2026).

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
  `No compile run yet` state. It has no compile card at all: Compile is the
  sidebar's action, and its result goes to Problems and the sidebar.

### 6.2 Compile and project identity

- `Compile` is a fixed sidebar action and palette escape hatch invoking
  `trust-lsp.checkProgram`; it is not nested in a retired Project menu.
- Compile checks the project of the nearest folder at or above the active file
  that holds `trust-lsp.toml`; without one, the workspace folder of the active
  file; without an active file, the first workspace folder. The language server
  chooses it (`trust-lsp.projectRoot`).
- A workspace is an open truST project when the language server reads its
  `trust-lsp.toml` as it loads a project (`trust-lsp.manifestCheck`). A
  manifest it cannot read, such as one with an unclosed table header or a
  value of the wrong kind (`include_paths = "src"` under `[project]`), shows
  `Project needs repair`, with the line the server could not read; valid TOML
  such as `[ project ]` or a quoted key is readable.
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
- The debug adapter answers a failed `stReload` with that summary, and the
  extension shows it as it is: `Compile failed — N error(s). Open Problems,
  then try again.` when N lines of the build's error name a source file
  (`<name>.st:` or `<name>.pou:`); else the error's first line that is not
  blank, a line longer than 160 characters cut to its first 157 and `...`;
  `Update did not report a reason.` for an error without such a line.
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
- A runtime's refusal names a credential failure by its code only:
  `missing_auth_token` is a missing token and `invalid_auth_token` a rejected
  one (specification 11 §6.9.6); its text is never read. A token travels on a
  trusted channel to a Unix socket and to a TCP endpoint on this computer
  (`localhost`, a `127.0.0.0/8` address or `[::1]`); any other TCP endpoint is
  an untrusted plain-TCP channel, and a blank, unreadable or serial endpoint
  has none.
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
  than raw JSON records: each record as `[level] event key=value …` with its
  details in the order the runtime wrote them and `ts` left out, any other line
  as it is.

### 6.6 Gates

The language server decides whether Start, Update running simulation and Debug
may run (`trust-lsp.compileGate`). The extension sends the error count of every
file VS Code shows in Problems, the last Compile and the selected target's
status, and shows the answer.

- An error in a Structured Text source (`.st`, `.pou`) blocks a verb with
  `Fix N error(s) to <verb>.`. An error in `runtime.toml` or `trust-lsp.toml`
  (any letter case) blocks it with `Fix runtime.toml to <verb>.`, also beside
  source errors. Errors in other files do not gate.
- Without errors in Problems, a failed Compile blocks the same way, by the kind
  of its error issues: an issue about `runtime.toml` or `trust-lsp.toml`, or
  whose code names the configuration (such as `config.resource`), is a settings
  error, and the count is the report's error count. A Compile that passed, is
  stale after a save or has not run blocks nothing.
- Only Start is gated as the primary action. The sidebar's Update running
  simulation is gated by Problems and the last Compile; the
  `trust-lsp.debug.reload` command by Problems alone.
- Debug is unavailable while its gate blocks it, with the gate's reason as its
  title, and while the target is unreachable or starting
  (`Debug is unavailable until the target is reachable.`).
- The Compile button shows the errors in Problems (`N error(s) — see
  Problems`) before the last Compile's.
- While the language server is not running no gate blocks; Start still
  completes Compile before it launches (§6.5).

### 6.7 Simulation speed

The `Simulation Speed…` chooser offers 1×, 2×, 5×, 10×, 20×, 50× and 100×
and saves the choice in the project's `simulation.toml`
(`[simulation] time_scale`, specification 46 §6.1 item 5), changing only
that line, or adding the key after the `[simulation]` header, or the table
before the first table; the file lies beside `runtime.toml` (the workspace
folder, else its `bundle` folder). The selected simulator takes the speed at
once with `config.set` only while it runs and has a control endpoint; any
other target only saves it. The language server saves the file and names
that request (`trust-lsp.simulationSpeed`).

### 6.8 Simulator control endpoint

A simulator that Start or a launch configuration without `controlEndpoint`
launches serves its control API on an endpoint, and with a token, that the
language server names (`trust-lsp.simulatorControl`): on Linux and macOS the
socket `trust-debug-<h>.sock` in the system temporary folder, where `<h>` is
the first 8 hexadecimal digits of the SHA-256 of the workspace folder's path,
and a new token of 32 bytes from the operating system's secure random source
as unpadded base64url (specification 11 §6.8.0a); on Windows none, and the
debug adapter binds its default endpoint. The extension asks once per
workspace folder in a VS Code session, so the launch and Devices & Connections
(§5.1) use the same endpoint and token. A launch configuration's own
`controlEndpoint` is used as it is. Before Start launches on a TCP endpoint,
the language server tries to connect to it for at most 300 ms; when a program
accepts the connection, Start is refused with `The runtime port is already in
use.`

### 6.9 Run targets and runtime requests

The language server decides what the sidebar, the status bar, Live Values and
the runtime lifecycle show of a runtime; the extension shows its answers.

- Every control request of the extension to a runtime goes through
  `trust-lsp.runtimeRequest`: it reads the endpoint, connects, sends the
  request and decodes the answer as a result, a refusal with its code and the
  result a refusal carries, or a failed transport. A probe connects and sends
  nothing.
- `trust-lsp.runTargets` answers the configured target's status and
  credential channel (`target`); the target list, the selected target with its
  status and its one action, whether the compile gate can block that action
  (an enabled Start, §6.6), the endpoint that selecting a target points the
  runtime settings at, and the runtime the status bar names connected
  (`selection`); the Live Values status with the row operations of §4.1 and
  whether a Force must be armed (`liveValues`); the simulator's speed and its
  words (`speed`); the Connect check of §5.1 (`connect`); the kind of a
  failure from its text (`failure`); the run target that the HMI Builder's
  Live mode and Deploy talk to (`hmiTarget`, specification 39 §11 item 6); and
  the managed runtimes with their states, the outcome of a managed Start or Stop
  and the managed logs (`managed`, `managedOutcome`, `managedLogs`).
- The status bar names a runtime connected while a debug session is attached
  to it, or while the extension is online with a runtime that answers, such as
  one started outside the extension.

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
- The panel's entries (status, version, update offer, symbols with their
  declaring line and insertion snippet) and every edit it makes (vendoring
  OSCAT or PLCopen Motion into `libraries/<folder>`, a local library's
  `[package] version = "0.1.0"` manifest template only after the user agrees,
  the Git pin's selector, the dependency entry) are answered by the language
  server's `trust-lsp.projectLibraries`; Plant is added by the HMI Builder's
  bundled-library install, whose packages, nested dependencies, files and
  manifest entry the language server's `trust-lsp.hmiLibraries` answers. `View source` opens the library's first `.st` or
  `.pou` file in path order and appears only when one exists.

## 8. Shared HMI and visual-editor contract

### 8.1 Shared theme and runtime placement

- SFC, Statechart, Ladder, and Blockly use the truST sidebar for lifecycle and
  Live Values for values. They MUST NOT embed runtime, I/O, runtime-settings,
  or compile-diagnostics panels.
- They retain the shared Structured Text generation, launch, attach, and debug
  command route from `17-visual-editors-runtime-unification.md` sections 1-4.
- Devices & Connections, Control Studio, and all four visual editors consume
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

### 8.2 HMI

The HMI Builder editor and its pages are specified in
`docs/specs/39-hmi-builder.md`.

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
- The language server finds the tests (`trust-lsp.testDiscovery`) by
  specification 22, Test POU discovery: namespace-qualified names and one-based
  declaration lines, from an open editor's text where a file is open; without
  named files, every `*.st`, `*.ST`, `*.pou` and `*.POU` below the workspace
  folders outside `node_modules`, `target` and `.git`. A test runs in the
  nearest folder at or above its file that holds `src` or `sources`, else in
  its workspace folder; a `trust-dev test` result belongs to the test with the
  same file, line, kind and name, the name in any letter case.
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
- PLCopen interchange carries Structured Text bodies. A POU with a Ladder (LD),
  FBD or SFC graphical body is skipped with its diagnostic (`PLCO215` to
  `PLCO217`); the visual editors have no PLCopen interchange of their own.

### 11.4 Visual-model transformations and generated execution

Every rule about a Ladder, Blockly, Statechart or SFC program is the language
server's (crate `trust-visual`). The editors draw the program and handle the
pointer (drag, snap, the wire being drawn); they reach every rule through
these commands and show the answers:

| Command | Answers |
|---|---|
| `trust-lsp.visualCompanion` | the Structured Text companion and runtime entry of a program |
| `trust-lsp.visualSources` | the glob of every visual program, for the save hook and Sync |
| `trust-lsp.visualValidate` | the editor's Validate: the problems of a program |
| `trust-lsp.visualParse`, `trust-lsp.visualFormat` | a file as the editor's model, and the model as file text |
| `trust-lsp.visualEdit` | an edit the editor stores in the file |
| `trust-lsp.visualNew` | the name rule and the file of a new program |
| `trust-lsp.visualImport` | the shape check and the file name of an imported copy |

- Ladder edits preserve deterministic IDs, ordering, declaration
  reconciliation, branch topology, and auto-route geometry. A new element,
  rung, branch node or wire gets the smallest free numbered id of its kind
  (`contact_1`, `rung_2`, `edge_<rung id>_3`), and a new timer, counter,
  compare or math block the smallest free instance name (`T_1`, `C_1`,
  `CMP_1`, `MATH_1`) with its operands declared as locals. Invalid shortcut or
  topology requests fail without partial mutation.
- A number typed into the Ladder properties panel (timer preset in
  milliseconds, counter preset, position X or Y) is read by the language
  server (`trust-lsp.visualEdit` `setNumber`) as JavaScript's `Number(text)`
  reads it: a decimal number with an optional sign, fraction and exponent, or
  a `0x`, `0o` or `0b` integer, with blanks around it. A timer preset is
  rounded half up and a negative one becomes 0; a counter preset is rounded
  half up; a position snaps to the 20-unit grid. Blank, non-numeric or
  non-finite text is refused with a message under the field, and nothing is
  stored. Dragging an element snaps it in the editor.
- Blockly lowering preserves connected statement order, control-input slots,
  variable identity, and deterministic inference for supported untyped numeric
  variables.
- SFC topology validation accepts a well-formed parallel split/join and rejects
  missing continuations or fewer than two branches.
- The SFC companion executes the chart by IEC 61131-3 §6.7 as
  `sfc-profile.md` ("Execution of a visual SFC") states it: a parallel split
  enters all its branches together and a join leaves them together when every
  branch's last step is active and its conditions hold; transitions clear
  together, in priority order; actions run with final scan under the
  qualifiers N, R, S, L, D, P, SD, DS, SL, P1 and P0 of Table 59, with their
  durations; each step is read as `<step>.X` and `<step>.T` (Table 54). A
  transition's Priority field in the SFC editor's properties sets its
  `priority` (a whole number from 0, the lowest first; empty for none), which
  Validate checks.
- Ladder, Blockly, Statechart, and SFC execution authority is the generated
  Structured Text companion `<base>.st` and runtime entry
  `<base>.visual.runtime.st` (`17-visual-editors-runtime-unification.md` §3).
  Companion generation preserves declared symbols, addressed globals,
  topology, and event/action inputs. Saving a program, Sync Structured Text,
  and an editor's Generate write the same two files; no other Structured Text
  file is written for a visual program.
- A direct address that a program names becomes one global
  `ld_io_<address>` that the runtime entry declares `AT` the address, typed by
  the address size (`03-variables.md` §5: `X` `BOOL`, `B` `BYTE`, `W` `WORD`,
  `D` `DWORD`, `L` `LWORD`); a declared variable keeps its declared type at its
  address. A text written like an address that the runtime's address parser
  refuses (such as `%IX0.9`) makes a Ladder program untranslatable.
- A Ladder operand or a Statechart action target written like an address that
  the runtime refuses makes the program untranslatable; an output is never
  skipped silently.
- Text the companion writes is Structured Text: strings use the escapes `$'`
  and `$$` (`01-lexical-elements.md` §9), and a name or message inside a
  generated comment cannot end the comment or, in a `//` comment, the line.
- A Blockly chain that leads back to itself is reported as a warning and left
  out of the companion.
- Statechart Validate refuses a chart its companion cannot be made of: no `id`,
  no state, or an action mapping in use without an action type. The editor's
  action-mapping panel shows the actions that states and transitions name
  without a mapping, and the mappings nothing names, as the language server
  answers them.
- The Statechart auto-transition timer and an action-mapping value are read
  by the language server from the text typed (`trust-lsp.visualEdit`
  `afterDelay`, `mappingValue`). A blank timer removes the delay; otherwise
  the timer is a number of milliseconds from 0 (read as `Number(text)`, its
  fraction dropped), and other text is refused with a message while the
  transition keeps its delay. A mapping value of exactly `true` or `false` is
  a Boolean, text that reads as a finite number is that number, and any other
  text, blank text included, is kept as text.
- SFC Validate checks the chart on the canvas and refuses a chart its
  companion cannot be made of (`sfc-profile.md`, "What Validate refuses");
  saving, Preview ST and Generate ST show those findings instead of a
  companion. Preview ST shows the companion without writing a file; Generate
  ST writes the companion and runtime entry. An action's duration is typed as
  a `TIME` literal and checked by the language server.
  A file whose steps or transitions are not lists, or a parallel split or join
  without `branchIds`, opens with the reason instead of failing.
- Saving a Statechart keeps the chart's `id`; saving an SFC keeps its `name`
  and metadata (`author`, `description`, `created`, `version`) and sets
  `metadata.modified` to the time of the save.
- The runtime entry runs the companion in one task with
  `INTERVAL := T#20ms` and `PRIORITY := 1`.
- Ladder division is Structured Text division: a zero divisor faults as it
  does in Structured Text (`16-ladder-profile-trust.md` §4).
- The editors have no execution engine, runtime controls, runtime settings or
  I/O pane of their own (`17-visual-editors-runtime-unification.md` §5): a
  visual program runs through its runtime entry, and its values show in Live
  Values.
- A new program's name is trimmed and may end with the suffix of its kind,
  which is cut; what remains must be one or more letters, digits, hyphens or
  underscores. The new file is the template of its kind with that name.
- An import accepts a JSON file of the shape of its kind: a Ladder program has
  a `networks` array, a Blockly workspace a `blocks.blocks` array, a Statechart
  `id` and `states`, an SFC `name`, `steps` and `transitions`. The copy keeps
  the file name and gets the suffix of its kind when the name lacks it, so it
  opens in its editor and gets its companion.
- An imported Ladder, Statechart or SFC file that already lies inside the
  workspace folder opens where it is; a file outside it, and every Blockly
  workspace, is copied. A file lies inside the folder when the folder's names
  begin its path, name by name, so `/ws-other/a.sfc.json` is not inside `/ws`.
  On Windows and macOS, whose file systems ignore case, the names match in any
  case, as VS Code compares paths there.
- Creating a Ladder, Blockly, Statechart, or SFC program opens the new file in
  its visual editor, in the active editor group and with focus. With
  `trust.visual.openStructuredTextCompanionOnCreate` on, the generated
  Structured Text companion is written and opens beside it without taking
  focus; with it off, no companion opens, and the companion is written only
  when `trust.visual.generateStructuredTextCompanions` is on.
- While `trust.visual.openVisualEditorsAutomatically` is on, a `.ladder.json`,
  `.statechart.json`, `.sfc.json`, or `.blockly.json` file opened as text also
  opens in its visual editor. A file already shown in its visual editor in the
  current window session stays in the text editor, so `Open as text`
  (section 8.3) keeps working.

#### 11.4.1 Execution engines and runtime panes removed

The TypeScript Ladder, Blockly, Statechart and SFC executors, their runtime
clients, the embedded runtime and I/O panel, its controller and bridge, and
the PLCopen Ladder interchange that only tests reached were removed on
26 October 2026 with the tests that locked them. No editor-specific component
defines visual execution: the generated Structured Text is the only execution
path, and its rules are tested in `trust-visual`.

### 11.5 HMI authoring

HMI authoring workflows are specified in `docs/specs/39-hmi-builder.md`.

### 11.6 Libraries, project creation, persistence, and focused recovery

- Library dependency edits preserve unrelated manifest structure. Path and Git
  dependencies parse deterministically; a Git dependency has exactly one
  reviewed pin selector; update and removal mutate only the selected entry.
- Library code actions are offered only for known OSCAT or PLCopen Motion
  symbols when the corresponding dependency is absent.
- The panel reads `[dependencies]` by the language server's own rules,
  including `[dependencies.<name>]` tables. An updated entry keeps its place
  and the comment above it, a new entry ends `[dependencies]`, and a removed
  entry takes the comment directly above it. A Git pin of 7 to 40 hexadecimal
  digits is `rev`, an optional `v` followed by `<digits>.<digit>` is `tag`,
  anything else is `branch`. Library code actions come from the language
  server: a QuickFix `Add <library> to this project` that runs
  `trust-lsp.libraries.add`.
- Create Project is cancellation-safe at every prompt, requires confirmation
  before using a nonempty target, produces parseable/buildable ST and TOML, and
  resolves single-root and multi-root workspace targets deterministically.
- Created control-lesson projects MUST persist their lesson identity and exact
  project location through the existing VS Code extension persistence. The
  record is additive, so two copies of one lesson remain independently
  reopenable and creating a project MUST NOT imply that its lesson is complete.
  Continue may select only a retained location whose project still exists. A
  deleted or moved location remains a visible project record with explicit
  recovery guidance; it MUST NOT be silently replaced by another copy or by a
  newly created project. A retained-copy action uses a QuickPick of the exact
  saved locations; a missing location offers Locate moved folder or Cancel.
  The generic Open existing folder picker remains separate from Create another
  copy.
- Visual right-pane width persistence uses stable per-editor keys, clamps
  configured bounds, prefers VS Code webview state, falls back to local
  storage, and falls back again to the default for invalid values. It governs
  authoring panes only.
- Compile reports actionable missing-runtime and report-version mismatch
  recovery. Runtime source options preserve explicit include globs and apply
  documented defaults only when they are absent.
- The simulator compiles the files its runtime source options choose: the
  include globs below the project folder (`**/*.{st,ST,pou,POU}` by default),
  less the exclude globs and the files whose first 20 lines hold an ignore
  pragma (`@trustlsp:runtime-ignore` by default), plus the entry, which is
  always taken, and the sources of the project's local package dependencies.
  One implementation (`trust_runtime::runtime_sources`) serves the debug
  adapter and the language server. This selection is not the build's:
  `trust-runtime build` and `check` take every source below `src/`, while the
  simulator keeps marked test sources out (specification 38 §3).
- Before a debug session starts, every `PROGRAM … : <type>` entry of its
  CONFIGURATION names a PROGRAM that those sources declare, by its
  namespace-qualified name or, for a type without a namespace, by its own
  name; `TEST_PROGRAM` declarations and configuration instances declare no
  PROGRAM type. Otherwise the session does not start, and the message names
  the unknown types (`trust-lsp.runEntry`). An open editor's text counts
  instead of its file.
- A debug session starts from a CONFIGURATION file: a Structured Text file
  below the workspace folders, outside `node_modules`, `target` and `.git`,
  whose syntax declares a `CONFIGURATION` (the word in a comment does not
  count). With several, Start lets the user pick; an automatic start takes the
  one it started from last, else the only one in the workspace folder of the
  active Structured Text file, else the first by path, and notes its choice in
  the debug output (`trust-lsp.runEntry`).
- Without one, Start writes a default CONFIGURATION that runs one PROGRAM in
  task `MainTask` every resource cycle. Its `RESOURCE` and task interval are
  the `[resource]` name and `cycle_interval_ms` of the project's
  `runtime.toml`, which a build requires the program to match (specification
  11 §6.8.1d), else `Res` and 10 ms, the simulator's cycle without one
  (specification 13). Its PROGRAM is one that the active file declares, else
  one that the simulator's sources declare: Start lets the user pick among
  several, and an automatic start takes the active file's first, else the
  first by name. The file goes to the workspace folder of that PROGRAM's file,
  else the only folder, else the folder the user picks (an automatic start:
  the active file's folder, else the first). It is `configuration.st`, else
  the first free name of `configuration_2.st` to `configuration_100.st`; an
  existing file is never overwritten.
- Live Values MAY retry a bounded transient-busy mutation. The retry does not
  authorize immediate success before a newer runtime state confirms the
  mutation.
