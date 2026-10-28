import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

/** The course's trajectory-feedforward loop (lesson 90): a PID whose program adds K_v·ṙ + K_a·r̈ to its own PID (Specification 35). */
function trajectoryLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/trajectory-control-loop.json"), "utf8"));
}

suite("Control Studio trajectory feedforward", () => {

  test("names the PID's gains and the two feedforward gains on Tune", () => {
    const { JSDOM } = require("jsdom");
    const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const root = dom.window.document.querySelector("main");
      renderTunedController(root, trajectoryLoop(), { status: "candidate", family: "PID",
        values: { "P1.Parameters.ProportionalGain": 18, "P1.Parameters.IntegralGain": 12, "P1.Parameters.DerivativeGain": 7, "P1.RateGain": 1, "P1.AccelerationGain": 1 },
        // The engine's words of the answer (TuneWords): the setting rows and their labels.
        words: { outcome: "Meets the requirements", settingLabels: [{ symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)" },
          { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)" }, { symbol: "P1.Parameters.DerivativeGain", label: "Derivative gain (Kd)" },
          { symbol: "P1.RateGain", label: "Rate feedforward (Kv)" }, { symbol: "P1.AccelerationGain", label: "Acceleration feedforward (Ka)" }] } });
      assert.deepStrictEqual(Array.from(root.querySelectorAll("dt")).map((term: any) => term.textContent), ["Proportional gain (Kp)",
        "Integral gain (Ki)", "Derivative gain (Kd)", "Rate feedforward (Kv)", "Acceleration feedforward (Ka)"]);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });

});
