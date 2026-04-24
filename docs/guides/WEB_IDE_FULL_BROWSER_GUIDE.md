# Web IDE Browser Guide

This guide covers the product Web IDE hosted by `trust-runtime` at `/ide`.
It is separate from the static `docs/demo/` showcase.

## Scope

The `/ide` surface is the runtime-hosted browser IDE for real project work:

- project selection/open flow (including no-bundle startup)
- workspace tree navigation
- file create/rename/move/delete
- multi-tab editing with save/format/build/test/validate workflows
- language intelligence (diagnostics, hover, completion, definition,
  references, rename)

## Start the Web IDE

1. Standalone IDE mode (no runtime cycle):
   `trust-runtime ide serve --project /path/to/project --listen 127.0.0.1:18080`
2. Runtime-hosted mode:
   `trust-runtime run --web --project /path/to/project`
3. Open:
   `http://127.0.0.1:18080/ide`

Notes:

- `trust-runtime config-ui serve ...` is still accepted as a deprecated alias for `ide serve`.
- If startup has no active project, use the IDE "Open Folder" flow to select a workspace root.

## Core API Surface

Session/capability:

- `GET /api/ide/capabilities`
- `POST /api/ide/session`
- `GET /api/ide/project`
- `POST /api/ide/project/open`

Workspace/filesystem:

- `GET /api/ide/tree`
- `POST /api/ide/fs/create`
- `POST /api/ide/fs/rename`
- `POST /api/ide/fs/move`
- `POST /api/ide/fs/delete`
- `GET /api/ide/fs/audit`

Documents/language:

- `GET /api/ide/files`
- `GET /api/ide/file`
- `POST /api/ide/file`
- `POST /api/ide/diagnostics`
- `POST /api/ide/hover`
- `POST /api/ide/completion`
- `POST /api/ide/definition`
- `POST /api/ide/references`
- `POST /api/ide/rename`
- `POST /api/ide/format`
- `GET /api/ide/symbols`

Build/test/validation:

- `POST /api/ide/build`
- `POST /api/ide/test`
- `POST /api/ide/validate`
- `GET /api/ide/task`

Health/telemetry:

- `GET /api/ide/health`
- `POST /api/ide/frontend-telemetry`

## Validation and Evidence

Implementation contract/spec:

- `docs/internal/runtime/trust-runtime-web-ide-full-specification.md`

Checklist and evidence:

- `docs/internal/testing/checklists/web-ide-full-implementation-checklist.md`
- `docs/internal/testing/evidence/web-ide-full-checklist-evidence-2026-02-15.md`

Accessibility/collaboration references:

- `docs/guides/WEB_IDE_ACCESSIBILITY_BASELINE.md`
- `docs/guides/WEB_IDE_COLLABORATION_MODEL.md`
