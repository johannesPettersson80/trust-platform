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
![Debug + Runtime](assets/debug.png)

### Runtime I/O panel
![Runtime I/O panel](assets/hero-runtime.png)

### Rename across files
![Rename across files](assets/rename.png)

---

## Commands You’ll Use Most

- `Structured Text: New Project`
- `Structured Text: Open Runtime Panel`
- `Structured Text: Open HMI Preview`
- `Structured Text: Start Debugging`
- `Structured Text: Attach Debugger`
- `Structured Text: Run All Tests`
- `Structured Text: Run Test`
- `Structured Text: Move Namespace`
- `Structured Text: Create/Select Configuration`

---

## Advanced Setup (optional)

Set custom binary paths if needed:

- `trust-lsp.server.path`
- `trust-lsp.debug.adapter.path`
- `trust-lsp.runtime.cli.path`

Full docs: https://github.com/johannesPettersson80/trust-platform/tree/main/docs
