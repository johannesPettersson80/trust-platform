# HMI Guardrail Checklist

Use this reference with `trust-hmi-contracts`.

## API Contract

- `hmi.schema.get` returns stable schema and widget IDs.
- `hmi.values.get` returns typed values with timestamps/freshness.
- `hmi.write` remains phase-gated and disabled by default.

## Safety Requirements

- No implicit force semantics.
- Write-capable fields must be allowlisted.
- Authorization is mandatory for writes.
- Audit logging is required for accepted writes.

## Process SVG Binding Rules

- Process pages bind against stable SVG element IDs.
- Prefer ID-based selectors.
- Allow only deterministic safe binding attributes such as `fill`, `stroke`, `opacity`, `data-value`, `x`, `y`, `width`, `height`, and text content.
- Binding failures degrade with diagnostics and last-known-good rendering; they must not crash runtime/UI.

## Mapping Baseline

- `BOOL` -> indicator/toggle depending on mode.
- Numeric -> gauge/value/slider depending on mode.
- Enum -> selector/readout.
- String -> text.
- Array/struct -> table/tree.

## Performance and Release Bar

- HMI polling must stay within cycle-time budget.
- WebSocket push path `/ws/hmi` is primary for live updates; polling remains the fallback.
- Descriptor edit to schema refresh p95 <= 1000 ms.
- Descriptor edit to client-visible update p95 <= 1500 ms.
- Value event to render latency p95 <= 100 ms, p99 <= 250 ms on local/LAN.

## Required Tests

- Schema snapshot compatibility tests.
- Typed value contract tests.
- Unauthorized write rejection tests.
- UI integration tests with zero manual config for read-only mode.
- Process SVG binding integration tests.
- WebSocket and polling fallback integration tests.
- SLO verification tests for refresh and live updates.
