import * as assert from "assert";

import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";
import { plantCandidateFromEditorDraft } from "../../networkCanvas/plant/editorModel";
import { plantDynamicsFields } from "../../networkCanvas/plant/editorModel";
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

const expectedState: Record<string, number[]> = {
  static_gain: [], first_order: [0], first_order_dead_time: [0], integrator: [0],
  gravity_tank: [0], piecewise_linear: [], robot_joint: [0, 0], second_order: [0, 0], state_space: [0, 0],
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
  test("defaultable gain and dead time are optional in the shared field descriptor", () => {
    for (const model of ["static_gain", "first_order", "first_order_dead_time", "integrator", "second_order"]) {
      const gain = plantDynamicsFields(model).find(field => field.key === "gain");
      assert.strictEqual(gain?.optional, true, `${model} gain can be cleared to its default`);
      assert.strictEqual(gain?.defaultValue, 1, model);
    }
    const delay = plantDynamicsFields("first_order_dead_time").find(field => field.key === "dead_time_s");
    assert.strictEqual(delay?.optional, true);
    assert.strictEqual(delay?.defaultValue, 0);
  });
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
  test("all nine runtime-valid omitted states reopen as resolved zeros and remain omitted after a label edit", () => {
    for (const model of Object.keys(expectedState)) {
      const projected = normalizePlantTopology(responseFor(model))[0];
      assert.ok(projected?.configuration, `${model} must remain editable when initial_state is omitted`);
      assert.deepStrictEqual(projected.configuration?.initialState, expectedState[model], model);
      const draft = createPlantEditDraft({
        ...projected.configuration!,
        uncertainties: undefined, scenarioEvents: [],
        id: `plant:${model}`, projectId: "project:omitted", runtimeId: "simulator", runtimeName: "Simulator",
        plantId: projected.id, displayName: `Renamed ${model}`, enabled: true, model: model as PlantModelKind,
        commandSources: projected.commandSources, feedbackTargets: projected.feedbackTargets,
        savedConfigurationRevision: "sha256:omitted",
      });
      const candidate = plantCandidateFromEditorDraft(draft);
      assert.strictEqual(candidate.name, `Renamed ${model}`, model);
      assert.strictEqual(Object.prototype.hasOwnProperty.call(candidate, "initial_state"), false,
        `${model} label edit must preserve the omitted initial_state representation`);
      assert.deepStrictEqual(candidate.parameters, scalarParameters[model], model);
      if (expectedState[model].length > 0) {
        const editedState = expectedState[model].map((_, index) => index + 1);
        assert.deepStrictEqual(plantCandidateFromEditorDraft({ ...draft, initialState: editedState }).initial_state,
          editedState, `${model} must persist an explicit nonzero starting-state edit`);
      }
    }
  });
});
