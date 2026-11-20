### Added

- A debug image times each call of a Rust POU against the budget its `#[budget]` declares for the
  image's target, on a computer and on a board. A call over the budget is the event
  `rust_budget_exceeded` with the POU's name, the call's time and the budget, recorded for the
  POU's first call over its budget and then only for a longer one, so a POU over its budget in
  every scan does not fill the event list; `tasks.stats` lists each Rust POU's budget, longest call
  and calls over the budget (specification 48 RU-49).
