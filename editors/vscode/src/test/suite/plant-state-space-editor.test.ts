import * as assert from "assert";
import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";
import { decodePlantContractResponse, type PlantContractMessage } from "../../networkCanvas/plant/contract";
import { plantCandidateFromEditorDraft, plantDynamicsFields, validatePlantDynamics, type PlantParameters } from "../../networkCanvas/plant/editorModel";
import { normalizePlantTopology } from "../../networkCanvas/plant/projection";
import { planStateSpaceResize, resizeStateSpace } from "../../networkCanvas/plant/stateSpaceEditor";

const parameters = (delays?: number[]): PlantParameters => ({
  a: [[-0.2]], b: [[0.04, -0.02]], c: [[1]], d: [[0, 0]],
  ...(delays === undefined ? {} : { input_delays_s: delays }),
});

function savedResponse(delays?: number[], projectStep?: number): PlantContractMessage {
  const plant = {
    id: "heater", name: "Delayed heater", description: "", enabled: true,
    model: "state_space", parameters: parameters(delays), initial_state: [0],
    command_sources: ["%QL0", "%QL8"], feedback_targets: ["%IL0"],
    initial_input: [0, 0], input_scale: [1, 1], input_offset: [0, 0],
    output_scale: [1], output_offset: [20],
  };
  return {
    contract: "trust.plant", schema_version: 1, operation: "plant.read",
    project_id: "project:sha256:input-delays", configuration_revision: "sha256:saved",
    restart_required: false,
    saved_configuration: {
      simulation: projectStep === undefined ? {} : { plant_step_ms: projectStep },
      plants: [plant],
    },
    plants: [{ id: plant.id, display_name: plant.name, description: "", model: plant.model,
      enabled: true, command_sources: plant.command_sources, feedback_targets: plant.feedback_targets, saved_order: 0 }],
  };
}

function draftFrom(response: PlantContractMessage) {
  const plant = normalizePlantTopology(response)[0];
  assert.ok(plant.configuration);
  return createPlantEditDraft({
    ...plant.configuration, id: "plant:heater", projectId: response.project_id,
    runtimeId: "simulator", runtimeName: "Simulator", plantId: plant.id,
    displayName: plant.displayName, enabled: plant.enabled, model: "state_space",
    commandSources: plant.commandSources, feedbackTargets: plant.feedbackTargets,
    uncertainties: undefined, scenarioEvents: [], savedConfigurationRevision: "sha256:saved",
  });
}

