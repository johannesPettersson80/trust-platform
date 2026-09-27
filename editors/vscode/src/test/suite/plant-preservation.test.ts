import * as assert from "assert";
import { normalizePlantTopology } from "../../networkCanvas/plant/projection";
import { createPlantEditDraft } from "../../networkCanvas/plant/addFlow";
import { plantCandidateFromEditorDraft } from "../../networkCanvas/plant/editorModel";
import type { PlantContractMessage } from "../../networkCanvas/plant/contract";

suite("Plant configuration preservation", () => {
  test("saved dynamics require replacement confirmation even before an edit", () => {
    const { planPlantStartingPointChange } = require("../../networkCanvas/plant/editorModel");
    const plan = planPlantStartingPointChange({
      currentStartingPoint: "custom_state_space", requestedStartingPoint: "tank",
      modelValuesTouched: false, hasSavedConfiguration: true,
    });
    assert.strictEqual(plan.kind, "confirm", "saved custom dynamics are not an untouched new proposal");
    assert.match(plan.message, /replace/i);
    assert.strictEqual(planPlantStartingPointChange({
      currentStartingPoint: "tank", requestedStartingPoint: "servo",
      modelValuesTouched: false, hasSavedConfiguration: false,
    }).kind, "apply", "an untouched new proposal can still change directly");
  });
  test("label edits preserve model validity through projection and serialization", () => {
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
    const response: PlantContractMessage = {
      contract: "trust.plant", schema_version: 1, operation: "plant.read", project_id: "project:test",
      configuration_revision: "sha256:saved", saved_configuration: { plants: [saved] },
      plants: [{ id: saved.id, display_name: saved.name, model: saved.model,
        enabled: true, command_sources: saved.command_sources, feedback_targets: saved.feedback_targets }],
    };
    const projected = normalizePlantTopology(response)[0];
    assert.ok(projected.configuration);
    const draft = createPlantEditDraft({
      ...projected.configuration, id: "plant:custom", projectId: "project:test",
      uncertainties: undefined, scenarioEvents: [],
      runtimeId: "simulator", runtimeName: "Simulator", plantId: saved.id,
      displayName: "Renamed model", enabled: true, model: "state_space",
      commandSources: projected.commandSources, feedbackTargets: projected.feedbackTargets,
      savedConfigurationRevision: "sha256:saved",
    });
    const candidate = plantCandidateFromEditorDraft(draft);
    assert.deepStrictEqual(candidate.validity, validity, "a label edit must not remove model validity");
    assert.deepStrictEqual(candidate, { ...saved, name: "Renamed model", scenario_events: [] });
    const cleared = plantCandidateFromEditorDraft({
      ...draft, actuator: undefined,
      preservedConfiguration: { ...draft.preservedConfiguration, actuator: { lag_s: 99 }, name: "Stale name" },
    });
    assert.strictEqual(cleared.actuator, undefined, "saved extras must not resurrect a cleared effect");
    assert.strictEqual(cleared.name, "Renamed model", "editable identity remains authoritative");
    (candidate.validity as typeof validity).state_minimum[0] = -99;
    assert.strictEqual(validity.state_minimum[0], -10, "candidate editing must not mutate the read response");
  });
});
