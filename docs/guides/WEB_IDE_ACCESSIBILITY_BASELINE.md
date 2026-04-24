# Web IDE Accessibility Baseline

Scope:

- `/ide` browser authoring surface in `trust-runtime` web UI.

Date:

- 2026-02-15

## Baseline Coverage
- Focus order is predictable: sidebar -> tabs -> editor -> insight panels.
- Skip link is present (`Skip to IDE content`) and moves focus to `#ideMain`.
- File tree is keyboard reachable and exposed as `role="tree"`.
- Open tabs are keyboard reachable and include explicit labels.
- Command palette is exposed as `role="dialog"` with `aria-modal="true"`.
- Save, command palette, and theme actions are keyboard accessible buttons.
- Status and diagnostics regions use `aria-live="polite"` for incremental updates.
- Light/dark themes preserve semantic colors and contrast via shared tokens.

## Keyboard-Only Validation
Run `trust-runtime` web UI and open `/ide`, then verify:

1. `Tab` reaches skip link, sidebar file tree, tab bar, editor, and insight cards.
2. `Ctrl/Cmd+Shift+P` opens command palette; arrow keys change selection; `Enter` runs command; `Esc` closes.
3. `Ctrl/Cmd+S` saves active file.
4. `Ctrl/Cmd+Tab` and `Ctrl/Cmd+Shift+Tab` cycle open tabs.
5. Offline/online transitions update visible status without mouse interaction.

## Known Limits
- Full screen-reader narration quality for CodeMirror internals is inherited from upstream behavior.
- Multi-user cursor presence is intentionally out of scope for phase 1.
