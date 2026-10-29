# 43. Agent-driven HMI authoring

> **Plant integration, 6 October 2026:** The ordinary HMI Builder remains usable without an AI provider or agent workflow. Agent tools use the same component/panel/project contracts specified in [hmi-component-catalogue.md](hmi-component-catalogue.md). Plant Library integration acceptance is tracked separately by PL/CAT rows; previous agent-tool evidence does not prove the new Plant workflow.

Status: implementation specification, 4 October 2026. Product implementation and acceptance
remain open. Owner: the HMI Builder. Implementation tracker: the **Agent-driven HMI authoring**
section of `docs/internal/testing/checklists/hmi-builder-implementation-checklist.md`.

## 1. Purpose and authority

An AI agent shall create and refine a visually polished, operationally understandable HMI using
truST engineering tools. The customer uses their existing agent conversation and the existing
visual builder interchangeably. The agent understands the process, composes a proposal, renders
it, inspects the actual image, corrects defects, and presents an editable result.

Specification 39 owns the document format, shared drawing, approved visual design, bindings,
operator authorization and deployment. Specifications 40–42 own alarms, equipment and links.
This specification owns agent authoring, draft transactions, visual feedback and acceptance.
It supersedes specification 39's direct model-calling assistant workflow. It does not restore
the retired descriptor format or require compatibility with its implementation.

The product supplies engineering tools, not a new model provider, chat window, subscription or
model login. The unfinished Codex SDK adapter is outside this design and shall be removed with
its own dependency/test/documentation additions, preserving unrelated work. A model is required
only by the customer's agent client; ordinary visual editing and operation remain independent.

### 1.1 Inspected starting point

- Canonical checkout `trust-platform`, main `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`:
  57 VS Code language-model tools, including 11 descriptor-era HMI tools.
- Implementation checkout `trust-platform-hmi-builder`, branch `feat/hmi-builder`, base
  `a85185968a7e97a4bcc46d8a10a504f93446f792`, with substantial uncommitted implementation:
  46 general tools. HB-0-01 removed the old HMI tools and tests explicitly.
- New HMI LSP commands already expose catalogue, bindings, parse/format/check, bundle,
  faceplates, page proposals and proposed missing-variable edits. Reuse their owning services.
- Current quality checks cover touch targets, missing units, unbound objects, clipped text,
  contrast and confusing colours. They do not establish a well-composed screen.
- `scripts/hmi-screen-tests` and the `hmi-screen-tests` CI job already exist. The job is absent
  from `ci.yml`'s aggregate gate dependencies. Its current build uses checked-in drawing assets.

These are source-inspection findings, not fresh execution or acceptance results.

## 2. Customer journeys and boundaries

### 2.1 Create from a program

The customer asks for an overview of a real project. The agent reads project context and the
visual library; states the operator task, proposed page structure and any material unknowns;
creates a draft; renders and inspects it; fixes defects; and opens the proposal in the builder. The review uses the shared primary-action style for
Apply, with an explicit disabled reason when engineering checks fail.
The customer can accept it, request changes, or edit it manually. Accepted pages are ordinary
TOML and normal builder objects, not agent-owned opaque output.

### 2.2 Refine an existing page

The customer selects equipment or a region and asks for a change. The agent reads that revision,
changes only the relevant objects, renders the changed region and whole page, and presents the
diff. Manual changes made in the meantime survive or cause an explicit conflict. Undo works.

### 2.3 Start from the process

A customer can describe a process or supply a reference drawing before PLC instances exist.
The draft may contain explicitly unresolved bindings. The tool returns proposed program edits
through the existing missing-variable service; it must not silently invent executable process
logic or claim the HMI is runnable. Applying PLC changes is a separate reviewed operation,
followed by recompilation and binding validation.

### 2.4 Design, simulation and operation

Authoring never starts a runtime, deploys a bundle, changes live values or acknowledges alarms.
Synthetic preview is labelled; an explicitly selected simulator journey is separate. Physical
runtime writes retain specification 39's named login, policy, confirmation and audit path.
No generic agent authoring tool bypasses it. Deployment remains the existing separate workflow.

## 3. Architecture and supported clients

One authoring service owns document snapshots, draft operations, revisions and findings. It calls
the existing native file/binding services and shared drawing/layout functions. Avoid a second
parser, renderer, binding resolver, theme, equipment catalogue or patch engine.

