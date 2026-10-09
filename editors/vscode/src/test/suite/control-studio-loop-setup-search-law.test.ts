import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { open } from "./control-studio-loop-setup-form-harness";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";

/** Loaded dynamically so that a missing generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** An oven whose program computes a law from coefficients no formula designs; Tune searches their values. */
const oven = {
  symbols: [
    { symbol: "P1.OvenC", type: "LREAL", address: "%IL0" }, { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
    { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.Started", type: "BOOL" },
    { symbol: "P1.Weight", type: "LREAL" }, { symbol: "P1.Horizon", type: "REAL" }, { symbol: "P1.Integral", type: "LREAL" },
    { symbol: "P1.Filtered", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }],
  links: [],
  writes: ["P1.HeaterPct", "P1.Integral", "P1.Filtered", "P1.Started"],
};
const draft = { ...scope, name: "Oven MPC", action: "direct", family: "MPC", instance: "P1",
  processVariable: { symbol: "P1.OvenC", unit: "degC", minimum: 0, maximum: 300 },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  coefficients: [{ symbol: "P1.Weight", unit: "1", values: [0.5, 1, 2] }, { symbol: "P1.Horizon", unit: "s", values: [0.1, 5] }],
  states: [{ symbol: "P1.Integral", unit: "%", start: "output" }, { symbol: "P1.Filtered", unit: "degC", start: "measurement" }],
  startupFlag: "P1.Started" };
const model = { kind: "transfer-function", numerator: [2], denominator: [60, 1], delaySeconds: 5, revision: "oven", source: "fitted",
  validation: "validated", evidence: ["recording"], inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: 0, maximum: 300 } }], operatingPoint: { inputs: [20], outputs: [60] } };

suite("Control Studio coefficients tuned by search setup", () => {
  test("writes a project controller that searches every combination of the coefficient values on the actual ST", () => {
    const api = setup();
    assert.strictEqual(typeof api.searchLawLoopDefinition, "function", "a law with coefficients no formula designs needs its own setup structure");
    const loop = api.searchLawLoopDefinition(oven, draft);
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.convention, project.seed], ["MPC", "project-owned-native-controller", undefined]);
    assert.deepStrictEqual(project.axes, [{ symbol: "P1.Weight", values: [0.5, 1, 2] }, { symbol: "P1.Horizon", values: [Math.fround(0.1), 5] }],
      "a REAL variable's values are rounded to the nearest REAL");
    assert.deepStrictEqual([project.objective, project.maximumCandidates], [{ requirementId: "target-error", direction: "minimize" }, 64]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit, parameter.minimum, parameter.maximum]),
      [["P1.Weight", "1", 0.5, 2], ["P1.Horizon", "s", Math.fround(0.1), 5]]);
    assert.deepStrictEqual(project.initialization.states.map((state: any) => [state.symbol, state.unit, state.initial]),
      [["P1.Integral", "%", { source: "model-input", index: 0 }], ["P1.Filtered", "degC", { source: "model-output", index: 0 }], ["P1.Started", "1", 1]]);
    const { engineerVerificationCampaign } = require("../../controlStudio/engineerVerificationCampaign");
    const measured = engineerVerificationCampaign(loop, model, { referenceChange: 10, maximumError: 1, settlingBand: 0.5, settlingTimeSeconds: 200, durationSeconds: 400 },
      { "P1.Weight": 1, "P1.Horizon": 5 }).flatMap((scenario: any) => scenario.requirements.map((requirement: any) => requirement.id));
    assert.ok(measured.includes(project.objective.requirementId), measured.join(", "));
  });

  test("refuses coefficients it cannot search", () => {
    const api = setup();
    assert.strictEqual(typeof api.searchLawLoopDefinition, "function", "a law with coefficients no formula designs needs its own setup structure");
    assert.throws(() => api.searchLawLoopDefinition(oven, { ...draft, coefficients: [] }), /at least one coefficient/);
    assert.throws(() => api.searchLawLoopDefinition(oven, { ...draft, coefficients: [1, 2, 3, 4, 5].map(n => ({ symbol: `P1.C${n}`, unit: "1", values: [1] })) }),
      /at most four/);
    assert.throws(() => api.searchLawLoopDefinition(oven, { ...draft, coefficients: [{ symbol: "P1.Weight", unit: "1", values: [1, 2, 3, 4, 5, 6, 7, 8, 9] },
      { symbol: "P1.Horizon", unit: "s", values: [1, 2, 3, 4, 5, 6, 7, 8] }] }), /64 combinations/);
    assert.throws(() => api.searchLawLoopDefinition(oven, { ...draft, states: [{ symbol: "P1.Weight", unit: "1", start: "zero" }] }), /different variable/);
    assert.throws(() => api.searchLawLoopDefinition(oven, { ...draft, coefficients: [{ symbol: "P1.Weight", unit: "1", values: [] }] }), /values/);
  });
});

/** The oven's compiled facts as the setup form sees them. */
const searchModel = { structure: "identify", pids: [], prefills: [], records: [], initializes: [], instances: ["P1"],
  addressedReal: ["P1.HeaterPct", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"], booleans: ["P1.AutoMode", "P1.Started"],
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

suite("Control Studio coefficients tuned by search Tune result", () => {
  test("names the law and its coefficients by what they are, not by internal ids", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const root = dom.window.document.querySelector("main");
      const loop = setup().searchLawLoopDefinition(oven, { ...draft, family: "split-range" });
      renderTunedController(root, loop, { status: "candidate", family: "split-range", values: { "P1.Weight": 1, "P1.Horizon": 5 } });
      assert.strictEqual(root.querySelector("p").textContent, "Split range controller · Main");
      assert.deepStrictEqual(Array.from(root.querySelectorAll("dt")).map((term: any) => term.textContent), ["Weight", "Horizon"]);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
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
      const { projectId: _project, runtimeId: _runtime, startupFlag: _flag, ...expected } = draft;
      assert.deepStrictEqual(view.posts[0].draft, { structure: "search", ...expected });
    } finally { view.close(); }
  });

  test("names a value list it cannot read and the combination limit", () => {
    const view = open(searchModel);
    try {
      for (const [field, value] of searchFill) view.set(field, value);
      view.set("search.coefficient.0.values", "0.5, fast");
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /values of P1\.Weight/);
      view.set("search.coefficient.0.values", "1, 2, 3, 4, 5, 6, 7, 8, 9");
      view.set("search.coefficient.1.values", "1, 2, 3, 4, 5, 6, 7, 8");
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /64 combinations/);
    } finally { view.close(); }
  });
});

suite("Control Studio coefficients tuned by search saving", () => {
  test("saves the searched law as one loop file", async () => {
    const { saveLoopSetup } = require("../../controlStudio/loopSetup/loopSetupSave");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-search-law-"));
    try {
      const { projectId: _project, runtimeId: _runtime, ...unscoped } = draft;
      await saveLoopSetup(root, scope, oven, { structure: "search", ...unscoped });
      const saved = JSON.parse(fs.readFileSync(path.join(root, "control-loops", "oven-mpc.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, scope).kind, "accepted");
      assert.deepStrictEqual(saved.controller.project.axes.map((axis: any) => axis.symbol), ["P1.Weight", "P1.Horizon"]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
