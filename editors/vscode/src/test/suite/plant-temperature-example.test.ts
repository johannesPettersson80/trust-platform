import * as assert from "assert";
import {
  createPlantEditorDraft, plantCandidateFromEditorDraft,
  proposePlantStartingPoint, type PlantEditorDraft,
} from "../../networkCanvas/plant/editorModel";

suite("Plant temperature example", () => {
  test("proposes explicit absolute-state FOPDT teaching values without changing the generic model seed", () => {
    assert.deepStrictEqual(proposePlantStartingPoint("temperature_example"), {
      model: "first_order_dead_time",
      parameters: {
        gain: 1, time_constant_s: 30, dead_time_s: 2,
        input_bias: 0, output_bias: 20, initial_delayed_input: 0,
      },
      initialState: [20],
      actuator: { minimum: 0, maximum: 100, initial_applied: 0 },
    });
    const generic = proposePlantStartingPoint("temperature")!;
    assert.strictEqual(generic.parameters.time_constant_s, 10);
    assert.deepStrictEqual(generic.initialState, [0]);
  });

  test("maps physical fields into the same draft while preserving unrelated configuration", () => {
    const api = require("../../networkCanvas/plant/temperatureExample") as {
      applyTemperatureExample?: (draft: PlantEditorDraft) => PlantEditorDraft;
      temperatureExampleValues?: (draft: PlantEditorDraft) => Record<string, number>;
      updateTemperatureExampleField?: (draft: PlantEditorDraft, field: string, value: number) => PlantEditorDraft;
    };
    const original: PlantEditorDraft = {
      ...createPlantEditorDraft({ runtimeId: "r1", runtimeName: "Lab", existingPlantIds: [] }),
      displayName: "My process", commandSources: ["%QL0"], feedbackTargets: ["%IL0"],
      inputScale: [0.1], outputOffset: [7], initialInput: [25],
      actuator: { lag_s: 5 },
      preservedConfiguration: { validity: { state_minimum: [-50] } },
    };
    const before = structuredClone(original);
    const applied = api.applyTemperatureExample?.(original);
    assert.ok(applied, "temperature example must update the canonical draft");
    assert.deepStrictEqual(original, before, "proposal must not mutate the original");
    assert.strictEqual(applied.displayName, "My process");
    assert.deepStrictEqual(applied.preservedConfiguration, original.preservedConfiguration);
    assert.deepStrictEqual(applied.commandSources, ["%QL0"]);
    assert.deepStrictEqual(applied.feedbackTargets, ["%IL0"]);
    assert.deepStrictEqual(applied.initialInput, [0]);
    assert.deepStrictEqual(applied.inputScale, [1]);
    assert.deepStrictEqual(applied.outputOffset, [0]);
    assert.deepStrictEqual(api.temperatureExampleValues?.(applied), {
      resting_temperature: 20, full_power_rise: 100, time_constant_s: 30, dead_time_s: 2,
    });
    const cooling = api.updateTemperatureExampleField?.(applied, "full_power_rise", -200);
    assert.ok(cooling);
    assert.strictEqual(cooling.parameters.gain, -2, "full-power change is divided by 100 percentage points");
    assert.deepStrictEqual(cooling.initialState, [20], "changing gain does not reset the start");
    const resting = api.updateTemperatureExampleField?.(cooling, "resting_temperature", 25);
    assert.ok(resting);
    assert.strictEqual(resting.parameters.output_bias, 25);
    const biased = { ...applied, parameters: { ...applied.parameters, input_bias: 4, output_bias: 30, gain: 2 } };
    assert.strictEqual(api.temperatureExampleValues?.(biased).resting_temperature, 22);
    const changedGain = api.updateTemperatureExampleField?.(biased, "full_power_rise", 300);
    assert.ok(changedGain);
    assert.strictEqual(changedGain.parameters.output_bias, 34, "changing rise retains zero-power temperature even with a custom input bias");
    const candidate = plantCandidateFromEditorDraft(resting);
    assert.deepStrictEqual(candidate.initial_state, [20]);
    assert.deepStrictEqual(candidate.validity, original.preservedConfiguration?.validity);
    assert.ok(!("startingPoint" in candidate), "UI presentation identity is not a runtime parameter");
  });
});
