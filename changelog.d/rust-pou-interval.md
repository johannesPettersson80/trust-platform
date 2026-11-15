### Added

- A Rust POU reads its task's interval with `cx.interval()`: the task's `INTERVAL`, or the
  resource's cycle interval for a program without a task. `trust_plc::test::task(interval)` gives a
  unit test the context of a task with that interval, and `trust_plc::test::advance` moves it on to
  the task's next activation (specification 48 RU-16, RU-56).
