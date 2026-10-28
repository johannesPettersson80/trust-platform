import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

/** The course's state-feedback loop (lesson 68): the two tanks' state gains K1 and K2 and the reference gain N. */
function stateFeedbackLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/state-feedback-control-loop.json"), "utf8"));
}

suite("Control Studio state-feedback setup", () => {

  test("shows each designed gain with the exact symbol it belongs to", () => {
    const loop = stateFeedbackLoop();
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<main><div data-studio-tuning-result hidden></div></main>");
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const root = dom.window.document.querySelector("main");
      renderTunedController(root, loop, { status: "candidate", family: "LQR", values: { "P1.K2": 0.0106, "P1.K1": 0.0291, "P1.N": 0.0598 },
        words: { outcome: "No prediction: The study keeps no prediction.", settingLabels: [
          { symbol: "P1.K2", label: "State feedback gain on P1.Tank2M" }, { symbol: "P1.K1", label: "State feedback gain on P1.Tank1M" },
          { symbol: "P1.N", label: "Reference gain on P1.Tank2SetpointM" }] } });
      const labels = [...root.querySelectorAll("dt")].map((label: any) => label.textContent);
      assert.deepStrictEqual(labels, ["State feedback gain on P1.Tank2M", "State feedback gain on P1.Tank1M", "Reference gain on P1.Tank2SetpointM"],
        "a symbol is shown exactly as the program declares it");
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });
});

suite("Control Studio state-feedback margins", () => {
});
