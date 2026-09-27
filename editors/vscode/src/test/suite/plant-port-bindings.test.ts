import * as assert from "assert";
import { createPlantEditorDraft, type PlantEditorDraft } from "../../networkCanvas/plant/editorModel";
import type { PlantSignalCatalog, PlantSignalDirection } from "../../networkCanvas/plant/signalCatalog";

type PortIssue = { direction: PlantSignalDirection; index: number; message: string };
const api = require("../../networkCanvas/plant/editorModel") as {
  plantPortLabels?: (draft: PlantEditorDraft) => { commands: string[]; feedbacks: string[] };
  validatePlantPortBindings?: (draft: PlantEditorDraft, catalog: PlantSignalCatalog, used: readonly string[]) => PortIssue[];
  updatePlantPortBinding?: (draft: PlantEditorDraft, direction: PlantSignalDirection, index: number, address: string) => PlantEditorDraft;
};

const catalog: PlantSignalCatalog = {
  status: "ready",
  commands: [0, 8].map((offset) => ({ symbol: `P1.Command${offset}`, address: `%QL${offset}`, dataType: "LREAL" })),
  feedbacks: [0, 8, 16].map((offset) => ({ symbol: `P1.Feedback${offset}`, address: `%IL${offset}`, dataType: "LREAL" })),
};
const base = () => createPlantEditorDraft({ runtimeId: "r1", runtimeName: "Lab", existingPlantIds: [] });

suite("Plant port bindings", () => {
  test("requires the robot's torque input and both independent feedback outputs", () => {
    const robot: PlantEditorDraft = {
      ...base(), model: "robot_joint", initialState: [0.5, 0],
      commandSources: ["%QL0"], feedbackTargets: ["%IL0"],
    };
    assert.deepStrictEqual(api.plantPortLabels?.(robot), {
      commands: ["Torque command (N·m)"],
      feedbacks: ["Joint angle (rad)", "Angular speed (rad/s)"],
    });
    const missing = api.validatePlantPortBindings?.(robot, catalog, []);
    assert.ok(missing?.some((issue) => issue.direction === "feedback" && issue.index === 1));
    const complete = { ...robot, feedbackTargets: ["%IL0", "%IL8"] };
    assert.deepStrictEqual(api.validatePlantPortBindings?.(complete, catalog, []), []);
    assert.ok(api.validatePlantPortBindings?.({ ...complete, feedbackTargets: ["%IL0", "%IL0"] }, catalog, [])?.length,
      "two output channels cannot own the same PLC input");
    assert.ok(api.validatePlantPortBindings?.(complete, catalog, ["%IL8"])?.some((issue) => issue.index === 1));
  });

  test("edits one MIMO binding without losing channel order, matrices, or engineering values", () => {
    const original: PlantEditorDraft = {
      ...base(), model: "state_space", startingPoint: "custom_state_space",
      parameters: { a: [[-1, 0], [0, -2]], b: [[1, 0], [0, 1]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]] },
      initialState: [10, 20], commandSources: ["%QL0", "%QL8"], feedbackTargets: ["%IL0", "%IL8"],
      initialInput: [3, 4], inputScale: [0.1, 0.2], inputOffset: [5, 6], outputScale: [10, 20], outputOffset: [7, 8],
    };
    const before = structuredClone(original);
    assert.deepStrictEqual(api.plantPortLabels?.(original), { commands: ["Input 1", "Input 2"], feedbacks: ["Output 1", "Output 2"] });
    assert.deepStrictEqual(api.validatePlantPortBindings?.(original, catalog, []), []);
    const edited = api.updatePlantPortBinding?.(original, "feedback", 1, "%IL16");
    assert.ok(edited);
    assert.deepStrictEqual(original, before);
    assert.deepStrictEqual(edited.feedbackTargets, ["%IL0", "%IL16"]);
    assert.deepStrictEqual({ ...edited, feedbackTargets: before.feedbackTargets, dirty: before.dirty }, before);
    const unbound = api.updatePlantPortBinding?.(edited, "command", 0, "");
    assert.ok(unbound);
    assert.deepStrictEqual(unbound.commandSources, ["", "%QL8"], "clearing a port must not shift other channels");
    assert.ok(api.validatePlantPortBindings?.(unbound, catalog, [])?.some((issue) => issue.direction === "command" && issue.index === 0));
  });
});
