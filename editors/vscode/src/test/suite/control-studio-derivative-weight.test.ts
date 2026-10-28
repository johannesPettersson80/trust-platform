import * as assert from "assert";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const WEIGHT = "P1.Parameters.DerivativeSetpointWeight";
const KP = "P1.Parameters.ProportionalGain", KD = "P1.Parameters.DerivativeGain";

suite("Control Studio derivative setpoint weight", () => {
  test("offers the derivative weight beside the PD gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    // The engine's rows for the PD and P settings, each with its label and unit (C2): the weight is dimensionless, so its unit is "1".
    setOpenedOffers({ controllerOffer: { families: [], familySettings: {}, settingRows: [
      { symbol: KP, label: "Proportional gain (Kp)", unit: "%/degC", minimum: 0, maximum: 100 },
      { symbol: KD, label: "Derivative gain (Kd)", unit: "%*s/degC", minimum: 0, maximum: 100 },
      { symbol: WEIGHT, label: "Derivative setpoint weight (γ)", unit: "1", minimum: 0, maximum: 1 }] } } as any);
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, undefined, family => family === "PD" ? [KP, KD, WEIGHT]
        : family === "P" ? [KP] : undefined);
      controls.render({ status: "candidate", family: "PD", values: { [KP]: 16, [KD]: 7, [WEIGHT]: 0 } });
      const field = root.querySelector(`[data-studio-manual-value='${WEIGHT}']`);
      assert.ok(field, "PD shows the declared derivative setpoint weight");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Derivative setpoint weight (γ)");
      assert.strictEqual(field.nextElementSibling.textContent, "", "a dimensionless weight shows no unit");
      assert.strictEqual(controls.read().values[WEIGHT], "0", "the entry is read as typed");
      controls.render({ status: "candidate", family: "P", values: { [KP]: 16 } });
      assert.strictEqual(root.querySelector(`[data-studio-manual-value='${WEIGHT}']`), null, "P has no derivative term to weight");
    } finally { setOpenedOffers({}); (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });
});
