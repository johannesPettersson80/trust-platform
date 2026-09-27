import * as assert from "assert";
import { initializeMultiSelect } from "../../webview/multiSelect";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

type MultiSelect = {
  trigger: HTMLButtonElement;
  setDisabled(disabled: boolean): void;
};

function withSelect(callback: (dom: any, select: HTMLSelectElement, multiSelect: MultiSelect) => void): void {
  const dom = new JSDOM(`<main><label for="signals">Plant inputs</label><select id="signals" multiple size="1" aria-label="Plant inputs"><option value="mv" data-unit="%">Heater output</option><option value="pv" data-unit="°C" selected>Process value</option><option value="load" data-unit="%">Load</option></select></main>`);
  dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  dom.window.HTMLDialogElement.prototype.close = function () { this.open = false; };
  const previousDocument = (globalThis as any).document;
  const previousWindow = (globalThis as any).window;
  (globalThis as any).document = dom.window.document;
  (globalThis as any).window = dom.window;
  try {
    const select = dom.window.document.querySelector("select") as HTMLSelectElement;
    callback(dom, select, initializeMultiSelect(select));
  } finally {
    (globalThis as any).document = previousDocument;
    (globalThis as any).window = previousWindow;
    dom.window.close();
  }
}

suite("Control Studio shared signal multi-select", () => {
  test("shows selected names and units and applies checkbox choices immediately", () => {
    withSelect((dom, select, multiSelect) => {
      assert.strictEqual(multiSelect.trigger.textContent, "Process value (°C)");
      assert.strictEqual(select.hidden, true, "The native control is hidden while retaining the selected values");
      assert.strictEqual(select.tabIndex, -1);
      assert.ok(multiSelect.trigger.id && dom.window.document.querySelector(`label[for="${multiSelect.trigger.id}"]`), "the existing label must target the compact trigger");
      multiSelect.trigger.click();
      const dialog = dom.window.document.querySelector("[data-multi-select-dialog]") as HTMLDialogElement;
      assert.strictEqual(dialog.open, true);
      const load = dialog.querySelector<HTMLInputElement>("[data-multi-select-option='load']")!;
      load.click();
      assert.strictEqual(select.options[2].selected, true, "A checkbox selection applies immediately");
      assert.strictEqual(dialog.querySelector("[data-multi-select-done],[data-multi-select-cancel]"), null, "A dropdown needs no Done/Cancel actions");
      assert.match(multiSelect.trigger.textContent ?? "", /Process value \(°C\)/);
      assert.match(multiSelect.trigger.textContent ?? "", /Load \(%\)/);
    });
  });

  test("closes on Escape without undoing selection and restores focus", () => {
    withSelect((dom, select, multiSelect) => {
      multiSelect.trigger.focus();
      multiSelect.trigger.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: " ", bubbles: true }));
      const dialog = dom.window.document.querySelector("[data-multi-select-dialog]") as HTMLDialogElement;
      assert.strictEqual(dialog.open, true);
      const load = dialog.querySelector<HTMLInputElement>("[data-multi-select-option='load']")!;
      load.focus(); load.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: " ", bubbles: true }));
      dialog.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      assert.strictEqual(select.options[2].selected, true);
      assert.strictEqual(dom.window.document.activeElement, multiSelect.trigger, "Escape returns focus to the trigger");
    });
  });

  test("toggles closed and dismisses on an outside pointer without blocking the page", () => {
    withSelect((dom, _select, multiSelect) => {
      multiSelect.trigger.click();
      const dialog = dom.window.document.querySelector("[data-multi-select-dialog]") as HTMLDialogElement;
      assert.strictEqual(dialog.open, true);
      multiSelect.trigger.click();
      assert.strictEqual(dialog.open, false, "Clicking the trigger again closes the dropdown");
      multiSelect.trigger.dispatchEvent(new dom.window.KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
      assert.strictEqual(dialog.open, true, "ArrowDown opens the choices");
      dom.window.document.body.dispatchEvent(new dom.window.Event("pointerdown", { bubbles: true }));
      assert.strictEqual(dialog.open, false, "Clicking outside closes the dropdown");
    });
  });

  test("disables the trigger and popup while the parent operation is busy", () => {
    withSelect((_dom, _select, multiSelect) => {
      multiSelect.setDisabled(true);
      assert.strictEqual(multiSelect.trigger.disabled, true);
      multiSelect.setDisabled(false);
      assert.strictEqual(multiSelect.trigger.disabled, false);
    });
  });
});
