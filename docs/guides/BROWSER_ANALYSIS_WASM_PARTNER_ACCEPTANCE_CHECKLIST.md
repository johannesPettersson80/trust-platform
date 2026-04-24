# Browser Analysis WASM Partner Acceptance Checklist

Purpose:

- Shared acceptance list for evaluating integration into a partner web editor (for example OpenPLC-style shells).

## A. Functional Acceptance

- [ ] Analyzer initializes in browser worker without server-side analysis service.
- [ ] Document apply loop works for incremental edits (`applyDocuments`).
- [ ] Diagnostics update after edits with correct ranges and severities.
- [ ] Hover content appears for symbols at mouse position.
- [ ] Completion suggestions appear while typing and can be accepted with keyboard.
- [ ] Stale responses are ignored when newer document revisions exist.

## B. UX Acceptance

- [ ] End-user flow requires no manual API/test harness buttons.
- [ ] Diagnostics, hover, and completion are visible in editor workflow.
- [ ] Keyboard completion flow supports Arrow/Enter/Tab/Escape.
- [ ] Visual shell integration fits partner editor branding/layout.

## C. Reliability Acceptance

- [ ] Startup errors are surfaced clearly to user.
- [ ] Per-request timeout behavior is handled gracefully.
- [ ] Cancel envelope is supported for in-flight requests.
- [ ] Worker failure state is detectable and recoverable by host.

## D. Performance Acceptance

- [ ] Edit-to-diagnostics latency stays within accepted product target.
- [ ] Completion latency remains low enough for interactive typing.
- [ ] Hover latency remains low enough for pointer exploration.

## E. Scope Guardrails

- [ ] Integration explicitly scoped to analysis-only workflows.
- [ ] Runtime execution model changes are not implied by this integration.
- [ ] Non-goals are documented and accepted by both teams.

## Evidence Links

- Demo script:
  `docs/guides/BROWSER_ANALYSIS_WASM_DEMO_SCRIPT.md`
- Integration brief:
  `docs/guides/BROWSER_ANALYSIS_WASM_INTEGRATION_BRIEF.md`
- OpenPLC mapping:
  `docs/guides/BROWSER_ANALYSIS_WASM_OPENPLC_EVENT_MAPPING.md`
- OpenPLC shell demo page:
  `docs/internal/prototypes/browser_analysis_wasm_spike/web/openplc-shell.html`
