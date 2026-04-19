# Screenshot Assets

Place Marketplace screenshots and GIFs here.

Expected files:
- `screenshot-diagnostics.png`
- `screenshot-refactor.png`
- `screenshot-debug.png`
- `screenshot-ladder-editor.png`
- `screenshot-statechart-editor.png`
- `screenshot-blockly-editor.png`
- `screenshot-sfc-editor.png`
- `demo-rename.gif`
- `demo-debug.gif`

## Fast Workflow

1. Start the extension development host (`F5`) and open a manual test workspace:
   - `manual-tests/trust-lsp-manual-tests.code-workspace`
2. Prepare each scene in VS Code:
   - diagnostics: show Problems panel with IEC-aware diagnostics
   - refactor: show `Structured Text: Move Namespace` flow
   - debug: show breakpoints/stack/variables during an active debug session
   - ladder/statechart/blockly/sfc: open the example file and let truST auto-open the visual editor
3. Capture images (interactive region select):

```bash
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-diagnostics.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-refactor.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-debug.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-ladder-editor.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-statechart-editor.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-blockly-editor.png
scripts/capture-screenshot.sh --output editors/vscode/assets/screenshot-sfc-editor.png
```

4. Record GIF (`demo-rename.gif`) with your screen recorder of choice.
5. Normalize/compress all media:

```bash
scripts/prepare-readme-media.sh --dir editors/vscode/assets
```

## One-command Auto Capture

For fully automatic captures from the real `examples/plant_demo` workspace:

```bash
scripts/capture-plant-demo-media.sh
```

For the Marketplace screenshots plus the public-doc visual-editor screenshots:

```bash
scripts/capture-readme-screenshots-auto.sh
```

For only the public-doc visual-editor screenshots:

```bash
scripts/capture-public-docs-visual-editors.sh
```

## Notes

- `capture-screenshot.sh` uses `grim+slurp` on Wayland, `scrot` on X11, then `flameshot` as fallback.
- `capture-plant-demo-media.sh` runs VS Code in extension-development mode (`editors/vscode`) so truST is active from startup.
- It uses layout-safe keyboard actions (works with Swedish keyboards).
- Use a clean VS Code layout for consistency: same zoom level, same panel widths, same theme.
- Keep UI text readable. If needed, re-run capture with `--max-width 1800`.
