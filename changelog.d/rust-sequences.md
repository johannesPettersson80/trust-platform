### Added

- Rust sequences (specification 48 §15): `#[trust_sequence]` on a POU's `impl` block turns an
  `async fn` written as straight-line Rust, which waits with `s.next_cycle()`, `s.until(pred)` and
  `s.wait(t)`, each limited by `.within(t)` when it must be, into a state machine that each
  activation runs from one wait point to the next, with no future and no executor. The POU names
  its sequence (`sequence = "run"`) and keeps where it waits and its locals in its last variable,
  `_seq`, which the generated declarations declare as a structure, so the debugger shows it and
  `variables.read` reads `<instance>._seq.point`. A cold or a warm restart starts the sequence at
  its top. A crate's unit tests run a sequence activation by activation with `trust_plc::test`.
- A Rust sequence waits for several things at once (specification 48 RU-72): `s.all(..)` until each
  of its waits has completed, at an activation of its own, as an SFC's simultaneous branches
  converge, and `s.race(..)` until the first of them completes, the earlier one in the list winning
  a tie, as an SFC's selection does. `let won: u16 = s.race(..).await;` tells which wait ended it,
  and the state's `done` shows which waits of an `s.all` have completed.
- A Rust sequence awaits another `async fn` of its block as a child sequence, `self.fill(s).await?`
  (specification 48 RU-72): the child runs as a call would, from the activation that reaches it, so
  moving part of a sequence into a child changes nothing about when anything happens, and it keeps
  a state of its own, which the debugger shows as `<instance>._seq.fill`. A loop whose only wait is
  a child that can return without waiting, or an inner loop that can `break` before its wait, is now
  refused, as it could keep a scan from ending.
- An online change keeps a Rust sequence where it waits when the changed code has the same wait
  points, each named by its step or its order, and the same locals, and otherwise starts the
  sequence at its top at its next activation (specification 48 RU-73). The change's report then
  lists the sequence state's new member `shape_<signature>`, whose name changes with the wait points
  and the locals. A child sequence that changed starts at its top the next time it runs.
