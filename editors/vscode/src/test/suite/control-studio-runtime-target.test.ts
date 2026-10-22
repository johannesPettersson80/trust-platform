import * as assert from "assert";
import { runtimeTargetFromStatus } from "../../controlStudio/runtimeTarget";

suite("Control Studio runtime target", () => {
  test("a target is simulated as the runtime's status states it; the client does not derive it from other facts", () => {
    assert.strictEqual(runtimeTargetFromStatus({ simulated: true }).simulated, true);
    assert.strictEqual(runtimeTargetFromStatus({ simulated: false, simulation_enabled: true, simulation_only_io: true }).simulated, false,
      "the runtime decides whether a target is simulated (A2.4); its other facts do not make it simulated here");
    assert.strictEqual(runtimeTargetFromStatus({ simulation_enabled: true, simulation_only_io: true }).simulated, false,
      "a report that does not state simulated is physical");
    assert.strictEqual(runtimeTargetFromStatus(undefined).simulated, false);
  });

  test("reads the resource cycle, time scale and speed words that the runtime reports, and no other fact of its status", () => {
    assert.deepStrictEqual(runtimeTargetFromStatus({ simulated: true, simulation_enabled: true, simulation_time_scale: 10,
      simulation_speed_text: "×10", simulation_speed_behind: false,
      session_epoch: "epoch-7", cycle_interval_ms: 10, tasks: [{ name: "MainTask", interval_ms: 100 }] }), {
      simulated: true, timeScale: 10, speedText: "×10", speedBehind: false, cycleIntervalMs: 10 });
  });
});
