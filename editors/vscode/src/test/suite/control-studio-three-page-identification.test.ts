import * as assert from "assert";
import { bindRecordingOpen } from "../../controlStudio/webview/threePageRecording";

import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";

type Posted = Array<{ kind: string; payload?: Record<string, unknown> }>;

/**
 * The models of the select as every answer of the Recorded-data fit lists them (`ManualIdentificationAnswer.modelChoices`, B3.3: the models in
 * the select's order, each with its words, and enabled while no held-step measurement limits them). The page lists its options from them and
 * keeps no list of its own, so a fixture without them leaves the select empty, which no answer of the engine does.
 */
const MODEL_CHOICES = [
  { model: "automatic", enabled: true, label: "Choose automatically" },
  { model: "integrating", enabled: true, label: "Integrating" },
  { model: "first_order", enabled: true, label: "First order" },
  { model: "fopdt", enabled: true, label: "First order with dead time" },
  { model: "second_order", enabled: true, label: "Second order" },
  { model: "sopdt", enabled: true, label: "Compare additional lag" },
];

/** The Recorded-data fit's answer (`ManualIdentificationAnswer`, B3.3) with the fields the page reads; `changes` adjusts it. */
function answerOf(changes: Record<string, unknown> = {}): Record<string, unknown> {
  return { status: "complete", nextFitRole: "estimation", hasRecording: true, primaryAction: "checkRecording",
    primaryActionLabel: "Check recording", acceptanceReady: false, phase: "recording-selected", modelChoices: MODEL_CHOICES, ...changes };
}

/**
 * Runs `exercise` on the Recorded-data fit's page in a template DOM: `before` is markup placed before the template and `attributes` go on the
 * root. The method is selected, and the commands the page posts are recorded in `posted`.
 */
function withFit(exercise: (root: HTMLElement, identification: ThreePageIdentification, posted: Posted, window: any) => void,
  before = "", attributes = ""): void {
  const { JSDOM } = require("jsdom");
  const dom = new JSDOM(`<main ${attributes}>${before}${renderThreePageIdentificationTemplate()}</main>`);
  const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
  try {
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const posted: Posted = [];
    const identification = new ThreePageIdentification(root, (kind, payload) => { posted.push({ kind, payload }); });
    identification.setSelectedMethod(true);
    exercise(root, identification, posted, dom.window);
  } finally { Object.assign(globalThis, original); dom.window.close(); }
}

