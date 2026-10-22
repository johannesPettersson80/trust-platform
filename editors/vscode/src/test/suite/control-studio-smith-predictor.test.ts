import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { open as openFormHarness } from "./control-studio-loop-setup-form-harness";

/** The engine's Smith predictor loop: a PI whose program feeds its PID the measurement and the predictor's correction (specification 35, C9). */
function smithLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/smith-control-loop.json"), "utf8"));
}

suite("Control Studio Smith predictor", () => {

  test("Tune names the PI gains and the program's model values", () => {
    const loop = smithLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "Smith", words: { gainLabels: { proportional: "Proportional gain (Kp)", integral: "Integral gain (Ki)", derivative: "Derivative gain (Kd)" } }, values: { "P1.Parameters.ProportionalGain": 10,
        "P1.Parameters.IntegralGain": 2, "P1.Parameters.DerivativeGain": 0, "P1.ModelGain": 0.2, "P1.ModelTimeConstantS": 5, "P1.ModelDelayS": 1 } });
      assert.deepStrictEqual([...dom.window.document.querySelectorAll("dt")].map((label: any) => label.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Model gain K", "Model time constant T", "Model delay θ"]);
    } finally { (globalThis as any).document = saved; }
  });
});

suite("Control Studio Smith predictor setup", () => {
  const signals = ["P1.HeaterPct", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"];
  const formModel = {
    structure: "pid", addressedReal: signals, measurements: signals, signals, booleans: ["P1.Auto"], modes: ["P1.Auto"],
    records: ["P1.Parameters"], writes: [], initializes: [], prefills: [], loopNames: [{ instance: "P1.Loop", name: "Loop" }],
    pids: [{ instance: "P1.Loop", task: { name: "MainTask", intervalMs: 100 } }], instances: ["P1"],
    reals: ["P1.CorrectionC", "P1.HeaterPct", "P1.ManualPct", "P1.ModelDelayS", "P1.ModelGain", "P1.ModelTimeConstantS", "P1.SetpointC", "P1.WaterC"],
    prefill: { instance: "P1.Loop", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualPct" },
  };
  test("sets up the PID and the program's predictor variables in one form", () => {
    const { open } = require("./control-studio-loop-setup-form-harness");
    const view = open(formModel);
    try {
      const structures = Array.from(view.root.querySelectorAll('[data-setup-field="structure"] option') as ArrayLike<{ value: string }>).map(option => option.value);
      assert.ok(structures.includes("smith"), `a Smith predictor has its own structure: ${structures}`);
      view.set("structure", "smith");
      for (const [field, value] of [["name", "Water temperature"], ["processVariable", "P1.WaterC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"]]) view.set(field, value);
      const save = view.root.querySelector("[data-setup-save]"), message = view.root.querySelector("[data-setup-message]");
      for (const [field, value] of [["smith.gain", "P1.ModelGain"], ["smith.timeConstant", "P1.ModelTimeConstantS"], ["smith.delay", "P1.ModelDelayS"],
        ["smith.correction", "P1.CorrectionC"]]) view.set(field, value);
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts, [{ kind: "control-studio.setup.save", draft: {
        structure: "smith", name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
        processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: "0", maximum: "80" },
        manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: "0", maximum: "100" },
        setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
        model: { gain: "P1.ModelGain", timeConstant: "P1.ModelTimeConstantS", delay: "P1.ModelDelayS", correction: "P1.CorrectionC" },
      } }]);
    } finally { view.close(); }
  });

  test("posts the library block's four variables together, and only when the program uses the block (C9)", () => {
    const view = openFormHarness({ ...formModel, reals: [...formModel.reals, "P1.BlockOutput", "P1.MaxCorrection", "P1.OpIn", "P1.OpOut"] });
    try {
      view.set("structure", "smith");
      for (const [field, value] of [["name", "Water temperature"], ["processVariable", "P1.WaterC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"], ["smith.gain", "P1.ModelGain"], ["smith.timeConstant", "P1.ModelTimeConstantS"],
        ["smith.delay", "P1.ModelDelayS"], ["smith.correction", "P1.CorrectionC"]]) view.set(field, value);
      const block = view.root.querySelector<HTMLElement>("[data-smith-block]")!;
      assert.strictEqual(block.hidden, true, "the library block's variables are asked only when the program uses the block");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.strictEqual("libraryBlock" in view.posts[0].draft, false, "a program that keeps its own predictor names no block");
      const box = view.root.querySelector<HTMLInputElement>('[data-setup-field="smith.libraryBlock"]')!;
      box.checked = true;
      box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      assert.strictEqual(block.hidden, false);
      for (const [field, value] of [["smith.block.output", "P1.BlockOutput"], ["smith.block.maximumCorrection", "P1.MaxCorrection"],
        ["smith.block.operatingInput", "P1.OpIn"], ["smith.block.operatingOutput", "P1.OpOut"]]) view.set(field, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[1].draft.libraryBlock, { output: "P1.BlockOutput", maximumCorrection: "P1.MaxCorrection",
        operatingInput: "P1.OpIn", operatingOutput: "P1.OpOut" });
    } finally { view.close(); }
  });
});
