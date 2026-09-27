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
  function setup(payload: Record<string, unknown>, definition = loop) {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const controls = initializeManualTuneSettings(root, definition, undefined);
    controls.render(payload);
    return { dom, root, controls, restore() { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); } };
  }

  test("manual PI exposes the declared anti-windup gain and retains its pin", () => {
    const definition = structuredClone(loop);
    definition.controller.law.antiWindup = { method: "back-calculation", trackingGainSymbol: "P1.TrackingGain" };
    definition.parameterBinding.parameters.push({ symbol: "P1.TrackingGain", iec_type: "LREAL", unit: "1/s", convention: "Anti-windup tracking gain", minimum: 0, maximum: 10 });
    const f = setup({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 2,
      "P1.Parameters.IntegralGain": 0.5, "P1.TrackingGain": 1 }, settingsPins: { values: { "P1.TrackingGain": 1 } } }, definition);
    try {
      assert.ok(f.root.querySelector("[data-studio-manual-value='P1.TrackingGain']"), "PI requires its declared anti-windup setting");
      assert.strictEqual(f.controls.read().values["P1.TrackingGain"], 1);
      assert.strictEqual(f.controls.read().pinnedValues["P1.TrackingGain"], 1);
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
      initializeManualTuneSettings(dom.window.document.querySelector("main") as HTMLElement, loop).render(payload);
      assert.ok(area.querySelector("[data-studio-manual-settings]"), "Tune result needs a manual settings editor");
      assert.strictEqual(area.querySelectorAll("input[data-studio-manual-value]").length, 3);
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.IntegralGain']") as HTMLInputElement)?.value, "0.5");
      assert.strictEqual((area.querySelector("[data-studio-manual-pin='P1.Parameters.IntegralGain']") as HTMLInputElement)?.checked, true);
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
      const pin = f.root.querySelector<HTMLInputElement>("[data-studio-manual-pin='P1.Parameters.IntegralGain']")!;
      pin.checked = true;
      assert.deepStrictEqual(f.controls.read().pinnedValues, { "P1.Parameters.IntegralGain": 0.500000001 });
    } finally { f.restore(); }
  });
});
