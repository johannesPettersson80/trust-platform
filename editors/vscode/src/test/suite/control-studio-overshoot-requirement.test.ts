import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import { editControlRequirements } from "../../controlStudio/webview/threePageRequirements";
import { validateReceiver } from "../../controlStudio/webview/engineValidate";

const { JSDOM } = require("jsdom");

/** The Requirements editor's single-output values, which the existing editor tests save without error. */
const REQUIREMENTS = { referenceChange: 5, maximumError: 1, settlingBand: 0.2, settlingTimeSeconds: 8, durationSeconds: 16 };

function fixtureLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
}

/** Runs one check on a fresh page, restoring the globals that the webview modules read. */
function withPage(markup: string, check: (root: HTMLElement, window: any) => void): void {
  const dom = new JSDOM(`<main>${markup}</main>`);
  const { window } = dom;
  const saved = {
    document: (globalThis as any).document, window: (globalThis as any).window, HTMLElement: (globalThis as any).HTMLElement,
    CustomEvent: (globalThis as any).CustomEvent, Option: (globalThis as any).Option,
  };
  Object.assign(globalThis, { document: window.document, window, HTMLElement: window.HTMLElement, CustomEvent: window.CustomEvent, Option: window.Option });
  // jsdom has no modal dialog; the editor needs only the open state and the close event.
  window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new window.Event("close")); };
  try {
    check(window.document.querySelector("main") as HTMLElement, window);
  } finally {
    Object.assign(globalThis, saved);
    window.close();
  }
}

function control<T extends Element = HTMLElement>(scope: ParentNode, selector: string): T {
  const found = scope.querySelector<T>(selector);
  assert.ok(found, `missing ${selector}`);
  return found!;
}

function submitRequirements(window: any): void {
  const form = window.document.querySelector("[data-trust-compact-editor] form") as HTMLFormElement | null;
  assert.ok(form, "the requirements editor is open");
  form!.dispatchEvent(new window.Event("submit", { cancelable: true }));
}

function messageOf(input: HTMLElement): string {
  const notice = input.closest(".trust-field")?.querySelector<HTMLElement>(".trust-field__message");
  return notice && !notice.hidden ? notice.textContent ?? "" : "";
}

suite("Control Studio maximum overshoot requirement", () => {
  test("shows an overshoot result in the engine's words, in its own row of the Validate table", () => {
    // The engine's table holds the row and its percent text (ValidateResult.table). The page writes those words and computes none of them.
    withPage(`<table data-studio-validation-result><tbody></tbody></table>`, root => {
      validateReceiver.result!(root, { applyAvailable: false, table: [{ title: "Maximum overshoot · Main.PV · setpoint", symbol: "Main.PV",
        source: "", measured: "3 %", limit: "≤ 5 %", verdict: "pass", verdictText: "Pass", verdictClass: "pass", requirement: "overshoot" }] },
      { receive: () => undefined } as any);
      const row = root.querySelector<HTMLElement>("tr[data-studio-requirement='overshoot']");
      assert.ok(row, "the overshoot result has its own row");
      const cells = Array.from(row!.querySelectorAll("td")).map(cell => cell.textContent ?? "");
      assert.match(cells[0] ?? "", /Maximum overshoot/);
      assert.deepStrictEqual(cells.slice(1), ["3 %", "≤ 5 %", "Pass"]);
    });
  });

  test("the Requirements editor shows a blank Maximum overshoot row, saves it blank, and keeps other requirements on save", () => {
    withPage("", (_root, window) => {
      const commits: any[] = [];
      // The engine's refusal of a requirement reaches the editor as its notes (the workflow passes it); no test here runs the engine.
      const open = (current: Record<string, unknown>, refusal?: { field: string; message: string }) =>
        editControlRequirements(fixtureLoop(), current as any, next => commits.push(next), undefined, undefined, refusal ? { refusal } : {});
      const field = (key: string) => control<HTMLInputElement>(window.document, `[data-studio-requirement='${key}']`);
      const cancel = () => control<HTMLButtonElement>(window.document, "[data-studio-editor-cancel]").click();

      open({ ...REQUIREMENTS });
      const overshoot = field("maximumOvershootPercent");
      assert.strictEqual(overshoot.value, "", "an undeclared limit leaves the row empty");
      assert.strictEqual(overshoot.required, false, "a blank limit is accepted");
      assert.strictEqual(overshoot.closest(".trust-field")?.querySelector("label")?.textContent, "Maximum overshoot (% of setpoint change)");
      assert.strictEqual(overshoot.closest("details"), null, "the row is visible, not under More…");
      submitRequirements(window);
      assert.strictEqual(commits.length, 1, "a blank row saves");
      assert.ok(!("maximumOvershootPercent" in commits[0]), "a blank row declares no limit");

      open({ ...REQUIREMENTS, maximumOvershootPercent: 4 });
      assert.strictEqual(field("maximumOvershootPercent").value, "4", "a declared limit fills its row");
      field("maximumOvershootPercent").value = "-1";
      field("maximumError").value = "";
      submitRequirements(window);
      // The page checks no entry and no range (B4.3): a blank required row and a negative limit go to the engine as typed, and the engine
      // refuses each under its field (`inputs_change.rs`, `requirements.rs`).
      assert.strictEqual(commits.length, 2, "the entries are sent to the engine as typed");
      assert.strictEqual(commits[1].maximumError, "", "a blank required value goes as typed");
      assert.strictEqual(commits[1].maximumOvershootPercent, -1, "a negative limit goes as typed");
      // The engine's refusal reopens the editor with the rows as they were typed and its sentence under the field it names.
      open({ ...REQUIREMENTS, maximumError: "", maximumOvershootPercent: -1 },
        { field: "maximumError", message: "Enter the error limit as a number." });
      assert.strictEqual(field("maximumError").value, "", "the refused row is shown as it was typed");
      assert.match(messageOf(field("maximumError")), /as a number/, "the engine's refusal of the blank row shows under its field");
      cancel();
      open({ ...REQUIREMENTS, maximumOvershootPercent: -1 },
        { field: "maximumOvershootPercent", message: "Enter an overshoot limit above 0 %, or leave it blank for no limit." });
      assert.match(messageOf(field("maximumOvershootPercent")), /above 0/, "the engine's refusal shows under its field");
      cancel();

      open({ ...REQUIREMENTS, constantInputDisturbance: 0.3, maximumOvershootPercent: 4 });
      field("maximumOvershootPercent").value = "6";
      submitRequirements(window);
      const changed = commits[commits.length - 1];
      assert.strictEqual(changed.maximumOvershootPercent, 6);
      assert.deepStrictEqual([changed.referenceChange, changed.maximumError, changed.settlingBand, changed.settlingTimeSeconds,
        changed.durationSeconds, changed.constantInputDisturbance], [5, 1, 0.2, 8, 16, 0.3], "every other declared requirement is saved unchanged");

      open({ ...REQUIREMENTS, constantInputDisturbance: 0.3, maximumOvershootPercent: 4 });
      field("maximumOvershootPercent").value = "";
      submitRequirements(window);
      const cleared = commits[commits.length - 1];
      assert.ok(!("maximumOvershootPercent" in cleared), "clearing the row removes the declared limit");
      assert.strictEqual(cleared.constantInputDisturbance, 0.3, "clearing the limit keeps the load trial");
    });
  });
});
