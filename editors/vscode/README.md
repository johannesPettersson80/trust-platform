# truST LSP for VS Code

**truST LSP** brings IEC 61131-3 Structured Text productivity to VS Code:

- Fast diagnostics and semantic highlighting
- Go to definition/references, rename, and formatting
- Runtime panel with live I/O control
- Debugging with breakpoints, step, continue, and runtime values
- ST test workflow with CodeLens + Test Explorer

---

## Quick Start (1 minute)

1. Install **truST LSP** from the Marketplace.
2. Open a folder with `.st` / `.pou` files.
3. Start editing. Language features start automatically.

Command line install:

```bash
code --install-extension trust-platform.trust-lsp
```

---

## Open the Runtime Panel (super quick)

1. Press `Ctrl+Shift+P`
2. Run **`Structured Text: Open Runtime Panel`**
3. Pick **Local** or **External**
4. Press **Start**
5. Set Inputs and watch Outputs update

---

## What You Can Do

- Catch issues early with IEC-aware diagnostics
- Refactor safely: rename symbols and move namespaces
- Debug real logic with breakpoints + runtime state
- Drive and observe process I/O directly in the panel
- Run ST tests from CodeLens (`TEST_PROGRAM` / `TEST_FUNCTION_BLOCK`) and Test Explorer

---

## Build an HMI with your agent (feature preview)

Use the agent you already run in VS Code. truST supplies engineering tools; it does not ask you
for a separate model-provider account. The tools read the active project's saved and unsaved
HMI/ST documents, browse component images, compose or refine an isolated draft, and return real
rendered previews for your agent to inspect.

VS Code agents can discover the nine `hmi_*` tools directly. For an MCP client, run
**truST: HMI: Connect Agent Tools** in the desired workspace. The command opens connection
instructions for that window. Use the shown configuration in your agent's MCP settings; the
connection ends when the workspace/window closes. Do not share the private connection file.

A typical request is: “Inspect my project and component library. Propose a tank overview with
readable level and temperature readings. Render it, fix any clipped labels, and show me the
review before applying it.” Ask the agent to use `sourceFile` when refining an existing page.
Review opens a read-only proposal in the HMI Builder with before/after images, object changes
and findings. Apply or discard there, or ask the agent to do it. Project context lists saved drafts
so your agent can resume them after a restart. Application checks source revisions, uses
VS Code Undo, and reports saving separately. These authoring tools do not deploy the HMI or
operate the controller.

Use `hmi_browse_library` to discover the five composition families. Search for an exact family
ID to receive its synthetic reference image and normal builder object model. References use
`Example` paths; replace these with inspected project bindings. Composition accepts explicit
`layout.regions`, `layout.gap`, `layout.flow` and per-item `pinnedAt` coordinates to preserve the
intended hierarchy and existing equipment placement. Impossible layouts report their conflicts.
For target displays, pass up to three content `viewports` to `hmi_validate_draft`. It reports
projected control sizes, overlap rectangles, page bounds and the shared visual warnings; inspect
the images as well, since geometry checks do not establish visual quality or physical performance.
Project image previews support static PNG/JPEG/SVG assets and unsaved SVG edits; changing an
asset after review blocks Apply until a fresh review. A failure before any editor transaction is
invoked can be retried; an uncertain editor outcome requires source inspection and cannot be
bypassed with another request key.

This feature is still under implementation: complete IDE-agent acceptance, the full visual
matrix, project-image previews, complete faceplate browsing and transaction recovery are tracked in
[the HMI checklist](../../docs/internal/testing/checklists/hmi-builder-implementation-checklist.md).
Read-only tool/image access has been verified in the actual Codex IDE window as well as the CLI.
Complete authoring journeys and other IDE chat clients remain separate acceptance checks.

---

## Example Projects

`examples/` are **not bundled** inside the Marketplace extension package.

Use the GitHub repo examples instead:

- Guided tutorial index: https://github.com/johannesPettersson80/trust-platform/tree/main/examples/README.md
- Filling line demo: https://github.com/johannesPettersson80/trust-platform/tree/main/examples/filling_line
- Plant demo: https://github.com/johannesPettersson80/trust-platform/tree/main/examples/plant_demo

Open it in VS Code:

1. Clone the repo
2. `File -> Open Folder...`
3. Select `trust-platform/examples/filling_line`
4. Run `Structured Text: Open Runtime Panel`

---

## Screenshots

### Debug + Runtime in one view
![Debug + Runtime](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/editors/vscode/assets/debug.png)

### Runtime I/O panel
![Runtime I/O panel](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/editors/vscode/assets/hero-runtime.png)

### Rename across files
![Rename across files](https://raw.githubusercontent.com/johannesPettersson80/trust-platform/main/editors/vscode/assets/rename.png)

---

## Commands You’ll Use Most

- `Structured Text: New Project`
- `Structured Text: Import PLCopen XML`
- `Structured Text: Open Runtime Panel`
- `Structured Text: Start Debugging`
- `Structured Text: Attach Debugger`
- `Structured Text: Run All Tests`
- `Structured Text: Run Test`
- `Structured Text: Move Namespace`
- `Structured Text: Create/Select Configuration`

## PLCopen XML Import (UI Flow)

Use this when you want to create a truST project from an existing PLCopen XML file.

1. Press `Ctrl+Shift+P`.
2. Run **`Structured Text: Import PLCopen XML`**.
3. Pick the input XML file.
4. Pick the target project folder.
5. Confirm overwrite when importing into a non-empty folder.

The command runs `trust-runtime plcopen import --json` in the background and
lets you open the imported project and migration report after completion.

---

## Language Model Tool Contract (Contributor Note)

When adding or renaming a VS Code LM tool, keep the name in sync in exactly
three places:

1. `package.json` -> `activationEvents` entries using `onLanguageModelTool:<name>`
2. `package.json` -> `contributes.languageModelTools[].name`
3. `src/lm-tools.ts` -> `registerLanguageModelTools(...)`

The extension test suite now checks this contract in
`src/test/suite/lm-tools-contract.test.ts`. If one of the three surfaces drifts,
tool discovery will silently break for Copilot/LM clients and the test should
fail.

---

## Advanced Setup (optional)

Set custom binary paths if needed:

- `trust-lsp.server.path`
- `trust-lsp.debug.adapter.path`
- `trust-lsp.runtime.cli.path`

Full docs: https://github.com/johannesPettersson80/trust-platform/tree/main/docs
