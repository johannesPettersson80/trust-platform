import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { open } from "./control-studio-loop-setup-form-harness";

/** The course's measured-disturbance loop (lesson 84): one PID whose program adds a lead and lag feedforward from the measured heat extraction. */
function feedforwardLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/feedforward-control-loop.json"), "utf8"));
}
const values = { "P1.Parameters.ProportionalGain": 5, "P1.Parameters.IntegralGain": 1, "P1.Parameters.DerivativeGain": 0,
  "P1.FeedforwardGain": 0.5, "P1.LeadS": 8, "P1.LagS": 3 };

suite("Control Studio feedforward from a measured disturbance", () => {

  test("Tune shows the engine's feedforward answer: both paths, K_ff, lead and lag, and the two largest deviations", () => {
    const loop = feedforwardLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      // The engine's FeedforwardAnswer in the names its JSON gives (messages/tune.rs): the page shows these and computes none of them.
      // Every gain comes with the engine's unit (the model's channels': degC per % and per kW, and % per kW for K_ff): the page composes none.
      const feedforward = { outputPath: { gain: 0.2, gainUnit: "degC/%", timeConstantSeconds: 8, delaySeconds: 0 },
        disturbancePath: { gain: -0.1, gainUnit: "degC/kW", timeConstantSeconds: 3, delaySeconds: 0 },
        gain: 0.5, gainUnit: "%/kW", leadSeconds: 8, lagSeconds: 3, disturbanceStep: 10, disturbanceUnit: "kW", measurementUnit: "degC",
        feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
      // The engine's words of the answer (TuneWords): one row for each named setting; the derivative gain of this law has none.
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "feedforward", values, design: { feedforward },
        words: { outcome: "Meets the requirements", settingLabels: [{ symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)" },
          { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)" }, { symbol: "P1.FeedforwardGain", label: "Feedforward gain" },
          { symbol: "P1.LeadS", label: "Lead time" }, { symbol: "P1.LagS", label: "Lag time" }] } });
      const document = dom.window.document;
      assert.deepStrictEqual([...document.querySelectorAll("dt")].map((label: any) => label.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Feedforward gain", "Lead time", "Lag time"]);
      const table = document.querySelector("[data-studio-feedforward-design]");
      assert.ok(table, "the design names both path models");
      assert.deepStrictEqual([...table.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent)), [
        ["Controller output P1.HeaterPct", "0.2 °C/%", "8 s", "0 s"],
        ["Disturbance P1.LoadKw", "-0.1 °C/kW", "3 s", "0 s"]]);
      assert.strictEqual(document.querySelector("[data-studio-feedforward-values]")?.textContent,
        "Feedforward gain K_ff 0.5 %/kW · lead time T_lead 8 s · lag time T_lag 3 s");
      assert.strictEqual(document.querySelector("[data-studio-feedforward-deviation]")?.textContent,
        "Largest deviation after a 10 kW disturbance step, on the model: 0.61 °C with the PI alone, 0.004 °C with feedforward");
    } finally { (globalThis as any).document = saved; }
  });

  test("Tune names only the gain of a feedforward without lead and lag, as the engine answers it", () => {
    const loop = feedforwardLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const feedforward = { outputPath: { gain: 0.2, gainUnit: "degC/%", timeConstantSeconds: 8, delaySeconds: 0 },
        disturbancePath: { gain: -0.1, gainUnit: "degC/kW", timeConstantSeconds: 3, delaySeconds: 0 },
        gain: 0.5, gainUnit: "%/kW", disturbanceStep: 10, disturbanceUnit: "kW", measurementUnit: "degC", feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "feedforward", values: { "P1.Parameters.ProportionalGain": 5,
        "P1.Parameters.IntegralGain": 1, "P1.Parameters.DerivativeGain": 0, "P1.FeedforwardGain": 0.5 }, design: { feedforward } });
      assert.strictEqual(dom.window.document.querySelector("[data-studio-feedforward-values]")?.textContent, "Feedforward gain K_ff 0.5 %/kW",
        "without lead and lag the answer gives no lead or lag time");
    } finally { (globalThis as any).document = saved; }
  });

  test("Tune shows a gain the engine gave no unit for without one, and composes none from the loop's signals", () => {
    const loop = feedforwardLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      // The same answer without its units: the loop declares degC, % and kW, and the page would have composed °C/% from them before.
      const feedforward = { outputPath: { gain: 0.2, timeConstantSeconds: 8, delaySeconds: 0 }, disturbancePath: { gain: -0.1, timeConstantSeconds: 3, delaySeconds: 0 },
        gain: 0.5, disturbanceStep: 10, feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "feedforward", values, design: { feedforward } });
      const document = dom.window.document;
      assert.deepStrictEqual([...document.querySelectorAll("[data-studio-feedforward-design] tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent)), [
        ["Controller output P1.HeaterPct", "0.2", "8 s", "0 s"], ["Disturbance P1.LoadKw", "-0.1", "3 s", "0 s"]]);
      assert.strictEqual(document.querySelector("[data-studio-feedforward-values]")?.textContent, "Feedforward gain K_ff 0.5");
      assert.strictEqual(document.querySelector("[data-studio-feedforward-deviation]")?.textContent,
        "Largest deviation after a 10 disturbance step, on the model: 0.61 with the PI alone, 0.004 with feedforward");
    } finally { (globalThis as any).document = saved; }
  });
});

