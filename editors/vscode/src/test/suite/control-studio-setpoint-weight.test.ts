import * as assert from "assert";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const WEIGHT = "P1.Parameters.ProportionalSetpointWeight";

suite("Control Studio setpoint weight", () => {

  test("offers the setpoint weight beside the PI gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    // The engine's rows for the PI settings, each with its label and unit (C2): the weight is dimensionless, so its unit is "1".
    setOpenedOffers({ controllerOffer: { families: [], familySettings: {}, settingRows: [
      { symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)", unit: "%/degC", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)", unit: "%/(degC*s)", minimum: 0, maximum: 100 },
      { symbol: WEIGHT, label: "Setpoint weight (β)", unit: "1", minimum: 0, maximum: 1 }] } } as any);
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, undefined, family => family === "PI"
        ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain", WEIGHT] : undefined);
      controls.render({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5, [WEIGHT]: 1 } });
      const field = root.querySelector(`[data-studio-manual-value='${WEIGHT}']`);
      assert.ok(field, "PI shows the declared setpoint weight");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Setpoint weight (β)");
      assert.strictEqual(field.nextElementSibling.textContent, "", "a dimensionless weight shows no unit, not a stray 1");
      assert.strictEqual(controls.read().values[WEIGHT], "1", "the entry is read as typed");
    } finally { setOpenedOffers({}); (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });
});
