import * as assert from "assert";

suite("Control Studio plant handoff", () => {
  test("accepts one versioned identity-only Simulation handoff and fails closed", () => {
    let api: any;
    try { api = require("../../controlStudio/handoff"); } catch { api = undefined; }

    const handoff = {
      schemaVersion: 1,
      handoffId: "handoff-1",
      projectId: "project:sha256:abc",
      runtimeId: "runtime:local",
      runtimeControlId: "control:local",
      plantId: "tank",
      savedConfigurationRevision: "sha256:saved",
      activeConfigurationDigest: "sha256:active",
      loopId: "loop:tank",
      source: "Simulation",
      requestedContext: { mode: "Monitor", selection: "plant_summary" },
    };

    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(handoff, {
      projectId: "project:sha256:abc",
      runtimeIds: ["runtime:local"],
      plantIds: ["tank"],
    }), { kind: "accepted", handoff });
    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(
      { ...handoff, schemaVersion: 2 },
      { projectId: handoff.projectId, runtimeIds: [handoff.runtimeId], plantIds: [handoff.plantId] }
    ), { kind: "rejected", reason: "unsupported_version" });
    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(
      { ...handoff, projectId: "project:sha256:other" },
      { projectId: handoff.projectId, runtimeIds: [handoff.runtimeId], plantIds: [handoff.plantId] }
    ), { kind: "rejected", reason: "wrong_project" });
    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(
      { ...handoff, runtimeId: "runtime:unknown" },
      { projectId: handoff.projectId, runtimeIds: [handoff.runtimeId], plantIds: [handoff.plantId] }
    ), { kind: "rejected", reason: "unknown_runtime" });
    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(
      { ...handoff, plantId: "deleted" },
      { projectId: handoff.projectId, runtimeIds: [handoff.runtimeId], plantIds: [handoff.plantId] }
    ), { kind: "rejected", reason: "unknown_plant" });
    assert.deepStrictEqual(api?.decodePlantControlStudioHandoff?.(
      { ...handoff, measuredFeedback: [1.2] },
      { projectId: handoff.projectId, runtimeIds: [handoff.runtimeId], plantIds: [handoff.plantId] }
    ), { kind: "rejected", reason: "malformed_handoff" });
  });
});
