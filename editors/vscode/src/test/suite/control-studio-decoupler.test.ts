import * as assert from "assert";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";
import { open } from "./control-studio-loop-setup-form-harness";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

/** Two coupled zones, one library PID each; the program passes their outputs through a decoupler (Specification 35 §17.15). */
const zones = {
  symbols: [
    { symbol: "P1.Zone1C", type: "LREAL", address: "%IL0" }, { symbol: "P1.Zone2C", type: "LREAL", address: "%IL8" },
    { symbol: "P1.Heater1Pct", type: "LREAL", address: "%QL0" }, { symbol: "P1.Heater2Pct", type: "LREAL", address: "%QL8" },
    { symbol: "P1.Zone1SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.Zone2SetpointC", type: "LREAL", address: "%ML8" },
    { symbol: "P1.Zone1ManualPct", type: "LREAL", address: "%ML16" }, { symbol: "P1.Zone2ManualPct", type: "LREAL", address: "%ML24" },
    { symbol: "P1.ZonesAuto", type: "BOOL" },
    { symbol: "P1.D11", type: "LREAL" }, { symbol: "P1.D12", type: "LREAL" }, { symbol: "P1.D21", type: "LREAL" }, { symbol: "P1.D22", type: "LREAL" },
    ...pidSymbols("P1.Zone1", "P1.Zone1Parameters"), ...pidSymbols("P1.Zone2", "P1.Zone2Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Zone1", type: "Control.PID", tasks: TASK },
    { instance: "P1.Zone2", type: "Control.PID", tasks: TASK }],
  links: [],
};
const member = (index: number) => ({ instance: `P1.Zone${index}`, parameterRecord: `P1.Zone${index}Parameters`,
  processVariable: { symbol: `P1.Zone${index}C`, unit: "degC", minimum: 0, maximum: 100 },
  manipulatedVariable: { symbol: `P1.Heater${index}Pct`, unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: `P1.Zone${index}SetpointC` }, manualOutput: `P1.Zone${index}ManualPct`, experimentStep: 10 });
const decoupler = [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]];
const draft = () => ({ ...scope, name: "Zones", action: "direct", mode: { symbol: "P1.ZonesAuto", whenTrue: "Auto" },
  experimentDurationSeconds: 120, members: [member(1), member(2)], decoupler });
