import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

/** The engine's cascade seed on the course's loop shape: the outer PI sets the inner PI's setpoint (specification 35, C6). */
function cascadeLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/cascade-control-loop.json"), "utf8"));
}

suite("Control Studio cascade tuned together", () => {

  test("Tune names each controller's gains and leaves out the unused derivative gains", () => {
    const loop = cascadeLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const values = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.symbol.endsWith("DerivativeGain") ? 0 : 1]));
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "cascade", words: { gainLabels: { proportional: "Proportional gain (Kp)", integral: "Integral gain (Ki)", derivative: "Derivative gain (Kd)" } }, values });
      // The served cascade shows one row for each controller, inner first, with its instance and its P and I (structureResults.ts).
      const table = dom.window.document.querySelector("[data-studio-cascade-design]");
      assert.ok(table, "the cascade's controllers are shown as one table");
      assert.deepStrictEqual([...table.tBodies[0].rows].map((row: any) => row.cells[0].textContent), [
        `Inner · ${loop.controller.project.seed.inner.instance}`, `Outer · ${loop.controller.project.seed.outer.instance}`]);
      assert.strictEqual(dom.window.document.querySelectorAll("dt").length, 0, "the derivative gains are left out");
    } finally { (globalThis as any).document = saved; }
  });

});
