import * as assert from "assert";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
const FILTER = "P1.FilterTimeS";

suite("Control Studio measurement filter", () => {
  test("offers the filter time beside the controller gains", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    // The engine's rows for the PI settings, each with its label, unit and bounds (C2).
    setOpenedOffers({ controllerOffer: { families: [], familySettings: {}, settingRows: [
      { symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)", unit: "%/degC", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)", unit: "%/(degC*s)", minimum: 0, maximum: 100 },
      { symbol: FILTER, label: "Measurement filter time (Tf)", unit: "s", minimum: 0, maximum: 5 }] } } as any);
    try {
      const root = dom.window.document.querySelector("main");
      const controls = initializeManualTuneSettings(root, undefined, family => family === "PI"
        ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain", FILTER] : undefined);
      controls.render({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 13, "P1.Parameters.IntegralGain": 3.5, [FILTER]: 0 } });
      const field = root.querySelector(`[data-studio-manual-value='${FILTER}']`);
      assert.ok(field, "PI shows the declared measurement filter time");
      assert.strictEqual(root.querySelector(`label[for='${field.id}']`).textContent, "Measurement filter time (Tf)");
      assert.strictEqual(field.nextElementSibling.textContent, "s");
    } finally { setOpenedOffers({}); (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });
});
