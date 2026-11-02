import * as assert from "assert";
import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";

// Rust builds the candidate from the draft and carries the saved fields the editor does not manage
// (specification 47 §13.2). Rust's `plant.topology` hands the editor the saved table (trust-runtime
// test `a_saved_plant_opens_with_its_whole_saved_table`); the draft keeps a copy of it.
suite("Plant configuration preservation", () => {
  test("a saved plant reopens with its whole saved table, copied, for Rust to carry", () => {
    const validity = { state_minimum: [-10, -20], state_maximum: [10, 20] };
    const saved = {
      id: "custom", name: "Custom model", description: "Existing model", enabled: true,
      model: "state_space", parameters: { a: [[-1, 0], [0, -2]], b: [[1, 0], [0, 1]],
        c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]] },
      initial_state: [1, 2], initial_input: [0, 0],
      command_sources: ["%QL0", "%QL8"], feedback_targets: ["%IL0", "%IL8"],
      input_scale: [1, 2], input_offset: [0, 3], output_scale: [4, 5], output_offset: [6, 7],
      validity,
    };
    const configuration = {
      description: saved.description,
      parameters: saved.parameters,
      initialState: saved.initial_state,
      initialInput: saved.initial_input,
      inputScale: saved.input_scale,
      inputOffset: saved.input_offset,
      outputScale: saved.output_scale,
      outputOffset: saved.output_offset,
      preservedConfiguration: saved,
    };
    const draft = createPlantEditDraft({
      ...configuration, id: "plant:custom", projectId: "project:test",
      uncertainties: undefined, scenarioEvents: [],
      runtimeId: "simulator", runtimeName: "Simulator", plantId: saved.id,
      displayName: "Renamed model", enabled: true, model: "state_space",
      commandSources: saved.command_sources, feedbackTargets: saved.feedback_targets,
      savedConfigurationRevision: "sha256:saved",
    });
    assert.deepStrictEqual(draft.preservedConfiguration, saved);
    assert.strictEqual(draft.originalPlantId, "custom", "the checks know the plant's own saved ID");
    assert.strictEqual(draft.startingPoint, "", "Rust opens the draft and names its starting point");
    (draft.preservedConfiguration!.validity as typeof validity).state_minimum[0] = -99;
    assert.strictEqual(validity.state_minimum[0], -10, "editing the draft must not mutate the read response");
  });
});
