import * as assert from "assert";
import { initializePidSettingsControls } from "../../controlStudio/webview/pidSettingsControls";

const valid = {
  proportionalGain: 1,
  integralGain: 0.1,
  derivativeGain: 0,
  proportionalSetpointWeight: 1,
  derivativeSetpointWeight: 0,
  bias: 0,
  outputMinimum: 0,
  outputMaximum: 100,
  rateMinimum: -100,
  rateMaximum: 100,
  trackingTime: 1,
  antiWindup: "BackCalculation",
};

suite("Control Studio PID settings validation", () => {
  test("Manual Generate rejects blank settings visibly without posting", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<select id="tune-method"><option>Manual</option></select>
      <button data-tune-generate>Generate candidate</button><p data-tune-state></p>
      ${Object.keys(valid).map(key => `<input data-pid-setting="${key}" value="${key === "antiWindup" ? valid[key] : ""}">`).join("")}`);
    const doc = dom.window.document as Document;
    const posts: unknown[] = [];
    const state = doc.querySelector("[data-tune-state]")!;
    initializePidSettingsControls({
      owner: doc,
      post: (_kind, payload) => posts.push(payload),
      setText: (_selector, text) => { state.textContent = text; },
      readManual: () => Object.fromEntries(Array.from(doc.querySelectorAll<HTMLInputElement>("[data-pid-setting]")).map(input => [input.dataset.pidSetting, input.dataset.pidSetting === "antiWindup" ? input.value : input.value.trim() === "" ? NaN : Number(input.value)])),
      imported: () => undefined,
    });
    doc.querySelector<HTMLButtonElement>("[data-tune-generate]")!.click();
    assert.deepStrictEqual(posts, []);
    assert.match(state.textContent ?? "", /finite number/i);
  });

  test("valid Manual Generate posts the complete Control.PID record", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<select id="tune-method"><option>Manual</option></select>
      <button data-tune-generate>Generate candidate</button><p data-tune-state></p>`);
    const doc = dom.window.document as Document;
    const posts: unknown[] = [];
    initializePidSettingsControls({
      owner: doc,
      post: (_kind, payload) => posts.push(payload),
      setText: () => undefined,
      readManual: () => valid,
      imported: () => undefined,
    });
    doc.querySelector<HTMLButtonElement>("[data-tune-generate]")!.click();
    assert.deepStrictEqual(posts, [{ method: "Manual", settings: valid }]);
  });
});
