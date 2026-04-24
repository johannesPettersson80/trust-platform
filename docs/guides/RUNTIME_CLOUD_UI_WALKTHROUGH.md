# Runtime Cloud UI Walkthrough

Goal: operate any runtime from any connected runtime safely, including cross-runtime settings and incident visibility.

## 1) Open Fleet UI

Connect to any runtime web endpoint, for example `http://127.0.0.1:18081/`, then open the runtime cloud fleet section.

## 2) Context Bar: `connected_via` vs `acting_on`

- `connected_via`: runtime endpoint you are connected to
- `acting_on`: selected target runtime(s)

Always confirm both before write actions.

## 3) Fleet Visualization

Use graph and table together:

- graph shows runtime nodes and communication edges
- table gives sortable operational details
- selection is synchronized between graph and table

Expected UX behavior:

- stale/degraded/failed links are visually distinct
- node badges show health, role, and state transitions

## 4) Communication Lens

Filter by:

- plane (`T1`, `T2`, `T3`)
- keyspace/category

Use timeline playback to inspect incident windows and communication state transitions.

## 5) Safe Cross-Runtime Settings Flow

1. Select target runtime (`acting_on`)
2. Open settings/config panel
3. Run dry-run preflight first
4. Review blast-radius summary and per-target effect
5. Confirm apply
6. Verify per-target result includes `audit_id` and shared `request_id`

If revision/etag conflict appears, use rebase/retry flow. Do not silently force overwrite.

## 6) Rollout Center

For multi-target updates:

- create rollout
- watch per-target states
- pause/resume/abort as needed

Expected behavior:

- deterministic state transitions
- explicit failure reasons and affected target list

## 7) Incident and Audit Timeline

Correlate:

- config writes
- communication state changes
- HA role transitions
- audit outcomes

Each action should be traceable by `request_id` and `audit_id`.

## 8) Accessibility and Resilience Baseline

- keyboard-only navigation works end-to-end
- screen-reader labels expose critical state metadata
- reduced-motion mode preserves clarity
- partial data loss still shows last-known snapshot with stale confidence

## 9) Daily Operator Checklist

- verify context bar before writes
- run preflight for non-trivial or multi-target changes
- confirm blast radius and authorization result
- record `request_id` and `audit_id` for high-risk actions
