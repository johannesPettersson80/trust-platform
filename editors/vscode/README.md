# truST LSP for VS Code

**truST LSP** brings IEC 61131-3 Structured Text productivity to VS Code:

- Fast diagnostics and semantic highlighting
- Go to definition/references, rename, and formatting
- Runtime panel with live I/O control
- HMI preview panel with live schema + value updates
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
- `Structured Text: Open HMI Preview`
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

## HMI Descriptor + LM Workflow

Use the HMI descriptor workflow when building `hmi/` pages and process SVG
layouts:

1. Run `Structured Text: Initialize HMI Descriptor`.
2. Open `Structured Text: Open HMI Preview`.
3. In LM-driven flows, use deterministic tool order:
   - `trust_hmi_init` -> `trust_hmi_get_bindings` -> `trust_hmi_get_layout`
   - `trust_hmi_apply_patch` (`dry_run=true` first, then apply)
   - `trust_hmi_validate` / `trust_hmi_run_journey` for evidence checks

Detailed guide:
- `docs/guides/HMI_DIRECTORY_WORKFLOW.md`

---

## Advanced Setup (optional)

Set custom binary paths if needed:

- `trust-lsp.server.path`
- `trust-lsp.debug.adapter.path`
- `trust-lsp.runtime.cli.path`

Full docs: https://github.com/johannesPettersson80/trust-platform/tree/main/docs
