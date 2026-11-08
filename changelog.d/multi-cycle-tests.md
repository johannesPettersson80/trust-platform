### Added

- A Structured Text test may run for many cycles and end itself with `TEST_FINISHED()`, as a PLC
  program runs (specification 22, "Multi-cycle tests"). Its cycles run in simulated time from zero,
  10 ms apart unless `{attribute 'test_cycle' := 'T#…'}` names another cycle, within a budget of
  10,000 cycles unless `{attribute 'test_max_cycles' := '…'}` names another. A test that has not
  finished when its budget runs out fails, so none can hang, and a failed assertion names its cycle
  and time: `ASSERT_TRUE expected TRUE, got FALSE (cycle 3, t = T#200ms)`. The JSON report gives
  such a test `cycles` and `simulated_time_ms`. The attributes work in a CODESYS project too, beside
  `{attribute 'test'}`.
