import * as assert from "assert";
import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";
import { receiveEngineMessage } from "../../controlStudio/webview/engineOpened";
import { renderStepResponse } from "../../controlStudio/webview/stepResponseControls";
import type { StepResponseAnalysis, StepResponseMeasurement } from "../../controlStudio/generated/ManualIdentificationAnswer";

/** The engine's answer to the Recorded-data fit (`ManualIdentificationAnswer`, B3.3), with the fields the page reads. */
function answerOf(changes: Record<string, unknown>): Record<string, unknown> {
  return { status: "complete", nextFitRole: "estimation", hasRecording: true, primaryAction: "checkRecording",
    primaryActionLabel: "Check recording", acceptanceReady: false, phase: "recording-selected", ...changes };
}

type Posted = Array<{ kind: string; payload?: Record<string, unknown> }>;

/** Runs `exercise` on the Recorded-data fit, in a template DOM whose commands are recorded in `posted`. */
async function served(exercise: (root: HTMLElement, identification: ThreePageIdentification, posted: Posted) => void | Promise<void>): Promise<void> {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main><button data-engineer-find></button>${renderThreePageIdentificationTemplate()}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const posted: Posted = [];
    const identification = new ThreePageIdentification(root, (kind, payload) => { posted.push({ kind, payload }); });
    identification.setSelectedMethod(true);
    await exercise(root, identification, posted);
  } finally { Object.assign(globalThis, original); dom.window.close(); }
}