suite("Plant input delays", () => {
  test("declares the optional continuous input-delay parameter", () => {
    const field = plantDynamicsFields("state_space").find((entry) => entry.key === "input_delays_s");
    assert.ok(field, "the existing dynamics contract must expose independent input delays");
    assert.strictEqual(field.optional, true);
    assert.strictEqual(field.valueKind, "vector");
    assert.strictEqual(field.unit, "s");
    assert.ok(!plantDynamicsFields("first_order").some((entry) => entry.key === "input_delays_s"));
  });

  test("preserves explicit and omitted delays through edit save and reopen without aliases", () => {
    for (const delays of [undefined, [0, 0], [1, 0.25]]) {
      const response = savedResponse(delays, 100);
      assert.strictEqual(decodePlantContractResponse(response).kind, "accepted");
      const draft = draftFrom(response);
      draft.displayName = "Edited heater";
      const candidate = plantCandidateFromEditorDraft(draft);
      const savedParameters = candidate.parameters as PlantParameters;
      assert.deepStrictEqual(savedParameters.input_delays_s, delays);
      const reopenedResponse = { ...response, saved_configuration: { simulation: { plant_step_ms: 100 }, plants: [candidate] } };
      assert.deepStrictEqual(draftFrom(reopenedResponse).parameters.input_delays_s, delays);
      assert.ok(!("projectStepMs" in candidate), "editor timing metadata must not enter saved plant configuration");
      if (delays) {
        const expected = [...delays];
        (savedParameters.input_delays_s as number[])[0] = 99;
        const original = (response.saved_configuration as any).plants[0].parameters.input_delays_s;
        assert.deepStrictEqual(original, expected, "candidate changes cannot mutate the confirmed source record");
        assert.deepStrictEqual(draft.parameters.input_delays_s, expected, "candidate changes cannot mutate the editable draft");
      }
    }
  });

  test("rejects negative nonfinite wrong-shaped and wrong-length delays", () => {
    for (const delays of [[-1, 0], [Number.NaN, 0], [Infinity, 0], [1], [], [[1], [0]]]) {
      const draft = draftFrom(savedResponse([0, 0], 100));
      draft.parameters = { ...draft.parameters, input_delays_s: delays as any };
      assert.ok(validatePlantDynamics(draft).some((issue) => issue.field.startsWith("input_delays_s")),
        "invalid delay vector must have a field-owned validation error");
    }
    const draft = draftFrom(savedResponse([0, 0], 100));
    assert.ok(validatePlantDynamics({ ...draft, model: "first_order", parameters: { time_constant_s: 5, input_delays_s: [1] } })
      .some((issue) => issue.field.startsWith("input_delays_s")), "other models cannot silently carry continuous state-space delays");
  });

  test("uses project timing and plant overrides for the native aggregate history bound", () => {
    const draft = draftFrom(savedResponse([5000, 5000], 100));
    assert.strictEqual((draft as any).projectStepMs, 100, "carry the known project setting into the editor");
    assert.ok(validatePlantDynamics(draft).some((issue) => issue.field === "input_delays_s"), "100006 entries exceed the native bound");
    draft.parameters.input_delays_s = [4999.7, 4999.7];
    assert.ok(!validatePlantDynamics(draft).some((issue) => issue.field.startsWith("input_delays_s")), "100000 entries fit");
    draft.parameters.input_delays_s = [100, 0];
    draft.stepMs = 1;
    assert.ok(validatePlantDynamics(draft).some((issue) => issue.field === "input_delays_s"), "the plant override determines effective resolution");
    delete draft.stepMs;
    assert.ok(!validatePlantDynamics(draft).some((issue) => issue.field.startsWith("input_delays_s")));
    assert.strictEqual((draftFrom(savedResponse([1, 0])) as any).projectStepMs, 10, "a read project without an override has the native10ms default");
  });

  test("aligns explicit delays when inputs grow and preserves omission and state-only edits", () => {
    const original = parameters([0.25, 1]);
    const grown = resizeStateSpace({ parameters: original, initialState: [0], requested: { states: 1, inputs: 3, outputs: 1 } });
    assert.deepStrictEqual(grown.parameters.input_delays_s, [0.25, 1, 0]);
    assert.deepStrictEqual(original.input_delays_s, [0.25, 1]);
    const stateEdit = resizeStateSpace({ parameters: original, initialState: [0], requested: { states: 3, inputs: 2, outputs: 1 } });
    assert.deepStrictEqual(stateEdit.parameters.input_delays_s, [0.25, 1]);
    const omitted = resizeStateSpace({ parameters: parameters(), initialState: [0], requested: { states: 2, inputs: 3, outputs: 1 } });
    assert.ok(!("input_delays_s" in omitted.parameters), "dimension changes must not materialize an omitted zero default");
  });

  test("names nonzero delay removal in the existing destructive resize confirmation", () => {
    const original = parameters([0.25, 1]);
    const plan = planStateSpaceResize({ parameters: original, initialState: [0],
      dimensions: { states: 1, inputs: 2, outputs: 1 }, requested: { states: 1, inputs: 1, outputs: 1 } });
    assert.strictEqual(plan.kind, "confirm");
    if (plan.kind === "confirm") assert.match(plan.message, /nonzero.*delay|delay.*nonzero/i);
    assert.deepStrictEqual(plan.parameters.input_delays_s, [0.25]);
    assert.deepStrictEqual(original.input_delays_s, [0.25, 1], "planning or cancelling cannot mutate the draft");
  });
});

suite("Plant state-space editor", () => {
  test("owns dimensions, exact cells, paste, and confirmed destructive resizing", () => {
    let api: any;
    try { api = require("../../networkCanvas/plant/stateSpaceEditor"); } catch { api = undefined; }
    const oneState = { a: [[-1]], b: [[2]], c: [[3]], d: [[4]] };
    assert.deepStrictEqual(api?.stateSpaceDimensions?.(oneState), { states: 1, inputs: 1, outputs: 1 });
    assert.deepStrictEqual(api?.parseStateSpacePaste?.("1\t2\n3\t4"), [[1, 2], [3, 4]]);
    assert.deepStrictEqual(api?.validateStateSpace?.({
      parameters: { a: [[0, Number.NaN], [0, 0]], b: [[1], [0]], c: [[1, 0]], d: [[0]] },
      initialState: [0, 0],
      dimensions: { states: 2, inputs: 1, outputs: 1 },
    }), [
      { matrix: "a", row: 0, column: 1, message: "A[1,2] must be a finite number." },
    ]);
    const grown = api?.planStateSpaceResize?.({
      parameters: oneState,
      initialState: [5],
      dimensions: { states: 1, inputs: 1, outputs: 1 },
      requested: { states: 2, inputs: 1, outputs: 1 },
    });
    assert.deepStrictEqual(grown, {
      kind: "apply",
      parameters: { a: [[-1, 0], [0, 0]], b: [[2], [0]], c: [[3, 0]], d: [[4]] },
      initialState: [5, 0],
    });
    assert.deepStrictEqual(api?.planStateSpaceResize?.({
      parameters: grown.parameters,
      initialState: grown.initialState,
      dimensions: { states: 2, inputs: 1, outputs: 1 },
      requested: { states: 1, inputs: 1, outputs: 1 },
    })?.kind, "confirm");
  });
});