The authoring service runs in the language server, `trust-lsp`, because its results are stored in
the project (AGENTS.md: all logic is in Rust). Each authoring decision is a `trust-lsp` command: the
context and its queries (AT-01), the library (AT-02), page inspection (AT-03), composition with its
patterns and layout (AT-04), draft edits including pipe and geometric edits (AT-05), validation with
the geometry and routing quality findings and the review of page changes (AT-06), and the draft
records with their lifecycle, revisions, hashes and receipts (§6). The language server persists the
draft records in the storage folder the extension passes (its workspace storage). The extension keeps
what presents and wires: the two adapters below, VS Code document access (it passes the unsaved texts
to each command, as the HMI Builder's commands receive them), rendering and rasterizing (AT-07),
preview data and the simulator (AT-08), and the editor transaction that applies an approved draft
(AT-09). An adapter forwards each tool call to its command and returns the command's answer; it
holds no rule of its own.

Two thin adapters expose the same versioned contracts:

1. VS Code tools via `contributes.languageModelTools` and `vscode.lm.registerTool`.
2. A local MCP adapter for the Codex IDE client, forwarding to the active VS Code authoring
   service. It must see the same unsaved documents, selection and draft state. Its MCP rules are
   in `trust-lsp hmi-agent` (§3.1).

The MCP bridge is opt-in, local-only, scoped to one selected VS Code window/workspace, with an
unpredictable per-session capability and authenticated connection. Discovery must distinguish
multiple windows; ambiguity returns a selection error. Credentials stay in private session
storage, never project files or tool output. Do not expose an arbitrary VS Code command executor,
LSP proxy or filesystem API through this bridge. Remote SSH/container workspaces bind to their
own extension host; do not resolve their URIs as local paths. If that connection is unsupported,
return an explicit unsupported-context error. Reconnecting invalidates expired capabilities.

An initial connectivity spike must prove both adapters against the same open unsaved page before
completing the tool suite. Other agent clients can use the documented MCP contract but are not
advertised as verified until their actual client journey has run. No automatic edits to the
customer's agent configuration: setup presents the exact server entry to connect.

The implementation architecture diagram shall show both adapters, document service, native
validation, renderer, draft store and editor transaction boundary. SOLID/KISS/DRY acceptance:
small responsibility-specific modules, no file grown past roughly 1,000 lines, shared semantics
across adapters, no second model orchestration layer.

### 3.1 The MCP adapter: `trust-lsp hmi-agent` (26 October 2026)

The MCP adapter is the stdio MCP server `trust-lsp hmi-agent <connection file>`, a subcommand of
the language server binary that the extension already ships. It owns every rule of the MCP
connection (AGENTS.md: all logic is in Rust): message form, protocol version, request ids, limits
and cancellation. The extension's loopback (`agent/bridge.ts`) checks only the transport and
invokes the VS Code tools. Rust tests: `crates/trust-lsp/src/hmi_agent_mcp/tests/`.

The adapter lists the loopback's tools with its own annotations, whatever the loopback sends:
`hmi_compose_page`, `hmi_edit_draft`, `hmi_apply_draft` and `hmi_preview_states` change the
draft or the project and are not read-only, `hmi_apply_draft` writes the project's files and is
destructive, every other tool only reads, and no tool reaches beyond the workspace
(`openWorldHint` false).

**Connection.** **HMI: Connect Agent Tools** starts one loopback per workspace of the window, on
127.0.0.1 with an ephemeral port and a random 64-digit hexadecimal capability, and writes the
connection file `{ "url", "capability", "workspaceId" }` with mode 600 into the extension's
private storage. The configuration entry it shows names the trust-lsp binary that the extension
launches as its language server, with the arguments `["hmi-agent", <connection file>]`. The
server refuses a file that group or other users may access (Unix), a URL other than
`http://127.0.0.1:<port>/mcp` without credentials, query or fragment, a capability other than 64
lowercase hexadecimal digits, and an empty workspace. It then asks the loopback for its tools. When
any of this fails, it writes "Unable to open the private HMI authoring connection. Connect from VS
Code first." to standard error and ends with status 1; a usage error ends with status 2. At the end
of standard input it waits until the calls in progress have answered and ends with status 0.

**Messages.** Standard input carries one JSON-RPC 2.0 message per line, ended by `\n` or `\r\n`.
Standard output carries only the answers, one per line, and never the capability. Parameters that
are not an object are read as none.

| Message | Answer |
| --- | --- |
| A line over 8 MiB (8,388,608 bytes before its line end), discarded unread | the failure answer below, id `null` |
| Not JSON, or not UTF-8 | -32700 "Invalid JSON.", id `null` |
| Not an object with `jsonrpc` "2.0" and a string `method`, a batch included | -32600 "Invalid request.", with its id when that is a string or a number, otherwise `null` |
| An `id` that is not a string or a number | -32600 "Invalid request id.", id `null` |
| A notification (no `id`) | none; `notifications/cancelled` cancels the call with its `requestId` |
| `initialize` | `protocolVersion`: the client's when it is 2025-11-25, 2025-06-18, 2025-03-26 or 2024-11-05, otherwise 2025-11-25; `capabilities: { tools: {} }`; `serverInfo` `trust-hmi-authoring` 1.0.0; the instructions "Use the connected VS Code project's HMI tools. Inspect context and the visual library before editing. Render and inspect proposals, validate bindings, and review changes before applying. Authoring is separate from deployment and equipment operation." |
| `ping` | `{}` |
| `tools/list` | the loopback's tools as it listed them at the start: the `hmi_` language-model tools of `package.json`, defined once |
| `tools/call` | the tool's result, after the checks below |
| Any other method | -32601 "Unknown method." |

**Tool calls.** `tools/call` checks, in this order: `name` is a listed tool (-32602 "Unknown
authoring tool."); `arguments` is an object (-32602 "Tool arguments must be an object.");
`arguments.workspaceId` is absent or the connection's workspace (-32602 "The tool belongs to
another workspace."); no call with the same id is in progress and fewer than eight calls run
(-32600 "Duplicate request id or too many active requests."). Ids are compared as JSON values, so
7 and "7" are two ids. The call then runs with the connection's workspace set in its arguments, on
its own thread, so that `ping` and cancellation are answered while calls run. Every other message
is answered before the next line is read, so at most eight requests are in progress at once. A
cancellation, or a call still running 120 seconds after it started, cancels the call at the
loopback once; the tool then answers its cancellation, or its committed outcome (§4), as the
call's result. The result is the tool's: `structuredContent`, `isError` and `content`, images
included.

**Failure.** A call whose exchange with the loopback fails is answered -32000 "HMI authoring
connection unavailable or request limit exceeded. Reconnect in the selected VS Code window." with
its id: the loopback cannot be reached, answers with a status other than 200, answers more than
64 MiB counted in raw bytes, answers no JSON, or has not answered 125 seconds after the call
started. The answer is read whole and parsed once complete (Repairs — 7 October 2026, item 3).

**Loopback.** The loopback refuses a request with an `Origin` header (403), without the bearer
capability (401, compared in constant time), on another path (404), with a method other than POST
(405), without a JSON content type (415), with a body over 8 MiB (413), or with a body that is not
the route's JSON object (400). Its routes: `POST /mcp/tools` answers `{ "tools" }`;
`POST /mcp/call` with `{ "id", "name", "arguments" }` invokes the named VS Code tool for the
loopback's own workspace, whatever the arguments name, and answers the tool's result (409 while a
call with that id runs, 500 when the tool fails); `POST /mcp/cancel` with `{ "id" }` cancels the
call with that id. The server gives each call its own id with a random prefix, so two servers on
one connection never share one. The loopback holds no MCP rule.

## 4. Common tool contract

MCP results expose the shared envelope as `structuredContent` and as JSON text for older clients.
Domain failures set `isError: true`; transport success must not disguise a rejected authoring
operation. This follows the [MCP tool result contract](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

All requests have `contractVersion: 1`, a workspace identifier, and a cancellation context.
The bridge derives workspace authority from its connection; a caller cannot choose an arbitrary
filesystem root. URI handling must preserve scheme, authority and case semantics; reject path
traversal, symlink escape and writes outside the authorized project.

Successful responses have `{ ok: true, data, revisions, warnings }`; failures have
`{ ok: false, code, message, retryable, details }`. Read queries may return explicitly partial
results with continuation tokens. Never silently truncate a catalogue or pretend an unavailable
compiler means an empty program. Schema errors identify the selected operation and its invalid field rather than listing every
unrelated union alternative. Stable errors include `INVALID_ARGUMENT`, `UNSUPPORTED_VERSION`,
`PROJECT_UNAVAILABLE`, `COMPILATION_FAILED`, `CONFLICT`, `CANCELLED`, `VALIDATION_FAILED`,
`RENDER_FAILED`, `NOT_CONNECTED`, `UNSUPPORTED_CONTEXT`, `LIMIT_EXCEEDED`, `EXPIRED_DRAFT`.

A clean cached editor document must not resurrect a file deleted from disk. A dirty open buffer
remains authoritative until the customer saves or discards it.
Revisions cover dirty editor contents and saved files, using document versions plus content
hashes. Draft IDs and object IDs are stable opaque identifiers; file names are not object IDs.
Every mutation supplies an expected draft revision and an idempotency key. Repeating the same
request returns its original outcome; reusing the key with different input fails. Cancellation
before commit makes no changes; cancellation after commit reports the committed revision.

Initial product limits: 100 returned catalogue entries per page; 1 MiB input text per file;
8 MiB per mutation; 32 files or 500 operations per batch; 50 MiB retained drafts per workspace;
5 simultaneous drafts; render dimensions at most 4096 pixels per side and 16 megapixels; 12
frames per preview batch. Return actionable limit errors and support smaller batches. Initial
timeouts: complete project discovery 120 seconds (including declared Plant/Control/OSCAT
compilation), catalogue/page inspection 30 seconds, validation/composition 120 seconds, and
render/state batch 120 seconds. Cancellation must end owned subprocesses and release resources. These are bounded
operation limits, not hardware performance acceptance claims.

Artifacts identify source/draft revision, theme, language, viewport, scale, renderer version,
scenario, synthetic/live origin and timestamp. Image results include viewable image content or
a supported resource plus a manifest; a filesystem path alone is insufficient for agent vision.
No capability tokens, runtime cookies, passwords or model credentials enter artifacts.

## 5. Authoring tools

These nine public tools define the initial API. Read and design operations must be available
without a runtime. Tool descriptions explain purpose, mutation effects and the next useful step.
MCP annotations and VS Code invocation messages must accurately distinguish read-only discovery,
draft mutation and source application. Draft tools cannot acquire source-write authority merely
by accepting an arbitrary path; apply remains the only source mutation in this tool set.

### AT-01 `hmi_get_context`

Input: selected project/page, requested sections and continuation cursor. Output: screen design
size and target viewport, theme/token settings, page navigation, equipment instances and types,
variable paths/kinds/directions, available units/ranges and their provenance, alarms, faceplate
pairs, current selection and document revisions. Include compilation errors and unresolved
metadata. Selection of a project in a multi-root workspace must be explicit when ambiguous.

A program instance proves equipment identity and typed members, not physical piping or operator
priority. Return connectivity as known only when present in project data or explicitly supplied
by the customer; inferred relationships carry provenance and remain proposals.

### AT-02 `hmi_browse_library`

Input: component/equipment/task query, category and theme. Output: bounded visual contact sheets
and structured definitions: IDs, default/minimum sizes, binding kinds/directions, ports, supported
states, faceplate links and available composition patterns. Include project custom symbols.
Each image cell maps to an exact definition/version. Cache by catalogue and theme revision.
The library also returns `compositionPatterns`, separately from component pagination, filtered
by the same text query. Each entry has a stable pattern ID and version, operator task, required
inputs, region hierarchy, spacing and density guidance, and visual-review status. Candidate
patterns remain labelled `pending` until their reference images have been reviewed; availability
in a tool is not visual approval. Their IDs are the exact values accepted by compose.
Searching for an exact pattern ID returns a browser-rendered reference image and its normal
builder object model. Reference bindings use an explicitly synthetic `Example` namespace,
including mixed unit states and a declared example process branch. They are design references,
not project connectivity, valid customer bindings, control logic, or deployable drafts. The agent
must replace them with inspected project facts. Render through the shared drawing and bundled
fonts in the requested theme; report unavailable components rather than substituting shapes.
A reference image is a design input, not executable content; preserve semantic state colours.

### AT-03 `hmi_inspect_page`

Input: page or draft, optional selected object IDs or rectangle. Output: object tree, stable IDs,
geometry, groups, z-order, pipes/ports, bindings, navigation/actions and resolved faceplates.
Explain the provenance of values, units, ranges and visual tokens. Include the revision and
unresolved references. Region queries include connected neighbouring objects as references,
without pretending they were selected for editing. A `region` is `{x, y, width, height}` in
design pixels, with nonnegative origins and positive dimensions; it selects objects whose rotated
design boxes overlap its interior. `ids` and `region` are mutually exclusive. Selection preserves
the page's object order and reports original zero-based z-order, including an empty selection.

### AT-04 `hmi_compose_page`

Input: source page or new page name, operator task, equipment selection, composition pattern,
regions, emphasis, confirmed process connections, fixed objects and target constraints. Output:
a draft ID/revision, editable object model, rationale, assumptions, unresolved bindings and
initial validation findings. Optional `operatorTask`, `rationale` and `assumptions` are persisted
with the proposal and shown in the existing builder review. When the task is omitted, use the
selected pattern's stated task (or the existing page title for a source-page refinement), mark
that provenance explicitly, and retain the facts that previews are synthetic and connectivity
is supplied rather than inferred. These notes are not PLC code or runtime authority.
Optional alternatives are limited to three named drafts with a
short explanation of the design tradeoff; do not generate meaningless random variants.

The tool computes a deterministic layout from supplied constraints; the calling agent chooses
and evaluates the design. It does not call another model. Unsatisfiable constraints return the
conflicting objects/constraints, not silently smaller text or cropped content. It may create a
multi-page proposal when that is explicitly requested; otherwise return the suggested split.

### AT-05 `hmi_edit_draft`

Input: draft ID, expected revision, idempotency key and typed operations. Supported operations:
add/remove/duplicate objects; change allowed properties; bind/unbind with direction; move/resize;
group/ungroup; align/distribute; reorder; connect/disconnect/reroute ports; place/rebind a
faceplate; add/change navigation; arrange a region while respecting pinned objects.

`equal-size` names selected IDs, a reference object and width/height/both; placed faceplates
retain their aspect ratio and incompatible equal-both constraints fail. `arrange` names IDs,
an in-page grid region, gap/fill direction and optional pinned IDs. Locked or explicitly pinned
objects keep their geometry, and unselected non-background objects reserve their occupied space.
`reroute` identifies one connection by from/to and supplies up to 64 explicit waypoints; an empty
list restores automatic shared routing. `reconnect` changes its named endpoints while retaining
its bindings, style, manual waypoints and instrument attachments. Invalid/self connections and
duplicate reconnections fail before replacing the draft. Pipe deletion/duplication remaps
index-based instrument attachments through the existing editor owners.

Apply the batch to a copy, check schema and structural integrity, then replace the draft revision
once. Invalid operations leave it unchanged. Preserve unrelated properties, IDs and connections.
Deletion reports dependent pipes/navigation/bindings and requires those effects in the proposal.
Return object-level before/after changes and findings. Edit receipts persist the operation's
native-model diff and structural findings with its committed revision, so a retry returns the
same receipt even after later edits. Engineering validity still requires explicit validation. Low-level coordinate edits remain possible
for deliberate refinement; they do not bypass document constraints.

### AT-06 `hmi_validate_draft`

Input: draft/revision, validation scope and target viewport profiles. Output: findings with
stable rule ID, severity, object IDs, file location and where relevant image rectangles,
measured/required values and suggested remedy. Reuse native schema, pair and binding validation.
Distinguish compile failure, unknown paths, wrong value kind and forbidden write direction.
Include semantic and visual checks from section 7. Return both engineering validity and unresolved
visual review state; no single score or `ok` flag means the design is beautiful.

### AT-07 `hmi_render_draft`

Input: exact draft/revision, viewport, theme, language, design scenario and optional crop/object
IDs. Output: actual browser-rendered PNG, artifact manifest, object hit map and available layout
measurements. Render the same drawing and presentation code used by the builder/runtime.
Use deterministic fonts, clock, animation phase and history for repeatable captures. Capture
whole-page context and optional native-scale detail; do not conceal defects by shrinking to fit.
Unsupported rendering or a missing font is reported, not replaced by a schematic placeholder.
Project image previews initially accept static PNG, JPEG and self-contained static SVG files under
`hmi/assets/`. Reuse the native HMI bundle reader and editor-authoritative SVG text; never fetch a
remote image. Limit each binary image to 8 MiB, SVG text to 1 MiB, and expanded referenced image
bytes to 32 MiB per frame, with at most 32 different images in a proposal. Decode failures, animations and external SVG dependencies fail
explicitly. Freeze decoded images into the same shared drawing before capture. Asset content
hashes accompany the image manifest and the review; changed image content prevents stale apply.


### AT-08 `hmi_preview_states`

Input: exact draft/revision and bounded named scenarios, optionally a declared simulator target
and permitted read/navigation steps. Output: images and findings per scenario, interaction trace,
readback where applicable, and explicit synthetic versus executed evidence labels. Synthetic
scenarios include normal/running, stopped, fault, alarm, stale, unknown, long translated text,
large/negative values and write-denied/command-pending presentation.

Synthetic presentation does not write to equipment. Navigation and faceplate opening can run
in the isolated preview. Simulator control actions, when tested, use the real named-session
command path and a separate explicit test authorization; no silent fallback to a live target.
Bound step count and duration; timeout reports the failed step and retains evidence.

### AT-09 `hmi_apply_draft`

Input: action `review`, `apply` or `discard`, draft/revision and expected base revisions.
Review returns changed files/objects, warnings, current validation and before/after previews,
and opens the proposal in the builder. The review uses the shared primary-action style for
Apply, with an explicit disabled reason when engineering checks fail. Apply requires current engineering validation and the
customer/agent client's normal edit authorization; visual warnings remain visible and do not
silently become new deployment blockers. A knowingly incomplete design may be saved only via an
explicit `allowIncomplete` choice, labelled incomplete, never as validated or deployable.

Recheck all base revisions immediately before applying. If any changed, return conflicts and
write nothing; the agent can make a new draft from the latest state. Use one undoable editor
transaction, update open views, and preserve unsaved status. Undo of a newly created page
removes its diagnostics as well as its contents; a deleted clean editor model must not resurrect
a missing-format error. Saving is an explicit option and
must report actual save results separately. No raw sequential multi-file writes presented as
atomic success. Run read-only review/program/asset/engineering preflight inside the draft mutation lock before
persisting an application intent. Re-read the stored draft in that lock. A failed preflight
consumes no application receipt and may be retried after correcting/reviewing the proposal.
Source-file preparation and the last pre-commit version checks likewise report a definite
no-edit outcome if they fail before invoking the editor transaction. Only that typed internal
outcome may clear a prepared receipt. Once the editor transaction is invoked, an unknown error
or refusal retains the uncertain receipt; a different request key, edit or new review may not
bypass it. Committed retries still return the original durable outcome without another edit.
Define rollback/recovery for filesystem failures and recover outstanding drafts
after extension restart. Discard removes only the named draft/artifacts, never source files.

## 6. Draft lifecycle and composition library

Creation requests require a caller-supplied unique `key`; replaying the same JSON data (regardless
of object-field order) returns the original proposal without rebuilding it. Reusing that key with
changed input conflicts. Discard removes proposal content but retains a minimal identity/request
receipt so old requests cannot resurrect it. Apply/discard retries are scoped to the draft and
expected revision; an applied draft cannot return to reviewed state.

Lifecycle: `draft -> validated -> reviewed -> applied` or `discarded`. Any edit invalidates bound
validation and renders. Review records its exact revision and unresolved findings only after
validation and preview production succeed; a failed or cancelled preview leaves the proposal
unreviewed. Old renders
remain identifiable but cannot certify a new revision. Persist private draft data in the
extension's workspace storage folder (written by the language server, §3); retain until
applied/discarded or explicitly cleaned by the customer. Context
includes recoverable draft IDs, current revisions, states and changed file names, allowing the
agent to resume or explicitly discard a proposal after restart without guessing identifiers.
Each persisted proposal carries SHA-256 hashes of its current file contents; edits regenerate
these hashes, and reads reject a mismatched record rather than exposing corrupted proposal text. A
record of an earlier format is not read (nothing of the HMI is released yet): it is reported as
draft storage to clean up. Malformed JSON is reported as corrupt draft storage with an actionable
typed error.
The listing is workspace-scoped and excludes discarded or applied proposals and source bodies. Quota
exhaustion asks for draft cleanup rather than deleting work automatically. An unavailable base
on restart produces a conflict, not reconstruction from guessed content.

Five initial approved composition families are required:

- process overview with dominant process area and supporting measurements;
- equipment detail with consistent state, measurements, faceplate/control entry points;
- repeated multi-unit overview with equal card hierarchy;
- trends and alarm investigation with equipment navigation;
- settings/commissioning with clearly grouped editable values.

Each is normal builder objects with a version, intended operator task, annotated reference image,
spacing and proportions, required inputs, supported density/viewport, and documented limits.
At least one process example includes a branch and one repeated-unit example includes mixed
states. Reuse the approved showcase design; do not assume all existing example pages are approved
patterns. Reference screenshots and visual verdicts must accompany each library entry.

For a lightly populated process overview, enlarge the primary equipment while preserving its
aspect ratio (up to 2.5 times the catalogue default when space permits). Stack supporting
measurement cards in a consistent side column, using the available width; catalogue thumbnail
sizes alone must not leave the operating process tiny in a large empty page. Explicit customer
sizes remain deliberate constraints. Dense pages retain fit checks rather than shrinking text.

Layout uses specification 39's 8-pixel grid, shared typography and semantic colour roles. Support
region constraints, padding/gaps, alignment, equal sizing, minimum readable sizes, pinned objects,
flow direction and port-aware routing. Initial explicit layout constraints use `layout.regions`
(keyed by the existing process/measurements/trends/alarms/controls roles), each with `x`, `y`,
`width`, `height`; `layout.gap` defaults to 24 design pixels and `layout.flow` to horizontal.
All region coordinates, dimensions and gaps use the 8-pixel grid. Regions remain below the
page heading and within the 32-pixel page margins, and may not overlap. Unspecified occupied
roles retain their default region; conflicts with an explicit region fail with both role names.
An item may supply `pinnedAt: [x,y]`; its catalogue or explicit size and position are preserved.
Auto placement reserves these boxes, including rotated bounds, and fills available grid cells
in the declared flow order. Pinned items must fit their role and may not overlap one another.
Failure names the role/item and recommends a larger region or page split; it never reduces
readable sizes. These constraints propose geometry only and never infer process connections.
Place instrument readings near their equipment. Trends,
alarms and secondary detail must not compete with the primary operating task. If content cannot
fit readably, propose scrolling or another page rather than compressing everything.

## 7. Visual quality contract

The goal is clear process understanding and a polished implementation of the approved design.
No arbitrary new CSS, parallel theme, remote image dependency or unsupported component can be
introduced by agent output. New custom symbols use the existing native symbol workflow and
require explicit visual review before joining the approved library.

Automated checks shall cover:

1. Invalid or unresolved bindings, missing faceplates, broken links, missing units and ranges.
2. Out-of-bounds content and unintended overlap, excluding declared containment/backgrounds,
   symbol internals and intentional overlays; do not flag every intersecting bounding box.
3. Actual text clipping at target scale, translated labels and representative numeric extremes.
4. Rendered touch-target size and readability at the target viewport, in addition to design-size
   checks. An explicit Fit view is not proof of readable operation.
5. Shared contrast/state-colour rules across both themes and supported visual states.
6. Pipe/body/text collisions, disconnected ports and ambiguous process routing, allowing explicit
   approved crossings and distinguishing connectivity from geometric intersection.
7. Repeated-component spacing/alignment/size consistency, missing state indication and unclear
   navigation destinations. Repeated faceplate cards report differing sizes. A non-overlapping
   row of at least three cards, with top edges within 16 design pixels, reports differing top
   edges and gaps differing by more than 8 pixels. Separate rows are not inferred to be one row;
   these findings are warnings with measured geometry and an explicit intentional-layout remedy.
8. Broken or inaccessible faceplate/navigation interactions and layering that hides controls.
   Authoring discovery and draft validation read declarations; neither compiles nor executes
   PLC code to discover alarm text. When actual alarm definitions are unavailable, explicitly
   requested synthetic exploration may show labelled `Preview alarm` samples for equipment
   already bound on its pages. Their reserved `.__preview_alarm` paths are preview-only, never
   generated PLC identities or live alarm commands. Ordinary Build mode does not activate these
   samples. Synthetic values and alarms use separate stream messages, and the embedded preview
   is contained inside review chrome.
   Synthetic exploration resolves an omitted faceplate instance from the same common binding
   parent as the editor; alarm equipment links use that resolved instance too. It must support
   page navigation, popup close, placed cards and alarm Details then Show equipment entirely
   within the captured snapshot, without source application or runtime login/writes.

`hmi_validate_draft` optionally accepts up to three `viewports` with unique `id`, `width`, and
`height`. Dimensions describe the available HMI content area, excluding browser/editor chrome.
Use the shipped adaptive layout, reporting fit scale, content size, scrolling and object
rectangles. Geometry alone is not physical-panel or browser-rendering proof. An actionable `viewport-touch-target` warning
includes actual width/height and the required 44-pixel minimum. Native design-size checks remain.
Rotated drawn boxes determine page-boundary and overlap findings. Fully containing group/panel
backgrounds and instruments explicitly attached to their equipment are intentional; other
positive-area object intersections are review warnings, including their object IDs and overlap
rectangle. Touching edges alone are not overlap. A group association alone is not permission
for controls to cover one another. Limit these geometry findings to 200 per page and explicitly
report truncation. These remain visual warnings, not new engineering/deployment errors.

Each rule has positive and deliberately defective fixtures to prove detection and avoid false
positives. Existing visual warnings retain their product severity; agent acceptance may require
fixing or explicitly reviewing them without changing runtime deployment rules.

The agent must additionally inspect whole-page and detail images and record a concise review of:
operator-task clarity, information hierarchy, process readability, balance/spacing, consistent
visual language, and visibility of abnormal conditions. Findings name affected objects and a
concrete remedy. A passing numeric test cannot substitute for this review. Final showcase
acceptance requires a human visual verdict. No fabricated objective beauty score.

The refinement loop stops when mandatory findings are resolved and the review is recorded.
After three unsuccessful repair iterations, present the remaining issues and alternatives to the
customer; do not loop indefinitely or weaken checks to obtain a pass.

## 8. Evidence and tests

Follow the user-directed implementation-first validation cadence in AGENTS.md: finish the
implementation and necessary tests before the consolidated batch, reuse unchanged passing
results, and retain the complete failure ledger. Do not fabricate pre-fix failures or repeat
passing gates for reassurance. Register VS Code tests in the real runner.
Fixtures cover dirty documents, multi-root workspaces, schemes/authorities, cancelled requests,
concurrent edits, stale drafts, repeated requests, malformed batches, filesystem failures,
restart recovery, escaped paths and expired bridge credentials. Keep adapter contract tests
identical and verify MCP image delivery to the actual Codex client.

Four acceptance projects are required: tank/pump/PID process; repeated multi-unit equipment;
branching thermal or mixing process; and one withheld project not used while tuning patterns.
Include at least one create-from-description and one refine-existing-page journey. The PID
fixture executes the real `Control.PID`; simulation is not physical qualification.

Required viewports: 1920x1080 desktop, 1280x800 panel, and 1024x768 constrained panel; dark/light;
English plus the longest supported fixture translations. The page design size and viewing scale
must be recorded separately. Scrolling may be appropriate; it must be intentional and checked.

Acceptance must demonstrate discovery -> composition -> screenshot inspection -> targeted repair
-> review -> apply -> manual edit -> agent revision -> Undo. Inject a bad binding, clipped label,
wrong-state colour, pipe collision and intervening manual edit. The agent must observe and repair
or report each correctly. Preserve prompts, tool inputs/results, revisions, images, diagnostics,
client/version and human visual verdict without credentials.

## 9. CI and release implementation

Yes: agent authoring requires additional CI/CD coverage, and the HMI itself needs existing gates
connected and strengthened. CI proves deterministic mechanics and regressions. Model-backed
acceptance and physical-panel performance are distinct evidence classes.

### 9.1 Required pull-request gates

- Native parser/binding/pair/proposal/transaction tests plus applicable runtime vertical suites.
- Registered VS Code tool tests, adapter parity, dirty-document/Undo/conflict/recovery tests.
- Local MCP transport and workspace-isolation tests; assert unavailable bridges fail clearly.
- Rebuild drawing/display/editor bundles from source and fail if tracked generated assets differ.
  Build the runtime embedding those exact outputs before browser verification.
- Real-browser authoring fixtures, operator journeys and screenshot regressions using locked
  dependencies, installed matching browser, fixed fonts/locale/time/scenarios. No paid model,
  user account or production credentials required for fork pull requests.
- Editor/runtime/draft-preview parity at equal dimensions/data; use specification 39's defined
  rasterization tolerance and negative-control test rather than widening thresholds on failure.
- Link HMI jobs into the aggregate gate and exact-candidate guard. A failed, cancelled, missing
  or unexpectedly skipped required HMI job must not yield aggregate success. Test this wiring.
- Upload image baselines/actuals/diffs, Playwright traces, findings, revision manifests and test
  summaries on failure; retain reviewed success evidence for releases. Do not auto-approve
  changed baselines. Baseline updates require image review tied to the candidate revision.

Extend existing `ci.yml` HMI and VS Code jobs; introduce a separate job only where ownership or
runtime isolation justifies it. Use reusable gate scripts so builder preflight and CI execute
the same command shapes. Relevant paths include HMI/native commands, shared drawing/themes,
agent adapters, fixtures, examples, lockfiles, build scripts and workflows. Shared dependencies
must trigger the required suites. Preserve branch-protection and release-guard enforcement.

### 9.2 Scheduled and manual gates

Nightly: broader page/state/language matrix, long-session memory/reconnect tests and deterministic
performance trend measurements. Scheduled/manual model-backed runs use an explicitly configured
agent account and spending limit, with no credentials on fork code. Prove both VS Code Agent
and Codex client journeys; a deterministic transcript replay cannot replace real-client proof.
Record model/client/version and distinguish service outages from product failures. New releases
of the agent feature require current real-client acceptance on the candidate, even though model
availability does not block every unrelated pull request.

Physical Pi 5 performance retains specification 39's budgets. CPU-only CI is not Pi evidence.
AM6442/A53 remains unavailable per the user; record its gate as unverified, never passed.

### 9.3 Packaged product and customer CI

Release smoke tests must install the produced VSIX in a clean profile, verify tool registration,
bundled native executables and rendering assets, start the bridge, and complete a fixture draft
journey without developer paths. Run packaging/launch checks on every shipped target; browser
pixel baselines may use a pinned Linux environment. User-facing documentation identifies tested
clients and setup steps. Continue normal release checks; a green development host is not proof
that the shipped extension contains the tools.

Customer projects reuse `trust-runtime hmi check` and `hmi test` for bindings and approved screens.
Add documented CI templates with nonzero exits for defects and retained actual/diff artifacts.
Drafts and agent transcripts are not deployed. Build/version the HMI bundle against the intended
program and retain deployment/rollback compatibility tests. CI never automatically deploys an
agent draft to physical equipment. Deployment authorization stays in specification 39.

## 10. Delivery order and completion

A: connect both agent interfaces to context, library, inspection and rendered output.
B: one complete tank/pump compose-refine-apply slice with shared editor transactions.
C: state previews, process routing, additional compositions and collaborative editing/recovery.
D: withheld-project acceptance, target matrix, packaged-product proof and all required CI gates.

Do not declare completion from tool registration, mock responses or screenshot file existence.
Completion requires the four project journeys, current visual review, both actual clients,
transaction/failure coverage, mandatory CI and packaged smoke evidence. Remaining hardware or
external-service limits must be listed explicitly. Implementation steps are tracked individually
in the owning checklist; unchecked rows remain work, not implied completion.

## 11. Existing-tool migration decisions

The new contracts replace descriptor-specific arguments/results; do not silently reinterpret
old requests. Keep the 46 general ST/file/LSP/debug tools independently registered.

| Former tool | New owner / disposition |
|---|---|
| trust_hmi_get_bindings | hmi_get_context, current native variable tree and metadata |
| trust_hmi_get_layout | hmi_inspect_page, current object/pipe/faceplate model |
| trust_hmi_apply_patch | hmi_edit_draft + hmi_apply_draft; retire descriptor JSON-pointer writes |
| trust_hmi_plan_intent | explicit composition inputs/review rationale; retire _intent.toml format |
| trust_hmi_trace_capture | hmi_preview_states explicit simulator evidence, no implicit live capture |
| trust_hmi_generate_candidates | hmi_compose_page constraints and up to three meaningful alternatives |
| trust_hmi_validate | hmi_validate_draft shared native validation + visual findings |
| trust_hmi_preview_snapshot | hmi_render_draft real shipped renderer; retire schematic candidate SVGs |
| trust_hmi_run_journey | hmi_preview_states bounded navigation/simulator journeys |
| trust_hmi_explain_widget | hmi_inspect_page property/binding/token provenance |
| trust_hmi_init | hmi_compose_page + explicit reviewed application using current project scaffold |

Port observable tests that still express current product behaviour (cancellation, workspace
selection, conflict reporting); rewrite descriptor fixtures. A legacy test proving an SVG file
exists cannot establish rendered-preview correctness. Registration parity alone is not client
invocation proof. The replacement is a documented tool-contract version change, not backward
compatibility with the retired descriptor API.

### Debug-session boundary

Debug sessions do not publish runtime mesh catalogues. A mesh-catalogue request reaching the
debug control bridge must return an explicit unavailable result, rather than silently dropping
the response or advertising an invented empty catalogue. This does not add remote operation
authority to agent authoring.

### 10.1 Implementation completion additions (4 October 2026)

The current execution order is implementation first, followed by one consolidated validation
batch. Historical milestone and red/green instructions do not authorize intermediate runs.
Implementation records remain distinct from final verification and customer visual acceptance.

Context accepts `sections`, a selected page `file`, and bounded page `cursor`/`limit`. The
`sources` section, answered only when a request names it, holds the text and unsaved state of each
page and popup as the documents observe them; a review with `explore: true` takes its snapshot from
it. Its `compilation` object explicitly identifies a failed compiler and `variables: null`; mutation
services still require a usable compiled program. Selection comes from the active builder panel
within the connected project and includes the observed document version. A specific page may be
selected when several panels are open. Catalogue results include content identities, supported
shared preview states, faceplate pair definitions, and optional category/state filtering.

Inspection accepts `draftId` with its exact `revision`, or a source page. It returns groups,
resolved placed faceplates, navigation metadata and binding/display-format provenance. Draft
inspection does not alter source. Project overrides use the same native faceplate parser.

Composition accepts an explicit `pages` array of at most 32 unique destinations as an alternative
to a single page. All pages are prepared before one draft commit; a failed page creates no partial
proposal. `layout.padding` reserves an inner margin on the 8-pixel grid; `equalSize` accepts width,
height or both and refuses to distort a faceplate or resize fixed equipment. Existing-page title
changes pass through the native formatter.

Render and state-preview requests accept `viewport: {width,height}`. The response keeps the full
native design image and optional native crop, then adds the first viewport of the shipped adaptive
layout, with its actual scale, scrollable content size and transformed hit map. Viewport images do not replace native detail review. Synthetic
scenarios include pending/refused commands, numeric minimum/maximum, and the shared longest-text
preview. Unsupported component-specific states remain listed explicitly.

Review accepts a qualitative `visualReview` with reviewer, verdict and object/file-specific issues.
Accepted reviews cannot contain unresolved issues. Edits invalidate the verdict; repair history
survives, and the third unsuccessful review requests a customer decision. No beauty score or
runtime authorization is derived from that record.

`hmi_validate_draft` with `proposeProgram: true` returns the existing native missing-variable
proposal separately, never applies it, and identifies the program revision. After separately
reviewing PLC changes, recompile and create a fresh HMI proposal. `allowIncomplete: true` on apply
explicitly saves all proposal pages with native `incomplete = true`; bundle validation refuses
deployment even when their current bindings happen to resolve. The page metadata edit can clear
that marker only as an explicit subsequent change, still subject to normal engineering checks.


Named simulation journeys are connected explicitly through **Connect Named HMI Simulator Journey**.
The selected target must report simulation mode and expose a local HMI web endpoint. A physical
mode, non-loopback endpoint, changed target or absent HMI endpoint is refused without fallback.
`trust-lsp.hmiAgent.checkTarget` decides whether a target is a local simulator, when the journey
is connected and before each of its requests: the target reports simulation mode, its HMI web
address names `localhost`, `127.0.0.1` or `[::1]`, and its control endpoint is `tcp://` to one of
those hosts with a port, `unix://` or `npipe:`. Without the language server's answer the target is
refused.
The user supplies a named simulator HMI login through the editor; credentials never become tool
inputs or outputs. Context lists opaque session IDs and expiry. Sessions last at most ten minutes.
`hmi_preview_states` may explicitly select `mode: simulator`, a `sessionId` and up to twelve steps.
Each step names a deployed page, optional control/action/value and optional readback path/expected
value. The deployed page must match the supplied draft and its local program revision must still
match. A journey never starts a process or deploys an HMI implicitly. It uses the existing named
session, command outcomes and readback stream; refusals, disconnects, cancellations and timeouts
are returned with the already-completed outcomes. It never resends a command automatically.

Review with `explore: true` includes a bounded snapshot of up to 32 project pages/popups and placed
faceplates, rendered through the existing runtime display in the proposal panel. This mode's
transport reads only that snapshot and refuses login and commands; it has no runtime connection.
Image review remains available alongside exploration. Neither an exploration interaction nor a
synthetic state frame is simulator or physical-operation evidence.

CI command owner: `scripts/hmi_ci_gate.sh` (bundle parity and browser phases). The normal PR
aggregate requires the HMI job, and candidate preparation records `remote_hmi`. The nightly HMI
workflow reuses that command and runs a clean-profile installed-VSIX authoring smoke. It never
uses source-checkout binaries for installed-artifact proof. It packages its debug binaries without
their debug information: vsce's secret scan reads each packaged file into one string, which Node
caps at 512 MiB, and the debug `trust-runtime` is larger with it (29 October 2026). `scripts/hmi-authoring/installed-vsix-smoke.cjs`
accepts a produced VSIX and a pinned VS Code executable override; native binaries, drawing assets,
fonts, tool invocation, PNG output and bridge startup must come from the installed artifact.
Customer CI template: `.github/workflows/templates/trust-hmi-project-ci.yml`. It requires an
administrator-managed runner with a pinned truST installation; it never downloads executable
code from an untrusted PR or updates approved screens automatically. Do not enable self-hosted
runners for untrusted public forks. CI reports and schema validators are evidence plumbing,
not substitutes for native behavior or actual-client acceptance.

Controlled actual-client runs are manually dispatched by `.github/workflows/hmi-real-agent.yml`
on protected `main`, using the `hmi-agent-acceptance` environment and a dedicated
`trust-hmi-agents` runner. `HMI_REAL_AGENT_DRIVER` is an administrator-configured executable for
that runner's already-installed/logged-in IDE profile. It must drive the actual selected client,
not replay tool calls or invoke a replacement model client. Its interface is recorded in
`scripts/hmi-authoring/real_client_gate.py`; the bounded run names four independent projects,
allows at most 80 tool calls and times out after 20 minutes. Provider/account/spend policy remains
in the protected runner configuration. No such credentials or executable configuration are taken
from a PR. Missing driver/profile is an unavailable prerequisite (exit 77), never a green run.
The artifact identity/hash check does not itself establish model quality or human acceptance.
Provisioning the protected account/profile/driver and recording both actual-client journeys remain
external acceptance work; this implementation does not claim they have happened.

### Cold discovery budget correction — 6 October 2026

Compose, edit and validate perform the same compiled-project discovery as get-context and
therefore share its 120-second ceiling. Cancellation remains immediate and prevents late
results from applying changes. This ceiling is not a responsiveness or performance claim;
record cold elapsed time separately. File and canvas editing shall not wait on discovery.

### Repairs — 7 October 2026

These repairs supersede conflicting statements above. Native tests: the draft store's Rust tests
(`crates/trust-lsp/src/handlers/commands/hmi_agent/drafts_tests.rs`, `review_apply_tests.rs`), the pipe tests
(`pipe_edits_tests.rs`, `pipe_rules_tests.rs`), the stdio MCP server's tests (`crates/trust-lsp/src/hmi_agent_mcp/tests/`,
§3.1) and the extension's editor-transaction suite `editors/vscode/src/test/suite/hmi-agent-apply.test.ts` (the
TypeScript draft and pipe suites moved to Rust with the authoring operations, §3; the TypeScript stdio client and its
UTF-8 suite moved to Rust with the MCP rules, §3.1).

1. **Bounded draft storage.** Edit and creation receipts keep the committed revision, the file
   content hashes and the receipt data (object-level changes, findings, creation data); they
   never copy proposal pages or base snapshots. A retry of an edit whose revision later edits
   superseded returns that receipt with `superseded: true` and without page bodies. Each draft
   keeps its newest 64 edit receipts; an older retry is answered as a changed revision
   (`CONFLICT`) and never applied again. Applying a draft keeps only its identity, creation key
   and application receipt; discarding keeps only its identity and creation key. These finished
   records are retry receipts for seven days. When a draft is created, expired ones and those
   beyond the newest 64 leave, oldest first, so a workspace holds at most 69 readable records;
   a write that would pass the 50 MiB bound first removes finished records, oldest first. A
   temporary file of an interrupted write is removed under the mutation lock. A creation
   retried after its record left is a new request. Open drafts (neither applied nor discarded,
   including reviewed ones) are never removed automatically: when five exist, or open drafts
   alone fill the byte bound, the
   `LIMIT_EXCEEDED` refusal names each one with its pages, state and revision so the customer
   or agent can discard one. Discard and application receipts are never refused for space, so
   storage can always be cleaned up. An unreadable record is retained for inspection and
   counted, but no longer blocks discovery or creation. On an applied draft only an apply retry
   returns its original outcome; operations that need its pages return `EXPIRED_DRAFT`.
2. **Opening is not a conflict.** Apply compares page content: a page conflicts only when its
   existence or text differs from the draft's base. Opening, closing, saving or otherwise
   re-versioning a page in an editor without changing its text is not a conflict. Disk reads
   drop a UTF-8 byte order mark, as the editor model does. Snapshot revisions still report the
   observed editor version with the content hash.
3. **UTF-8 on the MCP connection.** The stdio server, `trust-lsp hmi-agent` (§3.1), reads each
   loopback answer whole and parses it once complete, so a multi-byte character split across
   network reads reaches the agent intact. The answer size limit counts raw bytes.
4. **One pipe rule.** Agent pipe operations use the builder's own pipe editing. A port counts
   only when the canvas offers it: a catalogue port of an object on the page that the shared
   drawing places. Connect uses the canvas's `addPipe`: a pipe joins two different ports that
   have no pipe yet, and a branch goes through a junction. A refused connection fails with the
   builder's reason: `INVALID_ARGUMENT` for one port twice, `CONFLICT` naming the port that
   already has a pipe. Reconnect applies the same rule with the moved pipe set aside and keeps
   its place, binding, style, bends and attachments. Reroute, reconnect and disconnect name an
   existing connection by its two ends; an unknown connection fails. Duplicate uses the
   builder's copy and paste with the agent's chosen IDs: pipes between the copies come along,
   an instrument keeps what it measures only when that was copied too, and bindings are
   unchanged. The process-overview reference shows its declared branch through a junction.
