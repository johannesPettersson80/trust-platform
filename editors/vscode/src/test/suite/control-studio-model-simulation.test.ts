import * as assert from "assert";

type Materialize = (model: unknown, binding: unknown) => {
  simulation_override: {
    model_revision: string;
    model_provenance: string;
    simulation_toml: string;
  };
  stateCoordinateDescription: string;
};

function materialize(): Materialize | undefined {
  try {
    return require("../../controlStudio/modelSimulation").materializeSelectedModelSimulation as Materialize;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "MODULE_NOT_FOUND") return undefined;
    throw error;
  }
}

const channel = (id: string, unit: string) => ({
  id,
  unit,
  operatingRange: { minimum: -100, maximum: 100 },
});

const binding = (initialState: readonly number[]) => ({
  plantId: "selected-plant",
  commandSources: ["%QL0"],
  feedbackTargets: ["%IL0"],
  initialState,
  initialInput: [0.25],
  stepMs: 20,
});

suite("Control Studio selected-model simulation materialization", () => {
  test("materializes a delayed integrator with exact rate, input history and operating offsets", () => {
    const plant = { kind: "transfer-function", numerator: [4], denominator: [2, 0], delaySeconds: 0.13,
      revision: "delayed-integrator", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")],
      operatingPoint: { inputs: [2], outputs: [10] } };
    let result: ReturnType<Materialize> | undefined;
    assert.doesNotThrow(() => { result = materialize()!(plant, { ...binding([0.5]), initialDelayedInput: 0.25 }); });
    const toml = result!.simulation_override.simulation_toml;
    assert.match(toml, /model = "integrator_dead_time"/);
    assert.match(toml, /gain = 2/);
    assert.match(toml, /dead_time_s = 0.13/);
    assert.match(toml, /initial_delayed_input = 0.25/);
    assert.match(toml, /initial_state = \[1\]/, "native output state equals C*x = (4/2)*0.5");
    assert.match(toml, /input_offset = \[-2\]/);
    assert.match(toml, /output_offset = \[10\]/);
    assert.ok(!toml.includes("time_constant_s"));
    assert.throws(() => materialize()!(plant, binding([0])), /history|delayed|declared/i);
  });

  test("keeps every delay-free transfer-function mode and zero in controllable companion coordinates", () => {
    const materializeSelectedModelSimulation = materialize();
    assert.ok(
      materializeSelectedModelSimulation,
      "selected models must materialize an inline simulation override before Verify"
    );
    const materialized = materializeSelectedModelSimulation!({
      revision: "sha256:tf-with-zero",
      source: "provided",
      validation: "unvalidated",
      evidence: [],
      delaySeconds: 0,
      kind: "transfer-function",
      inputs: [channel("command", "V")],
      outputs: [channel("position", "m")],
      numerator: [1, 1],
      denominator: [1, 3, 2],
    }, binding([0.25, -0.5]));

    assert.strictEqual(materialized.simulation_override.model_revision, "sha256:tf-with-zero");
    assert.strictEqual(materialized.simulation_override.model_provenance, "declared-provided-unvalidated");
    assert.match(materialized.stateCoordinateDescription, /controllable companion/i);
    assert.match(materialized.stateCoordinateDescription, /not physical measured states/i);
    assert.match(materialized.simulation_override.simulation_toml, /model = "state_space"/);
    assert.match(materialized.simulation_override.simulation_toml, /command_sources = \["%QL0"\]/);
    assert.match(materialized.simulation_override.simulation_toml, /feedback_targets = \["%IL0"\]/);
    assert.match(materialized.simulation_override.simulation_toml, /initial_state = \[0.25, -0.5\]/);
    assert.match(materialized.simulation_override.simulation_toml, /initial_input = \[0.25\]/);
    assert.match(materialized.simulation_override.simulation_toml, /a = \[\[-3, -2\], \[1, 0\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /b = \[\[1\], \[0\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /c = \[\[1, 1\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /d = \[\[0\]\]/);
  });

  test("keeps supplied continuous state coordinates and ordered MIMO channels intact", () => {
    const materializeSelectedModelSimulation = materialize();
    assert.ok(materializeSelectedModelSimulation, "selected models must materialize before Verify");
    const materialized = materializeSelectedModelSimulation!({
      revision: "sha256:supplied-state-space",
      source: "fitted",
      validation: "validated",
      evidence: ["independent-record"],
      delaySeconds: 0,
      kind: "continuous-state-space",
      states: [channel("stored-energy", "J"), channel("flow-mode", "m3/s")],
      inputs: [channel("heater", "W"), channel("valve", "%")],
      outputs: [channel("temperature", "degC"), channel("level", "m")],
      a: [[-2, 1], [0, -3]],
      b: [[1, 2], [3, 4]],
      c: [[5, 6], [7, 8]],
      d: [[0, 9], [10, 0]],
    }, {
      plantId: "mimo-plant",
      commandSources: ["%QL0", "%QL8"],
      feedbackTargets: ["%IL0", "%IL8"],
      initialState: [11, 12],
      initialInput: [13, 14],
      stepMs: 50,
    });

    assert.match(materialized.stateCoordinateDescription, /stored-energy, flow-mode/);
    assert.match(materialized.simulation_override.simulation_toml, /command_sources = \["%QL0", "%QL8"\]/);
    assert.match(materialized.simulation_override.simulation_toml, /feedback_targets = \["%IL0", "%IL8"\]/);
    assert.match(materialized.simulation_override.simulation_toml, /initial_state = \[11, 12\]/);
    assert.match(materialized.simulation_override.simulation_toml, /initial_input = \[13, 14\]/);
    assert.match(materialized.simulation_override.simulation_toml, /a = \[\[-2, 1\], \[0, -3\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /b = \[\[1, 2\], \[3, 4\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /c = \[\[5, 6\], \[7, 8\]\]/);
    assert.match(materialized.simulation_override.simulation_toml, /d = \[\[0, 9\], \[10, 0\]\]/);
  });

  test("uses exact delay adapters and rejects mismatched discrete timing", () => {
    const materializeSelectedModelSimulation = materialize();
    assert.ok(materializeSelectedModelSimulation, "selected models must materialize before Verify");
    const delayed = materializeSelectedModelSimulation!({
      revision: "sha256:fopdt",
      source: "fitted",
      validation: "validated",
      evidence: ["independent-record"],
      delaySeconds: 1.5,
      kind: "transfer-function",
      inputs: [channel("command", "V")],
      outputs: [channel("temperature", "degC")],
      numerator: [4],
      denominator: [2, 1],
    }, { ...binding([18]), initialDelayedInput: 0.125 });
    assert.match(delayed.simulation_override.simulation_toml, /model = "first_order_dead_time"/);
    assert.match(delayed.simulation_override.simulation_toml, /dead_time_s = 1.5/);
    assert.match(delayed.simulation_override.simulation_toml, /initial_delayed_input = 0.125/);
    assert.throws(() => materializeSelectedModelSimulation!({
      revision: "sha256:discrete",
      source: "provided",
      validation: "unvalidated",
      evidence: [],
      delaySeconds: 0,
      kind: "discrete-state-space",
      sampleTimeSeconds: 0.1,
      states: [channel("x", "1")],
      inputs: [channel("command", "V")],
      outputs: [channel("output", "V")],
      a: [[0.5]], b: [[1]], c: [[1]], d: [[0]],
    }, binding([0])), /discrete.*sample time/i);
    const delayedState = materializeSelectedModelSimulation!({
      revision: "sha256:delayed-state-space",
      source: "provided",
      validation: "unvalidated",
      evidence: [],
      delaySeconds: 0.1,
      kind: "continuous-state-space",
      states: [channel("x", "1")],
      inputs: [channel("command", "V")],
      outputs: [channel("output", "V")],
      a: [[-1]], b: [[1]], c: [[1]], d: [[0]],
    }, binding([0]));
    assert.match(delayedState.simulation_override.simulation_toml, /model = "discrete_state_space"/);
    assert.match(delayedState.stateCoordinateDescription, /input-history.*Exact held-input delay/i);
  });
});
