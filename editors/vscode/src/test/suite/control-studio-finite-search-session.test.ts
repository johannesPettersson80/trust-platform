import * as fs from "fs";
import * as path from "path";
import * as assert from "assert";

suite("Control Studio finite search session", () => {
  test("exposes selected values only for complete compatible bindings and the same source", async () => {
    const { FiniteSearchSession } = require("../../controlStudio/finiteSearchSession");
    let revision = "source";
    const session = new FiniteSearchSession(async () => ({ source: "simulation", request: async () => ({ ready: true, project_revision: revision }) }));
    const run = { id: "run", evidence: "ST-executed", parameters: { "Main.Kp": 2 },
      conditions: { initialState: {}, events: [], durationSeconds: 1, sampleTimesSeconds: [0,1], seed: 1,
        modelRevision: "model", programRevision: "program", parameterSchemaRevision: "parameters" }, channels: [{ id: "y", unit: "1" }],
      samples: [0,1].map(sequence => ({ sequence, timeSeconds: sequence, values: { y: { value: 1, quality: "good" } } })) };
    session.restore({ origin: "runtime-isolated-simulation", selectedIndex: 0,
      request: { execution: "isolated-simulation", expected_project_revision: "source", trial_count: 1,
        grid: [{ symbol: "Main.Kp", unit: "1", minimum: 0, maximum: 10, values: [2] }], initial_values: {} },
      result: { job_id: "job", project_revision: "source", status: "completed", trial_count: 1,
        trials: [{ index: 0, parameters: { "Main.Kp": 2 }, status: "qualified", score: 1, run }], ranking: [0], unattempted_trials: [],
        source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" } });
    const binding = { loop_id: "loop", task_name: "Task", mode: { symbol: "Main.Auto", manual_value: false },
      parameters: [{ symbol: "Main.Kp", iec_type: "LREAL", unit: "1", convention: "gain", minimum: 0, maximum: 10 }] };
    assert.deepStrictEqual(await session.selectedParameterValues?.(binding), { "Main.Kp": 2 });
    await assert.rejects(() => session.selectedParameterValues({ ...binding, parameters: [...binding.parameters, { ...binding.parameters[0], symbol: "Main.Ki" }] }), /complete|missing/i);
    revision = "changed";
    await assert.rejects(() => session.selectedParameterValues(binding), /source|changed/i);
  });

  test("prepares a general declared grid with fixed values and explicit isolated initial mode", () => {
    let api: any;
    try { api = require("../../controlStudio/finiteSearchDraft"); } catch { api = undefined; }
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(root, "examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"), "utf8"));
    const draft = api?.buildFiniteSearchDraft(loop, { "P1.Kp": 16, "P1.Kd": 7 });
    assert.deepStrictEqual(draft?.grid, [
      { symbol: "P1.Kp", unit: "N·m/rad", minimum: 0, maximum: 20, values: [12, 16, 20] },
      { symbol: "P1.Kd", unit: "N·m·s/rad", minimum: 7, maximum: 7, values: [7] },
    ], "only the declared selected parameter varies; fixed values remain explicit");
    assert.strictEqual(draft.trial_count, 3);
    assert.deepStrictEqual(draft.initial_values, { "P1.ExperimentEnabled": 1 });
    assert.deepStrictEqual(draft.objective, { reference_symbol: "P1.Setpoint", feedback_symbol: "P1.Angle", error_unit: "rad" });
    assert.strictEqual(draft.step_ms, 20);
    assert.strictEqual(draft.duration_ms, 8100);
  });
});
