# Browser Analysis WASM Demo Script (5 Minutes)

Audience:

- Potential integration partners (for example web-based PLC editor teams).

Goal:

- Show live static analysis in browser with no server-side analyzer process.
- Show hover on mouse and autocomplete while typing.
- Show runtime UI parity styling with dark/light theme switching.

## 1. Demo Prep (30 seconds)

Run from repo root:

```bash
cd /home/johannes/projects/trust-platform
scripts/run_browser_analysis_wasm_spike_demo.sh --port 4173
```

Open:

```text
http://127.0.0.1:4173/web/
```

Optional OpenPLC-shell integration view:

```text
http://127.0.0.1:4173/web/openplc-shell.html
```

Then force refresh once:

- `Ctrl+Shift+R`

Expected:

- Status card shows `Worker ready. Live analysis active.`

## 2. Screenshot 1: Live Hover (60 seconds)

1. Move mouse over `Counter` in the editor.
2. Keep cursor still for a moment.

Capture:

- Hover popover next to the cursor.
- Hover card on the right populated with symbol/type details.

## 3. Screenshot 2: Theme Parity (45 seconds)

1. Click `Dark mode` in the left sidebar.
2. Click again to return to `Light mode` if needed.

Capture:

- Runtime-style sidebar/topbar/cards and truST logo.
- Theme toggle visibly switching the full UI palette.

## 4. Screenshot 3: Live Diagnostics (60 seconds)

1. In the editor, change a line to:
   `Counter := UnknownSymbol + 1;`
2. Wait a moment for live analysis.

Capture:

- Diagnostics badge > 0.
- Diagnostics list shows unresolved symbol error.

## 5. Screenshot 4: Autocomplete While Typing (75 seconds)

1. Add a new line and type:
   `Cou`
2. Wait for completion dropdown to appear.
3. Use `ArrowDown` and `Enter` (or `Tab`) to accept a suggestion.

Capture:

- Completion dropdown visible in editor.
- Completion card on the right shows suggestion list.
- Editor text updated after acceptance.

## 6. Talk Track (60 seconds)

Talking points:

- Analyzer is Rust compiled to WASM in a browser worker.
- Hover and completion are interactive in the browser editor itself.
- Diagnostics update automatically on edits.
- Browser demo uses the same design language and theme behavior as runtime UI.
- This path is analysis-only integration (runtime execution is out of scope).

## 7. Backup Commands

Parity/performance gate:

```bash
scripts/check_mp010_browser_analysis.sh
```

Build-only:

```bash
scripts/run_browser_analysis_wasm_spike_demo.sh --build-only
```
