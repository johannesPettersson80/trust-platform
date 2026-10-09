import * as assert from "assert";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function duration(): any {
  try { return require("../../controlStudio/runtimeDuration"); } catch { return {}; }
}

suite("Control Studio runtime-time observation", () => {
  test("measures process time from the runtime's own timestamps, not from this host's clock", () => {
    const api = duration();
    assert.strictEqual(typeof api.RuntimeReadingClock, "function", "Apply measures process time from the runtime's clock");
    const clock = new api.RuntimeReadingClock(() => 0);
    assert.strictEqual(clock.observe({ runtime_time_ns: 0 }), 0, "the first reading starts the process time");
    assert.strictEqual(clock.observe({ runtime_time_ns: 2.5e9 }), 2500, "2.5 s of runtime time is 2500 ms, whatever the wall clock says");
  });

  test("stops with 'The runtime stopped sending values.' when no newer runtime value arrives within the connection-loss timeout", () => {
    const api = duration();
    let wall = 0;
    const clock = new api.RuntimeReadingClock(() => wall);
    clock.observe({ runtime_time_ns: 1e9 });
    wall = 4_000;
    assert.strictEqual(clock.observe({ runtime_time_ns: 1e9 }), 0, "a repeated scan is not a newer value");
    assert.doesNotThrow(() => clock.ensureFresh(), "4 s without a newer value is within the 5 s timeout");
    wall = 6_000;
    assert.throws(() => clock.ensureFresh(), /The runtime stopped sending values\./, "6 s without a newer value ends the wait");
  });

  test("refuses a reading that carries no runtime clock", () => {
    const api = duration();
    const clock = new api.RuntimeReadingClock(() => 0);
    assert.throws(() => clock.observe({}), /does not report its clock/);
  });
});
