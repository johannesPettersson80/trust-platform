# truST Web IDE Full Specification

Status: Active
Last updated: 2026-02-15
Owner: trust-runtime web IDE track

## 1. Objective

Deliver a real, full browser IDE for IEC Structured Text inside `trust-runtime` at `/ide`.

This is not a spike target. This is a product target.

The IDE must support:
- browsing real project folders and files
- opening and editing files directly from the browser
- language intelligence with mouse hover and inline completion while typing
- save/build/test workflows without leaving the browser

## 2. Reality Baseline

Current `/ide` behavior is not accepted as "full IDE."

Known gaps:
- read-only sessions are common in production mode and block authoring
- directory-first workspace navigation is incomplete
- language experience is inconsistent in multi-file project contexts
- current UI flow does not meet investor-demo quality for an IDE product claim

## 3. Scope

In scope:
- product browser IDE at `/ide`
- project explorer with directory tree
- editor with multi-tab authoring
- ST language intelligence (diagnostics, hover, completion, symbol navigation)
- project actions (save all, build, test, validate)
- reliability, security, accessibility, and performance gates

Out of scope for phase 1:
- multi-user simultaneous editing
- cloud workspace hosting
- runtime execution inside browser-only sandbox

## 4. Product Requirements

### 4.1 Workspace and File System UX

- IDE must render a directory tree for the project root.
- Users must be able to expand/collapse directories.
- Users must be able to open files from any visible directory.
- Users must be able to create, rename, delete, and move files/folders.
- File operations must be conflict-safe and error-visible.
- Hidden/system files must be intentionally filtered or clearly marked.

### 4.2 Editor UX

- Multi-tab editing is required.
- Dirty state, save state, and unsaved-change warnings are required.
- Optional split editor is phase-1 stretch; single-pane tabbed editing is minimum.
- Cursor position, active file, and language status must always be visible.
- Keyboard-first workflow is mandatory.

### 4.3 Language Intelligence

- Diagnostics update while typing with bounded debounce.
- Mouse hover on symbols shows type/definition info.
- Completion appears while typing with in-scope ranking.
- Go to definition is required.
- Find references and rename symbol are required before "full IDE" release.
- Cross-file analysis must be consistent for project-level symbols.

### 4.4 Search and Navigation

- Quick Open (`Ctrl/Cmd+P`) must open files by fuzzy search.
- Workspace text search must support include/exclude globs.
- Symbol search (file + workspace) must be supported.

### 4.5 Commands and Workflows

- Command palette is required.
- Save, Save All, Format Document, Build, and Test commands are required.
- Build/test output panel with timestamps and pass/fail summaries is required.
- Errors from build/test must link back to source locations when available.

### 4.6 Session, Auth, and Modes

- Viewer vs editor mode must be explicit and understandable.
- Read-only mode must never pretend to be editable.
- Session expiration must preserve local unsaved drafts.
- Re-auth flow must be non-destructive.

### 4.7 Reliability and Recovery

- IDE must never hang in indefinite loading state.
- Offline or server-loss state must be detected and surfaced.
- Local draft persistence must protect unsaved edits.
- Recovery flow after reconnect must prevent silent overwrite.
- Multi-tab same-file editing must surface collision warnings.

### 4.8 Security Boundaries

- All file operations must remain inside project root.
- Path traversal is forbidden and tested.
- Sensitive runtime/deploy material must have explicit policy (allowed or denied) with audit trail.
- Write operations require authenticated editor session.

### 4.9 Accessibility and UX Quality

- WCAG 2.1 AA baseline is required.
- Full keyboard workflow is required.
- Focus management and ARIA semantics are required for dialogs/panels.
- Light/dark mode parity is required.
- Visual quality must match runtime product UI system (not prototype styling).

### 4.10 Performance Targets

Reference hardware:
- Raspberry Pi 5 (8GB), Chromium stable, local runtime host

Targets:
- boot-to-interactive p95 <= 2.5s (medium project)
- completion visible p95 <= 150ms
- hover visible p95 <= 150ms
- diagnostics refresh p95 <= 300ms
- workspace search (<= 40 ST files) p95 <= 400ms

### 4.11 Unified Product Shell and Runtime Integration

- `/`, `/ui`, and supported tab deep links resolve to the unified `/ide`
  product shell. The shell bootstraps only from the complete bundled module
  set; an omitted required module is a packaging failure.
- The shell retains explicit tab panels, a status bar, a compact toolbar with
  overflow actions, the HTML hidden-attribute contract, and ARIA-selected tab
  state. A hidden panel must remain hidden regardless of its layout class.
- Online runtime control defaults to the same origin and attempts its explicit
  connect flow without embedding a second host authority. Wrapped and direct
  API response payloads normalize to the same client result.
- I/O configuration reads and writes follow the active workspace. A successful
  write updates that workspace's `io.toml`; changing the active workspace
  changes the route target rather than mutating the former project.
- The hardware module exposes the reviewed MQTT connectivity probe plus
  Runtime Cloud topology and realtime-link transport projection. Settings
  preserve the documented realtime-link fields when reading and writing the
  runtime configuration.
- Legacy fleet mutation routes removed from the product return not found and
  are not silently aliased to a current Runtime Cloud operation.

## 5. Architecture Contract

### 5.1 Frontend

- Product frontend must be bundled and served locally (`/ide/assets/*`), no CDN dependency.
- Editor engine is Monaco for phase 1, and product requirements in Section 4 are mandatory.
- Monaco implementation details may evolve, but behavior in Section 4 is the release contract.

### 5.2 Backend APIs

Required API families:
- Session/capability: `/api/ide/capabilities`, `/api/ide/session`
- Workspace tree/filesystem: `/api/ide/tree`, `/api/ide/fs/*`
- Document IO: `/api/ide/file`, `/api/ide/files`
- Language: `/api/ide/diagnostics`, `/api/ide/hover`, `/api/ide/completion`, `/api/ide/definition`, `/api/ide/references`, `/api/ide/rename`
- Health/telemetry: `/api/ide/health`

If legacy endpoints are kept, they must map cleanly to this contract.

### 5.3 Concurrency Model

- Writes must use optimistic concurrency (`expected_version`) or equivalent revision tokens.
- File system mutations must reject stale operations with actionable conflicts.

## 6. Definition of Done

The Web IDE is "full" only when all are true:
- directory tree browse/open/edit lifecycle is complete
- language intelligence works reliably across project files
- build/test workflows run from IDE with clear output
- save/recovery/conflict flows are validated
- accessibility + performance gates pass on reference hardware
- release checklist `docs/internal/testing/checklists/web-ide-full-implementation-checklist.md` is fully green

## 7. Validation Gates

Must pass before a release claim:
- `just fmt`
- `just clippy`
- `just test`
- Web IDE integration suite (API + frontend contract)
- Web IDE performance gate suite
- Web IDE accessibility validation

No "world class" or "full IDE" claim is allowed unless all gates pass.
