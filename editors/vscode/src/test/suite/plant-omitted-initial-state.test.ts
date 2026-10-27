import * as assert from "assert";

import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";
import type { PlantModelKind } from "../../networkCanvas/plant/editorModel";
import { normalizePlantTopology } from "../../networkCanvas/plant/projection";
import type { PlantContractMessage } from "../../networkCanvas/plant/contract";

const scalarParameters: Record<string, unknown> = {
  static_gain: { gain: 2 },
  first_order: { time_constant_s: 2 },
  first_order_dead_time: { time_constant_s: 2, dead_time_s: 0.5 },
  integrator: { gain: 2 },
  gravity_tank: { area_m2: 2, outflow_coefficient_m2_sqrt_m_per_s: 0.5 },
  piecewise_linear: { input_points: [0, 1], output_points: [0, 1] },
  robot_joint: { inertia_kg_m2: 2, friction_smoothing_rad_s: 0.01 },
  second_order: { natural_frequency_rad_s: 2, damping_ratio: 0.7 },
  state_space: { a: [[0, 1], [-1, -1]], b: [[0], [1]], c: [[1, 0]], d: [[0]] },
};

function responseFor(model: string): PlantContractMessage {
  const saved = {
    id: `omitted-${model}`, name: model, description: "saved", enabled: true, model,
    parameters: scalarParameters[model], command_sources: ["%QL0"], feedback_targets: ["%IL0"],
  };
  if (model === "robot_joint") saved.feedback_targets = ["%IL0", "%IL8"];
  if (model === "state_space") {
    saved.command_sources = ["%QL0"];
    saved.feedback_targets = ["%IL0"];
  }
  return {
    contract: "trust.plant", schema_version: 1, operation: "plant.read", project_id: "project:omitted",
    saved_configuration: { plants: [saved] },
    plants: [{ id: saved.id, display_name: saved.name, model, enabled: true,
      command_sources: saved.command_sources, feedback_targets: saved.feedback_targets }],
  } as PlantContractMessage;
}

suite("Plant omitted initial-state projection", () => {
  test("explicit zero and malformed state are not treated as omission", () => {
    for (const state of [[0], null, []]) {
      const response = responseFor("first_order");
      const saved = (response.saved_configuration as { plants: Record<string, unknown>[] }).plants[0];
      saved.initial_state = state;
      const configuration = normalizePlantTopology(response)[0].configuration;
      if (state === null) {
        assert.strictEqual(configuration, undefined, "null is invalid, not an omitted state");
      } else {
        assert.deepStrictEqual(configuration?.initialState, state, "explicit arrays must not be replaced by defaults");
        assert.notStrictEqual(configuration?.initialStateDefaulted, true);
      }
    }
  });
  // Rust sizes an omitted state and keeps the omission when the editor opens and saves the plant
  // (specification 47 §9); the projection only carries the omission to it.
  test("all nine runtime-valid omitted states reach the editor marked as omitted", () => {
    for (const model of Object.keys(scalarParameters)) {
      const projected = normalizePlantTopology(responseFor(model))[0];
      assert.ok(projected?.configuration, `${model} must remain editable when initial_state is omitted`);
      assert.deepStrictEqual(projected.configuration?.initialState, [], model);
      assert.strictEqual(projected.configuration?.initialStateDefaulted, true, model);
      const draft = createPlantEditDraft({
        ...projected.configuration!,
        uncertainties: undefined, scenarioEvents: [],
        id: `plant:${model}`, projectId: "project:omitted", runtimeId: "simulator", runtimeName: "Simulator",
        plantId: projected.id, displayName: `Renamed ${model}`, enabled: true, model: model as PlantModelKind,
        commandSources: projected.commandSources, feedbackTargets: projected.feedbackTargets,
        savedConfigurationRevision: "sha256:omitted",
      });
      assert.strictEqual(draft.initialStateDefaulted, true, model);
      assert.deepStrictEqual(draft.parameters, scalarParameters[model], model);
    }
  });
});
