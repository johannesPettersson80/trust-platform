/// <reference lib="dom" />
import * as assert from "assert";

suite("Control Studio shared workflow layout", () => {
  test("preserves supported controller choices and rejects unknown saved choices", () => {
    const { decodeEngineerStudy } = require("../../controlStudio/engineerStudyPersistence");
    for (const controllerChoice of ["automatic", "P", "PI", "PD", "PID"]) {
      assert.strictEqual(decodeEngineerStudy({ campaigns: [], controllerChoice })?.controllerChoice, controllerChoice);
    }
    assert.strictEqual(decodeEngineerStudy({ campaigns: [], controllerChoice: "unknown" }), undefined);
    const legacy = decodeEngineerStudy({ campaigns: [], family: "PI" });
    const { restoredEngineerView } = require("../../controlStudio/restoredEngineerView");
    assert.strictEqual(restoredEngineerView(legacy).controllerChoice, "PI");
  });

  test("describes the selected model without exposing internal identifiers", () => {
    const api = require("../../controlStudio/engineerWorkflowPresentation");
    assert.strictEqual(typeof api.modelReadyReason, "function");
    assert.strictEqual(api.modelReadyReason({ source: "provided", revision: "sha256:internal" }, true),
      "Model ready. Recommend P control first; PI, PD and PID are available if the task needs them.");
    assert.strictEqual(api.modelReadyReason({ source: "fitted", revision: "sha256:internal" }, false),
      "Model found and validated. Tune the P controller for your requirements.");
  });
});
