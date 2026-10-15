import * as assert from "assert";
import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";

/** A served loop's answer to the Recorded-data fit (`ManualIdentificationAnswer`, B3.3), with the fields the page reads. */
function answerOf(changes: Record<string, unknown>): Record<string, unknown> {
  return { status: "complete", nextFitRole: "estimation", hasRecording: true, primaryAction: "checkRecording",
    primaryActionLabel: "Check recording", acceptanceReady: false, phase: "recording-selected", ...changes };
}

type Posted = Array<{ kind: string; payload?: Record<string, unknown> }>;

/** Runs `exercise` on a served loop's Recorded-data fit, in a template DOM whose commands are recorded in `posted`. */
async function served(exercise: (root: HTMLElement, identification: ThreePageIdentification, posted: Posted) => void | Promise<void>): Promise<void> {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main><button data-engineer-find></button>${renderThreePageIdentificationTemplate()}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const posted: Posted = [];
    const identification = new ThreePageIdentification(root, (kind, payload) => { posted.push({ kind, payload }); });
    identification.setEngineServed(true);
    identification.setSelectedMethod(true);
    await exercise(root, identification, posted);
  } finally { Object.assign(globalThis, original); dom.window.close(); }
}

suite("Control Studio Recorded-data fit on a loop the engine serves", () => {
  test("shows each stage of the engine's answer with its label and message, and sends the command the answer names", async () => {
    await served((root, identification, posted) => {
      const button = root.querySelector<HTMLButtonElement>("[data-engineer-find]")!;
      const status = root.querySelector<HTMLElement>("[data-three-page-identification-status]")!;
      const show = (changes: Record<string, unknown>) => {
        identification.receive("identify", answerOf(changes));
        posted.length = 0;
      };
      const pressed = () => {
        identification.primaryAction();
        return posted.map(message => message.kind);
      };

      show({ primaryAction: "checkRecording", primaryActionLabel: "Check recording" });
      assert.strictEqual(button.textContent, "Check recording", "the label is the engine's");
      assert.deepStrictEqual(pressed(), ["control-studio.identify.run"]);
      assert.strictEqual(Object.keys(posted[0]?.payload ?? {}).join(","), "form", "a check sends the form as the page holds it, and no settings of its own");

      show({ phase: "qualified", primaryAction: "fitPlant", primaryActionLabel: "Fit plant model", message: "Recording qualified. Fit plant model." });
      assert.strictEqual(button.textContent, "Fit plant model");
      assert.strictEqual(status.textContent, "Recording qualified. Fit plant model.", "the status is the engine's message");
      assert.deepStrictEqual(pressed(), ["control-studio.identify.fit"]);

      show({ phase: "validation_required", primaryAction: "chooseStructure", primaryActionLabel: "Keep first-order model", structureChoice: "first_order",
        message: "First order meets the limits; the extra state is not justified." });
      assert.strictEqual(button.textContent, "Keep first-order model");
      assert.deepStrictEqual(pressed(), ["control-studio.identify.choose-structure"]);
      assert.deepStrictEqual(posted[0]?.payload, { choice: "first_order" }, "the structure the engine keeps is the one it names");

      show({ phase: "acceptance_required", primaryAction: "accept", primaryActionLabel: "Validation not qualified", acceptanceReady: false });
      assert.strictEqual(button.textContent, "Validation not qualified");
      assert.strictEqual(button.disabled, true, "an acceptance the engine does not allow is disabled");
      assert.deepStrictEqual(pressed(), [], "and sends nothing");

      show({ phase: "acceptance_required", primaryAction: "accept", primaryActionLabel: "Use fitted model", acceptanceReady: true });
      assert.strictEqual(button.disabled, false);
      assert.deepStrictEqual(pressed(), ["control-studio.identify.accept"]);

      show({ phase: "accepted", primaryAction: "modelReady", primaryActionLabel: "Model ready" });
      assert.strictEqual(button.textContent, "Model ready");
      assert.strictEqual(button.disabled, true, "an accepted model has nothing to request");
      assert.deepStrictEqual(pressed(), []);

      identification.receive("identify", answerOf({ status: "failed", reason: "Qualify a suitable recording before fitting a model.",
        primaryAction: "checkRecording", primaryActionLabel: "Check recording" }));
      assert.strictEqual(status.textContent, "Qualify a suitable recording before fitting a model.", "a refusal shows its reason");
    });
  });

  test("sends a changed form as text once after the debounce, so a burst of typing is one edit the engine answers", async () => {
    await served(async (root, _identification, posted) => {
      const measurement = root.querySelector<HTMLInputElement>('[data-three-page-identification-setting="measurementBound"]')!;
      const view = root.ownerDocument.defaultView!;
      for (const value of ["0.0", "0.05"]) {
        measurement.value = value;
        measurement.dispatchEvent(new view.Event("input"));
      }
      // `length` rather than deepStrictEqual: the assertion would narrow `posted` to `never[]` for the lines after it.
      assert.strictEqual(posted.length, 0, "the change waits for the debounce");
      await new Promise(resolve => setTimeout(resolve, 400));
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.edit"]);
      const form = posted[0].payload?.form as { values: Record<string, string> };
      assert.strictEqual(form.values.measurementBound, "0.05", "the form carries the text the page holds");
    });
  });

  test("shows a refusal under the form field the engine names, and clears it with the next answer", async () => {
    await served((root, identification) => {
      identification.receive("identify", { status: "failed", reason: "Measurement bound must be positive.", field: "measurementBound" });
      const field = root.querySelector<HTMLInputElement>('[data-three-page-identification-setting="measurementBound"]')!;
      const note = root.querySelector<HTMLElement>("[data-three-page-identification-refusal]");
      assert.ok(note, "the refusal is shown under its field");
      assert.strictEqual(note.textContent, "Measurement bound must be positive.");
      assert.strictEqual(note.previousElementSibling, field.closest("label") ?? field, "the refusal follows the field it names");
      assert.strictEqual(field.getAttribute("aria-invalid"), "true");
      assert.strictEqual(root.querySelector("[data-three-page-identification-status]")!.textContent, "Measurement bound must be positive.");
      identification.receive("identify", answerOf({ message: "" }));
      assert.strictEqual(root.querySelector("[data-three-page-identification-refusal]"), null, "a new answer removes the old refusal");
      assert.strictEqual(field.getAttribute("aria-invalid"), null);
    });
  });
});
