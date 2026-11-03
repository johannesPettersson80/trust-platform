import * as assert from "assert";

import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";

// Rust's `plant.topology` marks an omitted initial state (trust-runtime test
// `only_an_omitted_initial_state_reaches_the_editor_as_omitted`) and Rust's `open` edit sizes it
// (specification 47 §9). Catches the editor draft losing the mark on the way.
suite("Plant omitted initial-state projection", () => {
  test("an omitted initial state reaches the editor draft marked as omitted", () => {
    const configuration = {
      description: "saved",
      parameters: { time_constant_s: 2 },
      initialState: [],
      initialStateDefaulted: true,
    };
    const draft = createPlantEditDraft({
      ...configuration,
      uncertainties: undefined,
      scenarioEvents: [],
      id: "plant:first_order",
      projectId: "project:omitted",
      runtimeId: "simulator",
      runtimeName: "Simulator",
      plantId: "omitted-first_order",
      displayName: "Renamed first order",
      enabled: true,
      model: "first_order",
      commandSources: ["%QL0"],
      feedbackTargets: ["%IL0"],
      savedConfigurationRevision: "sha256:omitted",
    });
    assert.strictEqual(draft.initialStateDefaulted, true);
    assert.deepStrictEqual(draft.initialState, []);
    assert.deepStrictEqual(draft.parameters, { time_constant_s: 2 });
  });
});
