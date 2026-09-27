import * as assert from "assert";
const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => { window: Window & typeof globalThis } };
import { historyShortcutAction, renderDesignComparisonPins, stableComparisonKey } from "../../controlStudio/webview/designComparisonPresentation";

suite("Control Studio pinned design comparison presentation", () => {
  test("renders selectable/removable pin metadata and preserves unavailable context", () => {
    const dom = new JSDOM(`<div data-pinned-designs></div>`);
    const host = dom.window.document.querySelector<HTMLElement>("[data-pinned-designs]")!;
    const selected: number[] = [];
    const removed: number[] = [];
    renderDesignComparisonPins(host, [
      { revision: 4, structure: "P", proportionalGain: 10, source: "SIMC", validation: "Pass", available: true },
      { revision: 3, structure: "PI", proportionalGain: 8, source: "Imported", validation: "Unavailable", available: false, reason: "Model revision changed." },
    ], 4, revision => { if (revision !== undefined) selected.push(revision); }, revision => removed.push(revision));
    assert.match(host.textContent ?? "", /P/);
    assert.match(host.textContent ?? "", /Kp 10/);
    assert.match(host.textContent ?? "", /SIMC/);
    assert.match(host.textContent ?? "", /Pass/);
    assert.match(host.textContent ?? "", /Unavailable/);
    assert.match(host.textContent ?? "", /Model revision changed/);
    host.querySelector<HTMLButtonElement>('[data-pinned-select="4"]')!.click();
    host.querySelector<HTMLButtonElement>('[data-pinned-remove="4"]')!.click();
    host.querySelector<HTMLButtonElement>('[data-pinned-return="current"]')?.click();
    assert.deepStrictEqual(selected, [4]);
    assert.deepStrictEqual(removed, [4]);
  });

  test("accepts history shortcuts only outside editable controls", () => {
    const dom = new JSDOM(`<main><input><div contenteditable="true"></div><button></button></main>`);
    const input = dom.window.document.querySelector("input")!;
    const editable = dom.window.document.querySelector("[contenteditable]")!;
    const button = dom.window.document.querySelector("button")!;
    const event = (target: EventTarget, key: string, shiftKey = false) => {
      const value = new dom.window.KeyboardEvent("keydown", { key, ctrlKey: true, shiftKey, bubbles: true });
      Object.defineProperty(value, "target", { value: target });
      return value;
    };
    assert.strictEqual(historyShortcutAction(event(button, "z")), "undo");
    assert.strictEqual(historyShortcutAction(event(button, "z", true)), "redo");
    assert.strictEqual(historyShortcutAction(event(input, "z")), undefined);
    assert.strictEqual(historyShortcutAction(event(editable, "z")), undefined);
  });

  test("keeps distinct candidate cache keys inside one context and changes them across context", () => {
    const context = { modelRevision: "model-1", evidenceRevision: "evidence-1", method: "SIMC" };
    const candidateA = { context, parameters: { proportionalGain: 1, integralGain: 0 } };
    const candidateB = { context, parameters: { proportionalGain: 10, integralGain: 0 } };
    assert.notStrictEqual(stableComparisonKey(candidateA.parameters), stableComparisonKey(candidateB.parameters));
    assert.notStrictEqual(stableComparisonKey(candidateA.context), stableComparisonKey({ ...context, modelRevision: "model-2" }));
  });
});
