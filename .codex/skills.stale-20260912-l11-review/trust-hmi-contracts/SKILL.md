---
name: trust-hmi-contracts
description: Implement and review trust-platform HMI schema/value/write contracts with safety guardrails. Use for MP-050..053 features, runtime HMI APIs, widget mapping, HMI web UI, process pages, writes, or authz behavior.
---

# Trust HMI Contracts

Use this workflow to keep HMI APIs stable, safe, and testable.
Use `trust-test-authoring` first for planner, catalog, invariant, oracle, and red-green routing.

## Required Guardrails

- Do not use `debug.variables` as the long-term HMI contract.
- Provide dedicated endpoints: `hmi.schema.get`, `hmi.values.get`, and phase-gated `hmi.write`.
- Keep writes disabled by default unless explicitly in scope.
- Allow writes only through an explicit allowlist plus authorization.
- Use stable widget IDs such as `resource/task/instance/field-path`.
- Enforce cycle-time impact budgets for polling and writes.

## Implementation Workflow

1. Define one observable contract or UI behavior slice and write its focused contract, interaction, or layout test first.
2. Run the focused test and confirm the expected behavior assertion is red before changing production code; harness, compile, dependency, timeout, or unrelated failures do not count.
3. Define or update the API contract and snapshot it before UI wiring, then implement only enough to make the same focused test green.
4. Implement deterministic type/direction to widget mapping.
5. Keep UI rendering dependent on HMI contracts, not debug transport internals.
6. Add write paths only behind phase gate, allowlist, and authorization checks.
7. Add performance checks for polling frequency and write impact.

## Validation

- Run schema/value snapshot tests, unauthorized-write negative tests, and widget mapping tests.
- Run targeted HMI API/UI/perf suites for changed behavior.
- Report the focused test's expected red result and the same test's green result.
- For browser-visible HMI changes, verify the live `/hmi` surface after the focused test is green with Playwright or an equivalent real browser session. Do not use Puppeteer MCP.
- If HMI assets are embedded into `trust-runtime`, rebuild and restart the runtime before browser verification.
- In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first and ask before broad local Rust gates.

## Reference

Read `references/hmi-guardrails.md` for the detailed contract checklist.
