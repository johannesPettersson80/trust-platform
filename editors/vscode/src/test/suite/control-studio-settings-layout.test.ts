import * as assert from "assert";
import { engineerWorkflowControls } from "../../controlStudio/engineerWorkflowPresentation";
const { JSDOM } = require("jsdom");
suite("Control Studio task settings", () => {
  test("requires explicit numeric goals instead of silently substituting defaults for blank fields", () => {
    const d = new JSDOM(engineerWorkflowControls("m", true, "V", true)).window.document;
    for (const name of ["reference", "error", "settling", "gain-value"]) {
      const field = d.querySelector(`[data-engineer-${name}]`);
      field.disabled = false; field.value = "";
      assert.strictEqual(field.checkValidity(), false, `${name} must reject a blank value`);
    }
  });
  test("places the response goal outside advanced settings without duplicate inputs", () => {
    const d = new JSDOM(engineerWorkflowControls("m", true, "V", true)).window.document;
    const goals = d.querySelector("[data-engineer-goal-fields]");
    assert.ok(goals);
    assert.strictEqual(goals.querySelectorAll("input").length, 3);
    for (const name of ["reference", "error", "settling"]) assert.strictEqual(d.querySelectorAll(`[data-engineer-${name}]`).length, 1);
    assert.strictEqual(d.querySelector("[data-engineer-options]").contains(goals), false);
    for (const stage of ["model", "tune", "verify", "apply"]) assert.ok(d.querySelector(`[data-engineer-settings-stage=${stage}]`));
    assert.match(d.querySelector("[data-engineer-scenario-controls]").textContent, /Add scenario/);
    assert.ok(d.querySelector("[data-engineer-scenario-advanced]"));
  });
});