suite("Control Studio Recorded-data fit", () => {
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

  test("lists the models the engine offers under its words, enables only the ones it enables, and enables every model when it names none", async () => {
    await served((root, identification) => {
      const options = () => Array.from(root.querySelectorAll<HTMLOptionElement>("[data-three-page-identification-model] option"));
      const states = () => options().map(option => `${option.value}:${option.disabled}`).join(",");
      assert.strictEqual(options().length, 0, "before an answer the page lists no model of its own");
      identification.receive("identify", answerOf({
        form: { schemaVersion: 1, model: "fopdt", heldStep: true, values: {}, excludedStart: "", excludedEnd: "" },
        modelChoices: [
          { model: "automatic", enabled: false, label: "Choose automatically" },
          { model: "integrating", enabled: true, label: "Integrating" },
          { model: "first_order", enabled: false, label: "First order" },
          { model: "fopdt", enabled: true, label: "First order with dead time" },
          { model: "second_order", enabled: false, label: "Second order" },
          { model: "sopdt", enabled: true, label: "Compare additional lag" },
          { model: "third_order", enabled: true, label: "Third order (a model this page has never heard of)" },
        ],
      }));
      assert.strictEqual(states(), "automatic:true,integrating:false,first_order:true,fopdt:false,second_order:true,sopdt:false,third_order:false",
        "the held step leaves the models the engine enables, and disables the rest");
      assert.deepStrictEqual(options().map(option => option.textContent), ["Choose automatically", "Integrating", "First order", "First order with dead time",
        "Second order", "Compare additional lag", "Third order (a model this page has never heard of)"],
        "each option reads the engine's words, and a model the page knows no words for is listed all the same");
      assert.strictEqual(root.querySelector<HTMLSelectElement>("[data-three-page-identification-model]")!.value, "fopdt", "the form's model is chosen among the engine's options");
      identification.receive("identify", answerOf({}));
      assert.strictEqual(options().some(option => option.disabled), false, "an answer that names no choices enables every model");
    });
  });

  test("shows the engine's sentence for a cancelled identification, and writes none when the engine sends none", async () => {
    await served((root, identification) => {
      const status = root.querySelector<HTMLElement>("[data-three-page-identification-status]")!;
      const ports = { select: () => undefined, receive: (stage: string, payload: Record<string, unknown>) => identification.receive(stage, payload) };
      identification.receive("identify", { status: "running", reason: "Running the identification." });
      receiveEngineMessage(root, { kind: "control-studio.engine.update", update: { command: "identify", state: "cancelled", message: "Identification cancelled." } }, ports);
      assert.strictEqual(status.textContent, "Identification cancelled.", "the sentence is the engine's");
      identification.receive("identify", { status: "running", reason: "Running the identification." });
      receiveEngineMessage(root, { kind: "control-studio.engine.update", update: { command: "identify", state: "cancelled" } }, ports);
      assert.strictEqual(status.textContent, "Running the identification.", "a cancelled update without a sentence writes none");
    });
  });

  test("starts with no number of its own and shows the numbers of the engine's form (B3.3)", async () => {
    await served((root, identification) => {
      const fields = Array.from(root.querySelectorAll<HTMLInputElement>("[data-three-page-identification-setting]"));
      const keys = fields.map(field => field.getAttribute("data-three-page-identification-setting")!);
      assert.strictEqual(keys.length, 21, "the held-step settings and the sensor-lag fields");
      assert.strictEqual(fields.filter(field => field.value !== "").length, 0, "the template holds no number: the engine's default form does");
      const values = Object.fromEntries(keys.map((key, index) => [key, String(index + 1)]));
      identification.receive("identify", answerOf({ form: { schemaVersion: 1, model: "automatic", heldStep: false, values, excludedStart: "", excludedEnd: "" } }));
      assert.deepStrictEqual(fields.map(field => field.value), keys.map((_, index) => String(index + 1)), "every field shows the engine's number");
    });
  });

  test("shows Cancel while a command of the engine runs, and the engine's label again once it is cancelled, on the button that keeps the focus", async () => {
    await served((root, identification, posted) => {
      const button = root.querySelector<HTMLButtonElement>("[data-engineer-find]")!;
      identification.receive("identify", answerOf({ phase: "qualified", primaryAction: "fitPlant", primaryActionLabel: "Fit plant model" }));
      button.focus();
      identification.primaryAction();
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.fit"]);
      assert.strictEqual(button.textContent, "Cancel", "a running command can be cancelled from the same button");
      posted.length = 0;
      identification.primaryAction();
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.cancel"]);
      identification.receive("identify", { status: "cancelled" });
      assert.strictEqual(button.textContent, "Fit plant model", "the engine's label returns once the calculation is cancelled");
      assert.strictEqual(root.ownerDocument.activeElement, button, "the keyboard focus never left the button");
    });
  });

  /** The measurement of a held step as the engine answers it (`StepResponseMeasurement`), with `change` as the step's size. */
  const measurementOf = (change: number): StepResponseMeasurement => {
    const span = (mean: number, start: number, end: number) => ({ mean, startSeconds: start, endSeconds: end, count: 50, minimum: mean, maximum: mean, range: 0, slopePerSecond: 0 });
    return { baseline: span(35, -5, 0), final: span(37.4, 35, 40), inputChange: change, stepTimeSeconds: 0, holdEndSeconds: 40, sampleTimeSeconds: 0.1,
      onsetThreshold: 35.15, onsetSide: "above", level63: 36.5, warnings: [],
      settings: { onsetSamples: 3, onsetThreshold: 0.15, measurementBound: 0.05 } } as unknown as StepResponseMeasurement;
  };

  test("words the onset criterion with the side the engine states and the gain with the unit it states, and derives neither (B3.3, N2.5)", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<details><summary></summary><table><tbody></tbody></table></details>");
    const previous = { document: (globalThis as any).document };
    Object.assign(globalThis, { document: dom.window.document });
    try {
      const details = dom.window.document.querySelector("details") as HTMLDetailsElement;
      const estimation = { best: { gain: 0.2, delaySeconds: 1, timeConstantSeconds: 5 }, settings: { measurementBound: 0.05 },
        consistency: { count: 3, gain: { minimum: 0.19, maximum: 0.21 } } } as unknown as StepResponseAnalysis;
      const rowsOf = () => new Map(Array.from(details.querySelectorAll("tbody tr"), row => [row.querySelector("th")!.textContent ?? "", row.querySelector("td")!.textContent ?? ""]));
      // The final window (37.4) lies above the baseline (35), yet the engine states the onset below it: its direction comes from the hold's last tenth
      // (N2.5), and a page that compared the two windows would say "above".
      renderStepResponse({ ...measurementOf(12), onsetSide: "below" }, "degC", "%", details, estimation, false, "degC/%");
      assert.strictEqual(rowsOf().get("Onset criterion"), "3 consecutive samples · 0.15 °C below baseline");
      assert.strictEqual(rowsOf().get("Best gain"), "0.2 °C/%", "the gain is shown with the model's unit");
      assert.strictEqual(rowsOf().get("Gain range"), "0.19–0.21 °C/%");
      // A record that states neither leaves them out: the page composes no side and no unit from the signals.
      const { onsetSide: _side, ...unstated } = measurementOf(12) as unknown as Record<string, unknown>;
      renderStepResponse(unstated as unknown as StepResponseMeasurement, "degC", "%", details, estimation, false);
      assert.strictEqual(rowsOf().get("Onset criterion"), "3 consecutive samples · 0.15 °C");
      assert.strictEqual(rowsOf().get("Best gain"), "0.2");
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });

  test("keeps the evidence of a refused independent prediction in view, and offers the check again (B3.3)", async () => {
    await served((root, identification) => {
      const points = [{ timeSeconds: 0, measured: 35, predicted: 35.1, residual: -0.1 }, { timeSeconds: 1, measured: 36, predicted: 35.3, residual: 0.7 }];
      // The engine states the largest absolute residual with the fit (B3.3); here it differs from the points' own, so the page is shown to compute none.
      identification.receive("identify", answerOf({ status: "failed", reason: "The prediction exceeds the independent measurement bound.",
        fit: { stepResponseMeasurement: measurementOf(12), points, rootMeanSquareError: 0.5 }, maximumAbsoluteResidual: 0.25,
        presentation: { units: { input: "%", output: "degC" }, predictedResponse: points } }));
      const details = root.querySelector<HTMLDetailsElement>("[data-step-response-evidence]")!;
      assert.strictEqual(details.hidden, false, "the refused prediction stays inspectable");
      const rowOf = (label: string): string | undefined =>
        Array.from(details.querySelectorAll("tr")).find(row => row.querySelector("th")?.textContent === label)?.querySelector("td")?.textContent ?? undefined;
      assert.match(rowOf("Prediction RMSE") ?? "", /^0\.5 /, "the fit's own error");
      assert.match(rowOf("Maximum absolute residual") ?? "", /^0\.25 /, "the engine's number, not the points' largest residual of 0.7");
      // An answer that states no residual shows no such row: the page does not fall back to computing one from the points.
      identification.receive("identify", answerOf({ status: "failed", reason: "The prediction exceeds the independent measurement bound.",
        fit: { stepResponseMeasurement: measurementOf(12), points, rootMeanSquareError: 0.5 },
        presentation: { units: { input: "%", output: "degC" }, predictedResponse: points } }));
      assert.strictEqual(rowOf("Maximum absolute residual"), undefined);
      assert.match(rowOf("Prediction RMSE") ?? "", /^0\.5 /);
      assert.strictEqual(root.querySelector("[data-three-page-identification-status]")!.textContent, "The prediction exceeds the independent measurement bound.");
      assert.strictEqual(root.querySelector("[data-engineer-find]")!.textContent, "Check recording", "the model is not accepted: the engine offers the check");
    });
  });

  test("the validation measurement table describes the current data and labels the retained estimation fit apart from it (B3.3)", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM("<details><summary></summary><table><tbody></tbody></table></details>");
    const previous = { document: (globalThis as any).document };
    Object.assign(globalThis, { document: dom.window.document });
    try {
      const details = dom.window.document.querySelector("details") as HTMLDetailsElement;
      const estimation = { best: { gain: 0.2, delaySeconds: 1, timeConstantSeconds: 5 }, settings: { measurementBound: 0.05 },
        consistency: { count: 3, gain: { minimum: 0.19, maximum: 0.21 } } } as unknown as StepResponseAnalysis;
      renderStepResponse(measurementOf(12), "degC", "%", details, estimation, true);
      const rows = new Map(Array.from(details.querySelectorAll("tbody tr"), row => [row.querySelector("th")!.textContent ?? "", row.querySelector("td")!.textContent ?? ""]));
      assert.strictEqual(rows.get("Applied change"), "12 %", "the table describes the validation step");
      assert.ok([...rows.keys()].some(label => /Estimation gain/.test(label)), "the retained estimation parameters are labelled separately from the validation measurements");
      assert.strictEqual(details.querySelector("summary")!.textContent, "Validation step measurements");
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });
});