/** Heat crosses the shared wall: x' = A·x + 0.01·u, so K = −A⁻¹·B = [[0.085, 0.026], [0.026, 0.085]] °C/%. */
const model: any = { kind: "continuous-state-space", revision: "zones", source: "fitted", validation: "validated", evidence: ["recording"], delaySeconds: 0,
  a: [[-0.13, 0.04], [0.04, -0.13]], b: [[0.01, 0], [0, 0.01]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]],
  states: [{ id: "P1.Zone1C", unit: "degC", operatingRange: { minimum: -60, maximum: 60 } }, { id: "P1.Zone2C", unit: "degC", operatingRange: { minimum: -60, maximum: 60 } }],
  inputs: [{ id: "P1.Heater1Pct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }, { id: "P1.Heater2Pct", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "P1.Zone1C", unit: "degC", operatingRange: { minimum: 0, maximum: 100 } }, { id: "P1.Zone2C", unit: "degC", operatingRange: { minimum: 0, maximum: 100 } }],
  operatingPoint: { inputs: [50, 50], outputs: [35, 35] } };
const zoneRequirement = { referenceChange: 1, maximumError: 0.3, settlingBand: 0.1, settlingTimeSeconds: 40 };
const task = { ...zoneRequirement, durationSeconds: 80, channelRequirements: { "P1.Zone1C": zoneRequirement, "P1.Zone2C": zoneRequirement } };
const k = 0.01 / (0.13 * 0.13 - 0.04 * 0.04), K = [[0.13 * k, 0.04 * k], [0.04 * k, 0.13 * k]];
/** D = K⁻¹·diag(K): K⁻¹ = −B⁻¹A = [[13, −4], [−4, 13]], so D = K₁₁·[[13, −4], [−4, 13]]. */
const expectedD = [[13 * K[0][0], -4 * K[0][0]], [-4 * K[0][0], 13 * K[0][0]]];
function decoupledLoop(): any {
  const api = setup();
  assert.strictEqual(typeof api.pidGroupLoopDefinition, "function");
  return api.pidGroupLoopDefinition(zones, draft());
}

suite("Control Studio static decoupler", () => {
  test("writes a group loop whose decoupler Tune sets and whose PIDs start at their share of the plant inputs", () => {
    const loop = decoupledLoop();
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    const validation: any = validateControlLoopDefinition(loop, scope);
    assert.strictEqual(validation.kind, "accepted", JSON.stringify(validation.reason ?? validation));
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.seed], ["decoupled-PI", { kind: "decoupled-pid", family: "PI", matrixSymbols: decoupler }]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.slice(6).map((parameter: any) => [parameter.symbol, parameter.unit]),
      [["P1.D11", "1"], ["P1.D12", "1"], ["P1.D21", "1"], ["P1.D22", "1"]]);
    const initial = (symbol: string) => project.initialization.states.find((state: any) => state.symbol === symbol)?.initial;
    assert.deepStrictEqual(initial("P1.Zone2.StateValue.NominalOutput"), { source: "decoupled-model-input", index: 1, matrixSymbols: decoupler });
    assert.deepStrictEqual(initial("P1.Zone1.StateValue.SaturatedOutput"), { source: "decoupled-model-input", index: 0, matrixSymbols: decoupler });
  });

  test("Tune offers the decoupled group as its controller", () => {
    const loop = decoupledLoop();
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    let families: unknown;
    try { families = controllerAdapter(loop).families; } catch (error) { families = String(error); }
    assert.deepStrictEqual(families, ["decoupled-PI"]);
  });

  test("designs D = K⁻¹·diag(K), so each member moves only its own measurement in steady state, and its PI through it", () => {
    const loop = decoupledLoop();
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const values = projectControllerSeed(loop, model, task, loop.controller.project.seed);
    decoupler.forEach((row, i) => row.forEach((symbol, j) => assert.ok(Math.abs(values[symbol] - expectedD[i][j]) < 1e-4, `${symbol}: ${values[symbol]} vs ${expectedD[i][j]}`)));
    for (const index of [1, 2]) assert.ok(values[`P1.Zone${index}Parameters.ProportionalGain`] > 0 && values[`P1.Zone${index}Parameters.IntegralGain`] > 0, JSON.stringify(values));
    const decoupled = K.map(row => [0, 1].map(j => row[0] * values[decoupler[0][j]] + row[1] * values[decoupler[1][j]]));
    assert.ok(Math.abs(decoupled[0][1]) < 1e-9 && Math.abs(decoupled[1][0]) < 1e-9, `K·D is diagonal: ${JSON.stringify(decoupled)}`);
  });

  test("validation starts each PID at its share of the operating inputs, and Apply reads it from the committed outputs", () => {
    const loop = decoupledLoop();
    const values: Record<string, number> = { "P1.Zone1Parameters.ProportionalGain": 5, "P1.Zone1Parameters.IntegralGain": 0.5, "P1.Zone1Parameters.DerivativeGain": 0,
      "P1.Zone2Parameters.ProportionalGain": 5, "P1.Zone2Parameters.IntegralGain": 0.5, "P1.Zone2Parameters.DerivativeGain": 0,
      "P1.D11": 1, "P1.D12": -0.5, "P1.D21": -0.5, "P1.D22": 1 };
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    // The session attaches each output's requirements to its channel before validation.
    loop.channels.outputs = loop.channels.outputs.map((output: any) => ({ ...output, requirements: zoneRequirement }));
    const plan = engineerVerificationPlan(loop, model, task, values);
    // D⁻¹·[50, 50] = [100, 100].
    assert.deepStrictEqual([plan.request.initial_values["P1.Zone1.StateValue.NominalOutput"], plan.request.initial_values["P1.Zone2.StateValue.SaturatedOutput"]], [100, 100]);
    const { declaredTransferBinding, declaredCommissioningProfile } = require("../../controlStudio/controllerInitialization");
    const initialization = loop.controller.project.initialization;
    const tracking = declaredCommissioningProfile(loop, model, initialization, "Decoupled", values).tracking(loop, declaredTransferBinding(loop, initialization));
    assert.deepStrictEqual(tracking.initialization_formulas["P1.Zone2.StateValue.NominalOutput"],
      { kind: "decoupled-output", output_symbols: ["P1.Heater1Pct", "P1.Heater2Pct"], matrix_symbols: decoupler, row: 1 });
  });

  test("Tune shows the decoupler", () => {
    const loop = decoupledLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "decoupled-PI", values: {},
        design: { decoupler: { matrix: [[1.1, -0.34], [-0.34, 1.1]], inputIds: ["P1.Heater1Pct", "P1.Heater2Pct"], memberIds: ["P1.Zone1", "P1.Zone2"] } } });
      const table = dom.window.document.querySelector("[data-studio-decoupler]");
      assert.ok(table, "Tune shows D");
      assert.deepStrictEqual([...table.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent)),
        [["P1.Heater1Pct", "1.1", "-0.34"], ["P1.Heater2Pct", "-0.34", "1.1"]]);
    } finally { (globalThis as any).document = saved; }
  });

  test("behind a decoupler, a member's output is the plant input, not the controller's own output", function () {
    this.timeout(10000);
    const view = open({ structure: "group", addressedReal: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC",
      "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"], signals: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Request1Pct", "P1.Request2Pct",
      "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC", "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"],
      booleans: ["P1.ZonesAuto"], modes: ["P1.ZonesAuto"], records: ["P1.Zone1Parameters", "P1.Zone2Parameters"], writes: [], initializes: [], instances: ["P1"],
      pids: [{ instance: "P1.Zone1", task: { name: "MainTask", intervalMs: 100 } }, { instance: "P1.Zone2", task: { name: "MainTask", intervalMs: 100 } }],
      reals: ["P1.D11", "P1.D12", "P1.D21", "P1.D22"],
      prefills: [1, 2].map(n => ({ instance: `P1.Zone${n}`, parameterRecord: `P1.Zone${n}Parameters`, manipulatedVariable: `P1.Request${n}Pct`,
        setpoint: `P1.Zone${n}SetpointC`, processValue: `P1.Zone${n}C`, manualOutput: `P1.Zone${n}ManualPct` })) });
    try {
      view.set("structure", "group");
      for (const index of [0, 1]) {
        const box = view.root.querySelector<HTMLInputElement>(`[data-setup-field="group.${index}.include"]`)!;
        box.checked = true; box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
        for (const [field, value] of [["processVariable.unit", "degC"], ["processVariable.minimum", "0"], ["processVariable.maximum", "100"],
          ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"], ["manipulatedVariable.maximum", "100"], ["experimentStep", "10"]]) {
          view.set(`group.${index}.${field}`, value);
        }
      }
      for (const [field, value] of [["name", "Zones"], ["group.mode", "P1.ZonesAuto"], ["group.experimentDuration", "120"]]) view.set(field, value);
      const decoupled = view.root.querySelector<HTMLInputElement>('[data-setup-field="group.decoupled"]')!;
      decoupled.checked = true; decoupled.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      decoupler.forEach((row, i) => row.forEach((symbol, j) => view.set(`group.decoupler.${i}.${j}`, symbol)));
      const message = () => view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "";
      assert.match(message(), /P1\.Zone1.*plant input.*P1\.Request1Pct/, `the prefilled controller output is named: ${message()}`);
      assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.disabled, true);
      view.set("group.0.manipulatedVariable", "P1.Heater1Pct"); view.set("group.1.manipulatedVariable", "P1.Heater2Pct");
      assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.disabled, false, message());
    } finally { view.close(); }
  });

  test("sets up the decoupler's variables with the group in the form", function () {
    // About thirty fields through JSDOM, each re-rendering the form.
    this.timeout(10000);
    const view = open({ structure: "group", addressedReal: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone1ManualPct", "P1.Zone1SetpointC",
      "P1.Zone2C", "P1.Zone2ManualPct", "P1.Zone2SetpointC"], booleans: ["P1.ZonesAuto"], modes: ["P1.ZonesAuto"],
      records: ["P1.Zone1Parameters", "P1.Zone2Parameters"], writes: [], initializes: [], instances: ["P1"],
      pids: [{ instance: "P1.Zone1", task: { name: "MainTask", intervalMs: 100 } }, { instance: "P1.Zone2", task: { name: "MainTask", intervalMs: 100 } }],
      reals: ["P1.D11", "P1.D12", "P1.D21", "P1.D22", "P1.Heater1Pct", "P1.Heater2Pct", "P1.Zone1C", "P1.Zone2C"] });
    try {
      view.set("structure", "group");
      for (const index of [0, 1]) {
        const n = index + 1;
        for (const [field, value] of [["include", "on"], ["parameterRecord", `P1.Zone${n}Parameters`], ["processVariable", `P1.Zone${n}C`],
          ["processVariable.unit", "degC"], ["processVariable.minimum", "0"], ["processVariable.maximum", "100"], ["manipulatedVariable", `P1.Heater${n}Pct`],
          ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"], ["manipulatedVariable.maximum", "100"], ["setpoint", `P1.Zone${n}SetpointC`],
          ["manualOutput", `P1.Zone${n}ManualPct`], ["experimentStep", "10"]]) {
          if (field === "include") { const box = view.root.querySelector<HTMLInputElement>(`[data-setup-field="group.${index}.include"]`)!; box.checked = true; box.dispatchEvent(new view.dom.window.Event("change", { bubbles: true })); }
          else view.set(`group.${index}.${field}`, value);
        }
      }
      for (const [field, value] of [["name", "Zones"], ["group.mode", "P1.ZonesAuto"], ["group.experimentDuration", "120"]]) view.set(field, value);
      const box = view.root.querySelector<HTMLInputElement>('[data-setup-field="group.decoupled"]');
      assert.ok(box, "the group offers a decoupler");
      box!.checked = true; box!.dispatchEvent(new view.dom.window.Event("change", { bubbles: true }));
      const message = () => view.root.querySelector<HTMLElement>("[data-setup-message]")!.textContent ?? "";
      assert.match(message(), /decoupler/i, `the decoupler's variables are next: ${message()}`);
      decoupler.forEach((row, i) => row.forEach((symbol, j) => view.set(`group.decoupler.${i}.${j}`, symbol)));
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, message());
      save.click();
      assert.deepStrictEqual(view.posts[0]?.draft?.decoupler, decoupler);
    } finally { view.close(); }
  });
});
