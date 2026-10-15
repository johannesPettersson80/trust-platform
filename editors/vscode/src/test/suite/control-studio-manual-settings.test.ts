import * as assert from "assert";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

const loop = {
  controller: {
    instance: "P1.Controller",
    type: "Control.PID",
    law: {
      kind: "pid",
      parameterRecordSymbol: "P1.Parameters",
      parameterRevisionSymbol: "P1.Parameters.Revision",
      proportionalGainSymbol: "P1.Parameters.ProportionalGain",
      integralGainSymbol: "P1.Parameters.IntegralGain",
      derivativeGainSymbol: "P1.Parameters.DerivativeGain",
    },
  },
  parameterBinding: {
    parameters: [
      { symbol: "P1.Parameters.ProportionalGain", iec_type: "LREAL", unit: "%/degC", convention: "Kp", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.IntegralGain", iec_type: "LREAL", unit: "%/(degC*s)", convention: "Ki", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.DerivativeGain", iec_type: "LREAL", unit: "%*s/degC", convention: "Kd", minimum: 0, maximum: 100 },
    ],
  },
} as any;

suite("Control Studio manual Tune settings", () => {
  test("labels a pinned finite design without presenting nominal Riccati gains as its active policy", () => {
    const f = setup({ status: "candidate", family: "LQR", values: {} });
    try {
      renderTunedController(f.root, loop, { status: "candidate", family: "LQR", values: {},
        design: { horizon: { steps: 1, policy: "pinned-fixed-gain", gainSequence: [[[.5]]], riccatiSequence: [[[1.5]], [[1]]] } } });
      assert.match(f.root.querySelector("[data-studio-design-policy]")!.textContent!, /Pinned fixed gain/);
      assert.match(f.root.querySelector("details summary")!.textContent!, /Nominal/);
      assert.strictEqual(f.root.querySelector<HTMLDetailsElement>("details")!.open, false);
    } finally { f.restore(); }
  });
  test("shows a dimensionless tuned value without a unit", () => {
    const lqg = { controller: { instance: "P1", type: "LQG state feedback" }, parameterBinding: { parameters: [
      { symbol: "P1.K1", iec_type: "LREAL", unit: "m3/s/m", convention: "Gain K1", minimum: -1, maximum: 1 },
      { symbol: "P1.L1", iec_type: "LREAL", unit: "1", convention: "Estimator gain L1", minimum: -5, maximum: 5 }] } } as any;
    const f = setup({ status: "candidate", family: "LQG", values: {} }, lqg);
    try {
      renderTunedController(f.root, lqg, { status: "candidate", family: "LQG", values: { "P1.K1": 0.07874, "P1.L1": 0.2393 } });
      assert.strictEqual(f.root.querySelector("[data-studio-parameter='P1.L1']")!.textContent, "0.2393");
      assert.match(f.root.querySelector("[data-studio-parameter='P1.K1']")!.textContent!, /^0\.0787 \S/);
    } finally { f.restore(); }
  });

  test("shows every Riccati stage's gain and cost with one precision per matrix", () => {
    const f = setup({ status: "candidate", family: "LQR", values: {} });
    try {
      renderTunedController(f.root, loop, { status: "candidate", family: "LQR", values: {},
        design: { horizon: { steps: 2, policy: "repeat-first-finite-gain", gainSequence: [[[0.07841259, 0.02451445]], [[0.01815348, 0.00036036]]],
          riccatiSequence: [[[1948.2568, 554.3539], [554.3539, 4065.8254]], [[756.1485, 14.4981], [14.4981, 769.2411]], [[400, 0], [0, 400]]] } } });
      const rows = [...f.root.querySelectorAll("[data-studio-riccati-stage]")].map(row => [...(row as HTMLTableRowElement).cells].map(cell => cell.textContent));
      assert.deepStrictEqual(rows, [
        ["0", "[0.0784, 0.0245]", "[1948.3, 554.4]; [554.4, 4065.8]"],
        ["1", "[0.018153, 0.00036]", "[756.15, 14.5]; [14.5, 769.24]"],
        ["2", "—", "[400, 0]; [0, 400]"],
      ], "Equal entries read alike and no entry switches to exponent notation");
    } finally { f.restore(); }
  });
  function setup(payload: Record<string, unknown>, definition = loop, familySymbols?: (family: string) => readonly string[] | undefined) {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const controls = initializeManualTuneSettings(root, definition, undefined, familySymbols);
    controls.render(payload);
    return { dom, root, controls, restore() { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); } };
  }

  test("names a project controller's gains in Manual as its Automatic result names them", () => {
    const definition: any = { controller: { instance: "P1", type: "Main", design: "project", project: { seed: { kind: "trajectory-feedforward-pid", family: "PID",
      proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki", derivativeSymbol: "P1.Kd", rateSymbol: "P1.Kv", accelerationSymbol: "P1.Ka" } } },
      parameterBinding: { parameters: ["P1.Kp", "P1.Ki", "P1.Kd", "P1.Kv", "P1.Ka"].map(symbol => ({ symbol, iec_type: "LREAL", unit: "1",
        convention: `raw ${symbol}`, minimum: 0, maximum: 100 })) } };
    const f = setup({ status: "candidate", family: "PID", values: { "P1.Kp": 18, "P1.Ki": 12, "P1.Kd": 7, "P1.Kv": 1, "P1.Ka": 1 } }, definition);
    try {
      assert.deepStrictEqual(Array.from(f.root.querySelectorAll("[data-studio-manual-settings] label[for]")).map(label => label.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Derivative gain (Kd)", "Rate feedforward (Kv)", "Acceleration feedforward (Ka)"]);
    } finally { f.restore(); }
  });

  test("manual PI exposes the declared anti-windup gain without a Pin checkbox", () => {
    const definition = structuredClone(loop);
    definition.controller.law.antiWindup = { method: "back-calculation", trackingGainSymbol: "P1.TrackingGain" };
    definition.parameterBinding.parameters.push({ symbol: "P1.TrackingGain", iec_type: "LREAL", unit: "1/s", convention: "Anti-windup tracking gain", minimum: 0, maximum: 10 });
    const f = setup({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 2,
      "P1.Parameters.IntegralGain": 0.5, "P1.TrackingGain": 1 }, settingsPins: { values: { "P1.TrackingGain": 1 } } }, definition);
    try {
      assert.ok(f.root.querySelector("[data-studio-manual-value='P1.TrackingGain']"), "PI requires its declared anti-windup setting");
      assert.strictEqual(f.controls.read().values["P1.TrackingGain"], 1);
      assert.strictEqual(f.root.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
    } finally { f.restore(); }
  });

  test("project-owned PID compositions expose all declared filter and reference settings", () => {
    const definition = structuredClone(loop);
    definition.controller.project = { family: "PID" };
    delete definition.controller.law;
    definition.parameterBinding.parameters.push({ symbol: "P1.FilterTime", iec_type: "LREAL", unit: "s", convention: "Measurement filter time constant", minimum: 0, maximum: 10 });
    const f = setup({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 2,
      "P1.Parameters.IntegralGain": 0.5, "P1.Parameters.DerivativeGain": 0, "P1.FilterTime": 0.25 } }, definition);
    try {
      assert.ok(f.root.querySelector("[data-studio-manual-value='P1.FilterTime']"), "The project adapter owns the full composed controller");
      assert.strictEqual(f.controls.read().values["P1.FilterTime"], 0.25);
    } finally { f.restore(); }
  });

  test("shows editable adapter settings and explicit pin controls in the result area", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    try {
      const area = dom.window.document.querySelector("[data-studio-tuning-result]") as HTMLElement;
      const payload = {
      status: "candidate", family: "PID",
      values: {
        "P1.Parameters.ProportionalGain": 2,
        "P1.Parameters.IntegralGain": 0.5,
        "P1.Parameters.DerivativeGain": 0.1,
      },
      settingsPins: { values: { "P1.Parameters.IntegralGain": 0.5 } },
      };
      renderTunedController(dom.window.document.querySelector("main") as HTMLElement, loop, payload);
      // An automatic result offers a Keep toggle per calculated setting; the pinned one starts kept.
      assert.strictEqual((area.querySelector("[data-studio-keep='P1.Parameters.IntegralGain']") as HTMLInputElement)?.checked, true);
      initializeManualTuneSettings(dom.window.document.querySelector("main") as HTMLElement, loop).render(payload);
      assert.ok(area.querySelector("[data-studio-manual-settings]"), "Tune result needs a manual settings editor");
      assert.strictEqual(area.querySelectorAll("input[data-studio-manual-value]").length, 3);
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.IntegralGain']") as HTMLInputElement)?.value, "0.5");
      assert.strictEqual(area.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
      assert.match(area.textContent ?? "", /%\/°C/);
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      dom.window.close();
    }
  });

  test("uses only the selected family, keeps full numeric precision, and avoids extra headings", () => {
    const candidate = { status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2.123456789,
      "P1.Parameters.IntegralGain": 0.500000001,
      "P1.Parameters.DerivativeGain": 0.1,
    } };
    const f = setup(candidate);
    try {
      const area = f.root.querySelector("[data-studio-tuning-result]")!;
      assert.strictEqual(area.querySelectorAll("input[data-studio-manual-value]").length, 2);
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.ProportionalGain']") as HTMLInputElement).value, "2.123456789");
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.IntegralGain']") as HTMLInputElement).value, "0.500000001");
      assert.strictEqual(area.querySelector("legend"), null);
      assert.strictEqual(area.querySelector("small"), null);
    } finally { f.restore(); }
  });

  test("rejects an invalid value with its field and declared bounds", () => {
    const f = setup({ status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5,
    } });
    try {
      const field = f.root.querySelector<HTMLInputElement>("[data-studio-manual-value='P1.Parameters.IntegralGain']")!;
      field.value = "";
      assert.throws(() => f.controls.read(), /integral gain.*0.*100/i);
      field.value = "0.500000001";
      assert.deepStrictEqual(f.controls.read().values, { "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.500000001 });
    } finally { f.restore(); }
  });

  test("names a rejected field as it is labelled, keeping the symbol's case", () => {
    const f = setup({ status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5,
    } });
    try {
      f.root.querySelector<HTMLInputElement>("[data-studio-manual-value='P1.Parameters.ProportionalGain']")!.value = "150";
      assert.throws(() => f.controls.read(), (error: Error) => error.message === "Enter proportional gain (Kp) between 0 and 100.");
    } finally { f.restore(); }
  });

  test("a served program's manual form asks only for the symbols the engine names for the chosen family: family P on a PI seed asks for Kp alone (C2)", () => {
    const definition: any = { controller: { instance: "P1", type: "Main", design: "project", project: { family: "PI", seed: { kind: "parallel-pid", family: "PI",
      proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki" } } },
      parameterBinding: { parameters: [
        { symbol: "P1.Kp", iec_type: "REAL", unit: "%/degC", convention: "Kp", minimum: 0, maximum: 20 },
        { symbol: "P1.Ki", iec_type: "REAL", unit: "%/(degC*s)", convention: "Ki", minimum: 0, maximum: 10 }] } };
    const named: Record<string, string[]> = { P: ["P1.Kp"], PI: ["P1.Kp", "P1.Ki"] };
    const f = setup({ status: "candidate", family: "P", values: { "P1.Kp": 2, "P1.Ki": 0.5 } }, definition, family => named[family]);
    try {
      const asked = () => Array.from(f.root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]"), input => input.dataset.studioManualValue);
      assert.deepStrictEqual(asked(), ["P1.Kp"], "family P asks for the proportional gain alone");
      assert.deepStrictEqual(f.controls.read(), { values: { "P1.Kp": 2 } }, "and reads only that setting");
      f.controls.render({ status: "candidate", family: "PI", values: { "P1.Kp": 2, "P1.Ki": 0.5 } });
      assert.deepStrictEqual(asked(), ["P1.Kp", "P1.Ki"], "family PI asks for both gains");
    } finally { f.restore(); }
  });
});
