import * as assert from "assert";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { pidLoop } from "./control-studio-pid-fixtures";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

suite("Control Studio candidate robustness margins", () => {
  test("the Tune page keeps the host's margins when it shows the candidate", () => {
    const dom = new JSDOM(`<main><select data-studio-controller></select><select data-engineer-tuning-method><option value="automatic" selected>Automatic</option></select>`
      + `<div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = new Map<string, unknown>();
    for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
    try {
      const { ThreePageTuning } = require("../../controlStudio/webview/threePageTuning");
      const tuning = new ThreePageTuning(root, pidLoop(), () => undefined, () => "PI");
      tuning.receive({ status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: 12.04, phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      assert.strictEqual(root.querySelector("[data-studio-robustness]")?.textContent,
        "Gain margin 12.04 dB · phase margin 58.3° · delay margin 1.27 s · peak sensitivity Ms 1.43");
    } finally { for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
  });

  test("a failed Tune leaves no earlier candidate on the Tune page", () => {
    const dom = new JSDOM(`<main><select data-studio-controller></select><select data-engineer-tuning-method><option value="automatic" selected>Automatic</option></select>`
      + `<div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = new Map<string, unknown>();
    for (const key of ["document", "window", "Option", "HTMLElement"]) { previous.set(key, (globalThis as any)[key]); (globalThis as any)[key] = key === "window" ? dom.window : dom.window[key]; }
    try {
      const { ThreePageTuning } = require("../../controlStudio/webview/threePageTuning");
      const tuning = new ThreePageTuning(root, pidLoop(), () => undefined, () => "PI");
      tuning.receive({ status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 } });
      assert.strictEqual(root.querySelector("[data-studio-tuning-result]").hidden, false, "the candidate is shown");
      tuning.receive({ status: "failed", flowStage: "Tune", reason: "Tuning failed: PI cannot meet the requirements on the model." });
      const area = root.querySelector("[data-studio-tuning-result]");
      assert.ok(area.hidden && !area.textContent.includes("Proportional gain"),
        "the host dropped the candidate, so its gains no longer stand beside the refusal as if they were its result");
    } finally { for (const [key, value] of previous) (globalThis as any)[key] = value; dom.window.close(); }
  });

  test("the Tune page shows the margins beside the candidate", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const root = dom.window.document.querySelector("main");
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const loop = pidLoop();
      renderTunedController(root, loop, { status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: 12.04, phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      const line = root.querySelector("[data-studio-robustness]");
      assert.ok(line, "the candidate shows its margins");
      assert.strictEqual(line.textContent, "Gain margin 12.04 dB · phase margin 58.3° · delay margin 1.27 s · peak sensitivity Ms 1.43");
      renderTunedController(root, loop, { status: "candidate", family: "PI", values: { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 },
        robustness: { gainMarginDb: "unbounded", phaseMarginDegrees: 58.3, delayMarginSeconds: 1.27, peakSensitivity: 1.43 } });
      assert.match(root.querySelector("[data-studio-robustness]").textContent, /^Gain margin unbounded · /);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });
});
