### Added

- `runtime.build`, `runtime.validate`, and `runtime.compile_reload` accept an
  opt-in `issues_limit` cap, so large projects stay under agent hosts'
  message limits. An applied limit truncates `issues[]` to the first entries
  of the deterministic order, bounds the failed-build error text and the
  compile_reload build-failure `runtimeMessage`, and adds `issuesTruncated`
  and `totalIssues`. An omitted limit preserves the whole-list responses.
