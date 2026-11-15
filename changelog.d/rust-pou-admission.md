### Added

- `trust-runtime build` admits a project's Rust work (specification 48 RU-46, RU-47): it warns of a
  task whose Rust POUs may take longer than its interval, each POU's `#[budget]` times the most calls
  a scan can make of it through the Structured Text that calls it, and of a task that calls a Rust
  POU in a loop without a bound. `trust.toml`'s `[timing] profile = "production"` makes the warnings
  errors, and `[timing.budgets]` may lower a POU's budget for a target.