/** The heater program as the setup form sees it: one library PID and the feedforward's variables (Specification 35 §17.12). */
const feedforwardSignals = ["P1.HeaterPct", "P1.LoadKw", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"];
const formModel = {
  structure: "pid", addressedReal: feedforwardSignals, measurements: feedforwardSignals, signals: feedforwardSignals, booleans: ["P1.Auto"],
  modes: ["P1.Auto"], prefills: [], loopNames: [{ instance: "P1.Loop", name: "Loop" }],
  records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Loop", task: { name: "MainTask", intervalMs: 100 } }],
  instances: ["P1"], reals: ["P1.FeedforwardGain", "P1.FilteredLoadKw", "P1.HeaterPct", "P1.LagS", "P1.LeadS", "P1.LoadKw", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"],
  prefill: { instance: "P1.Loop", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualPct" },
};

suite("Control Studio feedforward setup", () => {
  test("sets up the PID and the program's feedforward from a measured disturbance in one form", () => {
    const view = open(formModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("feedforward"), `a feedforward has its own structure: ${structures}`);
      view.set("structure", "feedforward");
      assert.strictEqual(view.root.querySelector<HTMLElement>('[data-setup-section="feedforward"]')?.hidden, false);
      for (const [field, value] of [["name", "Water temperature"], ["processVariable", "P1.WaterC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      save.click();
      assert.strictEqual(view.posts[0].draft.disturbance.symbol, "", "the measured disturbance is posted blank, and the engine names it");
      for (const [field, value] of [["ff.disturbance", "P1.LoadKw"], ["ff.disturbance.unit", "kW"], ["ff.disturbance.minimum", "0"],
        ["ff.disturbance.maximum", "50"], ["ff.disturbanceCommand", "P1.LoadKw"], ["ff.gain", "P1.FeedforwardGain"]]) view.set(field, value);
      // The draft carries the disturbance's identification step, in its unit (the engine's disturbanceStep); the form asks for it (C8).
      save.click();
      assert.strictEqual(view.posts[1].draft.disturbanceStep, "", "the step is still missing: it is posted as the blank text typed, and the engine asks for it");
      view.set("ff.disturbanceStep", "10");
      view.set("ff.lead", "P1.LeadS");
      save.click();
      assert.strictEqual(view.posts[2].draft.feedforward.lag, null, "a lead time posted without its lag time and filtered disturbance: the engine asks for both");
      view.set("ff.lag", "P1.LagS"); view.set("ff.filtered", "P1.FilteredLoadKw");
      save.click();
      assert.deepStrictEqual(view.posts[3], { kind: "control-studio.setup.save", draft: {
        structure: "feedforward", name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
        processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: "0", maximum: "80" },
        manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: "0", maximum: "100" },
        setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
        disturbance: { symbol: "P1.LoadKw", unit: "kW", minimum: "0", maximum: "50" }, disturbanceCommand: "P1.LoadKw", disturbanceStep: "10",
        feedforward: { gain: "P1.FeedforwardGain", lead: "P1.LeadS", lag: "P1.LagS", filtered: "P1.FilteredLoadKw" },
      } });
    } finally { view.close(); }
  });

  test("offers a disturbance the program reads from memory, as a simulated or entered load is", () => {
    // Measurements for the controller are what a simulated plant can write; a disturbance is only read by the program.
    const view = open({ ...formModel, signals: formModel.addressedReal, measurements: ["P1.WaterC"] });
    try {
      const offered = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="ff.disturbance"] option')).map(option => option.value);
      assert.ok(offered.includes("P1.LoadKw"), `offered: ${offered}`);
    } finally { view.close(); }
  });
});
