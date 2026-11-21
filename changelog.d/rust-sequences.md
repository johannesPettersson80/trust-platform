### Added

- Rust sequences (specification 48 §15): `#[trust_sequence]` on a POU's `impl` block turns an
  `async fn` written as straight-line Rust, which waits with `s.next_cycle()`, `s.until(pred)` and
  `s.wait(t)`, each limited by `.within(t)` when it must be, into a state machine that each
  activation runs from one wait point to the next, with no future and no executor. The POU names
  its sequence (`sequence = "run"`) and keeps where it waits and its locals in its last variable,
  `_seq`. A crate's unit tests run a sequence activation by activation with `trust_plc::test`; a
  project's build refuses a POU that runs a sequence until its generated declarations declare the
  state.
