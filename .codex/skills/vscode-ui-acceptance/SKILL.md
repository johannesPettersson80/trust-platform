---
name: vscode-ui-acceptance
description: Maintain and prove truST VS Code UI acceptance journeys. Use for acceptance-board rows, ux_accepted gates, screenshot evidence, journey coverage, and visible workflow review.
---

# VS Code UI Acceptance

Use this skill when a VS Code workflow is not complete until a visible user journey is accepted.
Use `trust-test-authoring` first for the cataloged behavior and proof route; this skill owns the
additional rendered acceptance evidence.

## Workflow

1. Identify the acceptance row or create a new row before implementation.
2. Name the visible VS Code surface: Home view, Testing view, Problems, Live Values, Devices & Connections, HMI, generated-ST diff, or report webview.
3. Define the journey in user terms, not command-palette terms.
4. Define one observable behavior slice and add/register the smallest focused rendered interaction, state, or layout test before implementation.
5. Run it and confirm the expected behavior assertion is red because the feature is missing or the behavior is wrong; compile, dependency, harness, timeout, and unrelated failures do not count.
6. Implement only enough to make the same focused test green, then rerun it until green before starting another slice.
7. Capture screenshots using the `vscode-capture` skill for core visible states after the focused test is green.
8. Run the required theme triplet for core surfaces: light, dark, high contrast/forced colors when applicable.
9. Record the red and green test commands/results, and mark `ux_accepted` only after reviewer != implementer and evidence artifacts are attached.

## Acceptance Rules

- A slice that says "visible" must name the VS Code surface and attach screenshot evidence.
- CLI-only proof does not close a VS Code UX acceptance row.
- Static source-text assertions do not prove rendered behavior or layout.
- Disabled actions must carry the reason in the UI.
- Do not add a second Start/Run surface when the existing shell can be wired by project kind.
- New webview chrome must use shared theme tokens, not one-off colors.

## Reference

Read `references/journeys.md` for the expected journey/evidence shape.