suite("Control Studio three-page recorded identification", () => {
  test("shows the sensor-lag comparison as collapsed evidence, and keeps the review open while the engine's choice updates", () => {
    withFit((root, identification, posted) => {
      const candidate = (lag: boolean) => ({
        adequate: true, maximumAbsoluteResidual: lag ? .02 : .03, finalError: .01, rootMeanSquareError: .015,
        model: { gain: .2, timeConstantSeconds: 5, deadTimeSeconds: 1, secondTimeConstantSeconds: lag ? .1 : undefined },
        equation: lag ? "B equation" : "A equation",
        analysis: { consistency: { count: 3, gain: { minimum: .19, maximum: .21 } }, best: { baseline: 35 },
          settings: { measurementBound: .05 }, warnings: ["Search boundary reached."] },
      });
      const comparison = { candidateA: candidate(false), candidateB: candidate(true),
        requirements: { maximumAbsoluteResidual: .2, maximumFinalError: .1 }, rmsImprovement: .01,
        recommended: "first_order" };
      const presentation = { modelStructure: comparison, units: { input: "%", output: "degC" } };
      const answer = answerOf({ phase: "validation_required", primaryAction: "chooseStructure", primaryActionLabel: "Keep first-order model",
        structureChoice: "first_order", presentation });
      identification.receive("identify", answer);
      const details = root.querySelector("[data-model-comparison-evidence]") as HTMLDetailsElement;
      assert.ok(details && !details.hidden, "The comparison needs reachable evidence without a second report page");
      assert.strictEqual(details.open, false, "Comparison detail must remain collapsed by default");
      (details.querySelector("summary") as HTMLElement).click();
      assert.match(details.textContent ?? "", /Peak residual \(°C\)/);
      assert.match(details.textContent ?? "", /0.03/);
      assert.match(details.textContent ?? "", /0.02/);
      assert.match(details.textContent ?? "", /A equation/);
      assert.match(details.textContent ?? "", /B equation/);
      assert.match(details.textContent ?? "", /Unresolved/);
      assert.match(details.textContent ?? "", /Search boundary reached/);
      assert.strictEqual(identification.primaryActionLabel, "Keep first-order model", "the label is the engine's");
      identification.primaryAction();
      assert.deepStrictEqual(posted.pop(), { kind: "control-studio.identify.choose-structure", payload: { choice: "first_order" } });
      identification.receive("identify", answerOf({ phase: "validation_required", primaryAction: "openValidationRecording",
        primaryActionLabel: "Open validation recording", presentation: { ...presentation, modelStructure: { ...comparison, choice: "first_order" } } }));
      assert.strictEqual(identification.primaryActionLabel, "Open validation recording");
      assert.strictEqual(details.open, true, "Updating the choice must preserve the open review");
      identification.receive("identify", answerOf({ phase: "editing" }));
      assert.strictEqual(details.hidden, true, "Changed inputs must remove stale comparison evidence");
    });
  });

  test("holds the engine's answer while another method is shown, and shows it when the Recorded-data fit is chosen (A8.6, B3.3)", () => {
    withFit((root, identification) => {
      const status = root.querySelector("[data-three-page-identification-status]") as HTMLElement;
      const before = status.textContent;
      identification.setSelectedMethod(false);
      identification.receive("identify", answerOf({ message: "Recording opened. Check it before fitting a model." }));
      assert.strictEqual(status.textContent, before, "an answer shown under another method would write over that method's page");
      assert.strictEqual(identification.primaryActionLabel, "Check recording", "the page holds the engine's answer meanwhile");
      identification.setSelectedMethod(true);
      assert.strictEqual(status.textContent, "Recording opened. Check it before fitting a model.", "the held answer appears when the method is chosen");
      identification.receive("identify", answerOf({ message: "Recording qualified. Fit plant model." }));
      identification.setSelectedMethod(true);
      assert.strictEqual(status.textContent, "Recording qualified. Fit plant model.", "choosing the method again shows no older answer over a newer one");
    });
  });

  test("shows engineering units at once, and the held-step and additional-lag settings as the choices ask, before the engine answers the changed form", () => {
    withFit((root, _identification, posted, window) => {
      const settings = root.querySelector("[data-three-page-identification-settings]") as HTMLElement;
      assert.strictEqual(settings.hidden, true, "the settings wait for the held-step switch");
      (root.querySelector("[data-three-page-identification-held-step]") as HTMLElement).click();
      assert.strictEqual(settings.hidden, false, "the switch shows the settings without waiting for the engine");
      assert.match(root.querySelector('[data-three-page-identification-label="measurementBound"]')!.textContent ?? "", /°C/,
        "Bounds must show their engineering unit rather than the PV role");
      const lag = root.querySelector('[data-three-page-identification-setting="sensorTimeConstant.minimum"]') as HTMLElement;
      assert.strictEqual((lag.closest("label") as HTMLElement).hidden, true, "Additional-lag settings do not belong in a first-order fit");
      // The model options are the engine's choices, listed with the first answer.
      _identification.receive("identify", answerOf({ modelChoices: [{ model: "fopdt", enabled: true, label: "First order with dead time" },
        { model: "sopdt", enabled: true, label: "Compare additional lag" }] }));
      const model = root.querySelector("[data-three-page-identification-model]") as HTMLSelectElement;
      model.value = "sopdt";
      model.dispatchEvent(new window.Event("change"));
      assert.strictEqual((lag.closest("label") as HTMLElement).hidden, false, "the comparison family shows them at once");
      assert.match(root.querySelector('[data-three-page-identification-label="gainStep"]')!.textContent ?? "", /°C\/%/);
      assert.deepStrictEqual(posted, [], "the changed form waits for the debounce");
    }, "", 'data-process-unit="degC" data-output-unit="%"');
  });

  test("clears file-read progress before starting the host operation", async () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main><button data-identify-import>Open recording…</button><input type="file" data-identify-import-file></main>');
    try {
      const root = dom.window.document.querySelector("main"), input = root.querySelector("input");
      const messages: string[] = [];
      bindRecordingOpen(root, () => false, () => messages.push("Checking recording…"), message => messages.push(message));
      Object.defineProperty(input, "files", { value: [{ size: 2, text: async () => "{}" }] });
      input.dispatchEvent(new dom.window.Event("change"));
      await new Promise(resolve => setTimeout(resolve, 0));
      assert.deepStrictEqual(messages, ["Reading recording…", "", "Checking recording…"], "A completed read must not leave stale progress or erase the next operation's message");
    } finally { dom.window.close(); }
  });

  test("labels the frozen-model comparison with the engine's own words and sends the fit", () => {
    withFit((_root, identification, posted) => {
      identification.receive("identify", answerOf({ phase: "qualified", nextFitRole: "validation", primaryAction: "compareWithModel",
        primaryActionLabel: "Compare with model" }));
      assert.strictEqual(identification.primaryActionLabel, "Compare with model", "Validation compares a frozen model rather than fitting new coefficients");
      identification.primaryAction();
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.fit"]);
    });
  });

  test("shows one recording opener, which stays while the next step is anything but a recording to open", () => {
    withFit((root, identification) => {
      const open = root.querySelector("[data-identify-import]") as HTMLButtonElement;
      assert.strictEqual(open.hidden, true, "The primary action already opens the recording");
      identification.receive("identify", answerOf());
      assert.strictEqual(open.hidden, false, "Another file can be opened while the next step is a check");
      identification.receive("identify", answerOf({ status: "failed", reason: "Use a distinct validation step with a different applied amplitude." }));
      assert.strictEqual(identification.primaryActionLabel, "Check recording", "The retained record remains available; the action says what it does");
      assert.strictEqual(open.hidden, false);
      identification.receive("identify", answerOf({ phase: "validation_required", primaryAction: "openValidationRecording",
        primaryActionLabel: "Open validation recording" }));
      assert.strictEqual(open.hidden, true, "Open validation recording already uses the same picker");
      identification.setSelectedMethod(false);
      assert.strictEqual(open.hidden, false, "Another method keeps the file button: it opens the recording that method fits");
    }, '<button data-engineer-find></button><button data-identify-import>Open recording…</button>');
  });

  test("keeps measured windows and fitted uncertainty available in collapsed fit details", () => {
    withFit((root, identification) => {
      const window = { startSeconds: 0, endSeconds: 5, count: 51, mean: 35, minimum: 34.99, maximum: 35.01, range: 0.02, slopePerSecond: 0 };
      const analysis = { baseline: window, final: { ...window, startSeconds: 30, endSeconds: 35, mean: 37 },
        inputChange: 10, stepTimeSeconds: 5, holdEndSeconds: 35, sampleTimeSeconds: 0.1, detectableOnsetSeconds: 1.2,
        onsetThreshold: 35.15, level63: 36.264, settings: { onsetSamples: 3, onsetThreshold: 0.15, measurementBound: 0.05 },
        best: { gain: 0.2, delaySeconds: 1, timeConstantSeconds: 5 },
        consistency: { count: 42, gain: { minimum: 0.19, maximum: 0.21 }, delaySeconds: { minimum: 0.9, maximum: 1.1 }, timeConstantSeconds: { minimum: 4.8, maximum: 5.2 } }, warnings: [] };
      identification.receive("identify", answerOf({ phase: "validation_required", fit: { stepResponse: analysis },
        presentation: { units: { output: "degC", input: "%" } } }));
      const details = root.querySelector("[data-step-response-evidence]") as HTMLDetailsElement;
      assert.ok(details && !details.hidden, "Measured qualification and uncertainty must be inspectable after fitting");
      assert.strictEqual(details.open, false, "Fit details must not clutter the default model result");
      assert.match(details.textContent ?? "", /42 candidates/);
      assert.match(details.textContent ?? "", /0.19–0.21/);
      assert.match(details.textContent ?? "", /Transport delay/);
      identification.receive("identify", answerOf({ phase: "editing" }));
      assert.strictEqual(details.hidden, true, "Changed fit settings must not keep stale qualified measurements visible");
    });
  });

  test("renders a compact selected-method identification workspace", () => {
    const html = renderThreePageIdentificationTemplate();
    assert.match(html, /data-three-page-identification/);
    assert.match(html, /data-three-page-identification-model/);
    assert.match(html, /data-three-page-identification-held-step/);
    assert.match(html, /data-three-page-identification-excluded-start/);
    assert.match(html, /data-three-page-identification-settings/);
    assert.match(html, /data-model-comparison-evidence/);
  });

  test("keeps a running identify operation cancellable and preserves failures", () => {
    withFit((root, identification, posted) => {
      identification.receive("identify", { status: "running", reason: "Checking recording…" });
      assert.strictEqual(identification.primaryActionLabel, "Cancel");
      assert.strictEqual(identification.primaryAction(), true);
      identification.receive("identify", { status: "failed", phase: "qualified", reason: "Recording is not independent." });
      assert.notStrictEqual(identification.primaryActionLabel, "Cancel", "a failed command is no longer running");
      assert.match((root.querySelector("[data-three-page-identification-status]") as HTMLElement).textContent ?? "", /not independent/);
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.cancel"]);
    });
  });

  test("sends the form as typed, with every setting the template holds, and no check of its own", () => {
    withFit((root, identification, posted) => {
      identification.receive("identify", answerOf());
      const bound = root.querySelector('[data-three-page-identification-setting="measurementBound"]') as HTMLInputElement;
      const start = root.querySelector("[data-three-page-identification-excluded-start]") as HTMLInputElement;
      bound.value = "";
      start.value = "2";
      assert.strictEqual(identification.primaryAction(), true, "a blank setting and a partial exclusion interval are the engine's to refuse");
      const form = posted[0].payload?.form as { values: Record<string, string>; excludedStart: string; excludedEnd: string; model: string; heldStep: boolean };
      assert.deepStrictEqual(posted.map(message => message.kind), ["control-studio.identify.run"]);
      assert.strictEqual(form.values.measurementBound, "", "the blank text goes as it is");
      assert.strictEqual(Object.keys(form.values).length, 21, "every setting field of the template goes, by its own key");
      assert.strictEqual(form.excludedStart, "2");
      assert.strictEqual(form.excludedEnd, "");
      assert.strictEqual(form.model, "automatic");
      assert.strictEqual(form.heldStep, false);
    });
  });
});
