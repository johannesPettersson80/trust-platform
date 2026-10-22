import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import { open } from "./control-studio-loop-setup-form-harness";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** The search draft as the form posts it (B1.3, C5): every number as the text typed, and every kept-value row the template draws (the
 * engine's limit, three), the last one blank at the engine's default start; the engine skips a row without its variable. */
const draft = { ...scope, name: "Oven MPC", action: "direct", family: "MPC", instance: "P1",
  processVariable: { symbol: "P1.OvenC", unit: "degC", minimum: "0", maximum: "300" },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: "0", maximum: "100" },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  coefficients: [{ symbol: "P1.Weight", unit: "1", values: [0.5, 1, 2] }, { symbol: "P1.Horizon", unit: "s", values: [0.1, 5] }],
  states: [{ symbol: "P1.Integral", unit: "%", start: "output" }, { symbol: "P1.Filtered", unit: "degC", start: "measurement" },
    { symbol: "", unit: "", start: "zero" }],
  startupFlag: "P1.Started" };
/** A coefficient row the template draws (the engine's limit, four) and the engineer left blank. */
const blankCoefficient = { symbol: "", unit: "", values: "" };

suite("Control Studio coefficients tuned by search setup", () => {
});

/** The oven's compiled facts as the setup form sees them. */
const searchSignals = ["P1.HeaterPct", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"];
const searchModel = { structure: "identify", pids: [], prefills: [], loopNames: [], records: [], initializes: [], instances: ["P1"],
  addressedReal: searchSignals, measurements: searchSignals, signals: searchSignals, booleans: ["P1.AutoMode", "P1.Started"], modes: ["P1.AutoMode", "P1.Started"],
  reals: ["P1.Filtered", "P1.HeaterPct", "P1.Horizon", "P1.Integral", "P1.ManualPct", "P1.OvenC", "P1.SetpointC", "P1.Weight"],
  writes: ["P1.HeaterPct", "P1.Integral", "P1.Filtered"] };
const searchFill: [string, string][] = [["structure", "search"], ["name", "Oven MPC"], ["action", "direct"], ["search.family", "MPC"], ["search.instance", "P1"],
  ["search.processVariable", "P1.OvenC"], ["search.processVariable.unit", "degC"], ["search.processVariable.minimum", "0"], ["search.processVariable.maximum", "300"],
  ["search.setpoint", "P1.SetpointC"], ["search.manipulatedVariable", "P1.HeaterPct"], ["search.manipulatedVariable.unit", "%"],
  ["search.manipulatedVariable.minimum", "0"], ["search.manipulatedVariable.maximum", "100"], ["search.mode", "P1.AutoMode"], ["search.manualOutput", "P1.ManualPct"],
  ["search.coefficient.0", "P1.Weight"], ["search.coefficient.0.unit", "1"], ["search.coefficient.0.values", "0.5, 1, 2"],
  ["search.coefficient.1", "P1.Horizon"], ["search.coefficient.1.unit", "s"], ["search.coefficient.1.values", "0.1; 5"],
  ["search.state.0", "P1.Integral"], ["search.state.0.unit", "%"], ["search.state.0.start", "output"],
  ["search.state.1", "P1.Filtered"], ["search.state.1.unit", "degC"], ["search.state.1.start", "measurement"]];

/** The course's trajectory loop (lesson 90): its controller is named Main, which the Tune page's title reads. The law's name comes from the engine's family. */
function searchLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-loop-shapes/trajectory-control-loop.json"), "utf8"));
}

suite("Control Studio coefficients tuned by search Tune result", () => {
  test("names the law and its coefficients by what they are, not by internal ids", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const root = dom.window.document.querySelector("main");
      // The law's name is the engine's label of its family: the open answer's controller offer (`family_label` of open_offers.rs, C2).
      setOpenedOffers({ controllerOffer: { families: [{ family: "split-range", label: "Split range" }], familySettings: {}, settingRows: [] } });
      renderTunedController(root, searchLoop(), { status: "candidate", family: "split-range", values: { "P1.Weight": 1, "P1.Horizon": 5 },
        words: { outcome: "Meets the requirements", settingLabels: [{ symbol: "P1.Weight", label: "Weight" }, { symbol: "P1.Horizon", label: "Horizon" }] } });
      assert.strictEqual(root.querySelector("p").textContent, "Split range controller · Main");
      assert.deepStrictEqual(Array.from(root.querySelectorAll("dt")).map((term: any) => term.textContent), ["Weight", "Horizon"]);
    } finally { setOpenedOffers({}); (globalThis as any).document = previous; dom.window.close(); }
  });
});

suite("Control Studio coefficients tuned by search setup form", () => {
  test("sets up the coefficients, their values and the values the program keeps from scan to scan", () => {
    const view = open(searchModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("search"), `a law tuned by search has its own structure: ${structures}`);
      for (const [field, value] of searchFill) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, view.root.querySelector("[data-setup-message]")!.textContent ?? "");
      save.click();
      // The coefficients' values are posted as typed: the engine reads them (C5).
      const { projectId: _project, runtimeId: _runtime, startupFlag: _flag, coefficients: _coefficients, ...expected } = draft;
      assert.deepStrictEqual(view.posts[0].draft, { structure: "search", ...expected,
        coefficients: [{ symbol: "P1.Weight", unit: "1", values: "0.5, 1, 2" }, { symbol: "P1.Horizon", unit: "s", values: "0.1; 5" },
          blankCoefficient, blankCoefficient] });
    } finally { view.close(); }
  });

  test("posts a value list it cannot read as typed; the engine refuses it and the combination limit", () => {
    const view = open(searchModel);
    try {
      for (const [field, value] of searchFill) view.set(field, value);
      view.set("search.coefficient.0.values", "0.5, fast");
      view.set("search.coefficient.1.values", "1, 2, 3, 4, 5, 6, 7, 8");
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, "the engine reads the lists and checks the combinations, not the form");
      save.click();
      assert.deepStrictEqual(view.posts[0].draft.coefficients.map((entry: { values: string }) => entry.values), ["0.5, fast", "1, 2, 3, 4, 5, 6, 7, 8", "", ""]);
    } finally { view.close(); }
  });
});
