import * as assert from "assert";
import { mountThreePageScenarios } from "../../controlStudio/webview/threePageScenarios";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** A delayed-update scenario: an event 1 s into the run. */
const scenario = { id: "delayed-update", initialValues: { "P1.SetpointC": 35 },
  events: [{ atMs: 0, symbol: "P1.DisturbanceKw", value: 30 }, { atMs: 1000, symbol: "P1.DelayUpdateS", value: 2 }],
  requirements: [{ id: "delayed-minimum", kind: "bounds", channel: "P1.TemperatureC", window: { start: 0, end: 60 }, minimum: 33, maximum: 80 }] };

suite("Control Studio scenario timing", () => {
  test("leaves a check beyond a changed validation duration to the engine, which refuses it under the duration field (B4.3)", () => {
    // The page keeps no duration gate and no live limit of its own: the engine refuses the change (`inputs_action_tests`).
    const dom = new JSDOM(`<main><section data-trust-compact-editor><form>
      <input data-studio-requirement="durationSeconds" type="number" value=""><div class="trust-compact-editor-actions"></div></form></section></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const dialog = dom.window.document.querySelector("[data-trust-compact-editor]");
      const editor = mountThreePageScenarios(dialog, { scenarioDefinitions: [scenario] });
      // The list the editor holds is what a commit sends to the engine, whatever the duration field holds.
      const sent = () => editor.read().map(item => item.id);
      assert.deepStrictEqual(sent(), ["delayed-update"], "an event at 1 s is not refused before a duration is entered");
      const duration = dialog.querySelector("[data-studio-requirement='durationSeconds']");
      duration.value = "60"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.deepStrictEqual(sent(), ["delayed-update"], "an event at 1 s is not refused in a 60 s validation");
      duration.value = "0.5"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.deepStrictEqual(sent(), ["delayed-update"], "a 0.5 s validation is not refused by the page: the engine answers the requirements change");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });

  test("does not refuse a check until 60 s in a 30 s validation on the page: the engine names the duration it needs (B4.3)", () => {
    // The refusal the engine gives (`checks ... until 60 s ... Allow a validation duration of at least 60 s.`) reaches the dialog
    // under the duration field; the page only shows it, and Rust pins the sentence (`inputs_action_tests`).
    const dom = new JSDOM(`<main><section data-trust-compact-editor><form>
      <input data-studio-requirement="durationSeconds" type="number" value="60"><div class="trust-compact-editor-actions"></div></form></section></main>`);
    const previousDocument = (globalThis as any).document, previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document; (globalThis as any).window = dom.window;
    try {
      const dialog = dom.window.document.querySelector("[data-trust-compact-editor]");
      const editor = mountThreePageScenarios(dialog, { scenarioDefinitions: [scenario] });
      const sent = () => editor.read().map(item => item.id);
      assert.deepStrictEqual(sent(), ["delayed-update"], "a check until 60 s lies inside a 60 s validation");
      const duration = dialog.querySelector("[data-studio-requirement='durationSeconds']");
      duration.value = "30"; duration.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.deepStrictEqual(sent(), ["delayed-update"], "a check until 60 s is not refused by the page; the commit reaches the engine");
    } finally { (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); }
  });
});
