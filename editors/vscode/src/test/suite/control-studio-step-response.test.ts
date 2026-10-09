import * as assert from "assert";
import { identificationPresentationState } from "../../controlStudio/identificationPresentationState";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";
import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";
import { IdentificationWorkflow, identifiedModelPresentation, type IdentificationSample } from "../../controlStudio/identification";
import { initializeStepResponseControls, readStepResponseSettings } from "../../controlStudio/webview/stepResponseControls";
import { ALL_IDENTIFICATION_FORM_KEYS } from "../../controlStudio/identificationFormDraft";

const settings = {
  baselineDurationSeconds: 5, finalDurationSeconds: 5,
  maximumWindowRange: .10, maximumWindowSlopePerSecond: .01,
  measurementBound: .05, onsetThreshold: .15, onsetSamples: 3,
  delay: { minimum: 0, maximum: 2, step: .1 },
  timeConstant: { minimum: .1, maximum: 20, step: .1 },
  baselineStep: .01, gainStep: .001, timestampShiftSeconds: 0,
};

function record(change = 10): IdentificationSample[] {
  return Array.from({ length: 452 }, (_, i) => {
    const time = i / 10 - 5;
    return { timeSeconds: time, input: time >= 0 && time < 40 ? 75 + change : 75,
      output: time <= 1 ? 35 : 35 + .2 * change * (1 - Math.exp(-(time - 1) / 5)),
      quality: "good", inputUnit: "%", outputUnit: "degC" };
  });
}

suite("Control Studio bounded step response", () => {
  test("the retained step-response editor preserves invalid draft fields across a late acknowledgement", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-loop-id="l" data-editor-session-id="s" data-loop-revision="r"><section data-workflow-pane="Identify"><div class="field"><select id="identify-model"><option value="automatic">Automatic</option><option value="fopdt">FOPDT</option></select></div><details><input id="identify-exclude-start"><input id="identify-exclude-end"></details><button data-identify-import>Import</button><button data-identify-run>Check</button><button data-identify-fit hidden>Fit</button><button data-identify-cancel hidden>Cancel</button><button data-identify-accept hidden>Accept</button><button data-identify-record-validation hidden>Validate</button><div class="summary-panel"><div data-identify-state></div></div></section></main>', { runScripts: "outside-only", url: "https://studio.test" });
    dom.window.structuredClone = structuredClone;
    dom.window.HTMLElement.prototype.scrollIntoView = () => undefined;
    const posts: any[] = [];
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window };
    Object.assign(globalThis, { document: dom.window.document, window: dom.window });
    try {
      // Exercise this retained helper directly. The three-page shipped bundle
      // intentionally no longer mounts the manual held-step editor.
      initializeStepResponseControls((kind, payload) => posts.push({ kind, payload }));
      const doc = dom.window.document;
      doc.querySelector('[data-step-response-enabled]').click();
      const input = doc.querySelector('[data-step-field="measurementBound"]');
      input.value = "0"; input.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      assert.strictEqual(readStepResponseSettings(), null);
      const error = doc.querySelector('[data-identify-state]').textContent;
      assert.match(error, /Review.*measurement/i);
      const reply = () => dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
        kind: "control-studio.engineering.result", stage: "identify", projectId: "p", loopId: "l", editorSessionId: "s", expectedLoopRevision: "r",
        payload: { status: "complete", phase: "editing", hasRecording: true, nextFitRole: "estimation", form: posts[posts.length - 1].payload.form },
      } }));
      reply();
      assert.strictEqual(doc.querySelector('[data-identify-state]').textContent, error,
        "the older edit reply must not replace the newer Check failure");
      assert.strictEqual(input.getAttribute("aria-invalid"), "true");
      input.value = ".05"; input.dispatchEvent(new dom.window.Event("input", { bubbles: true })); reply();
      assert.strictEqual(input.getAttribute("aria-invalid"), null);
      assert.match(doc.querySelector('[data-identify-state]').textContent, /Analysis settings changed/);
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });

  test("captures every raw field, including blanks, and restores the same draft without validating while typing", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-loop-id="l" data-editor-session-id="s" data-loop-revision="r"><section data-workflow-pane="Identify"><div class="field"><select id="identify-model"><option value="automatic">Automatic</option><option value="fopdt">FOPDT</option></select></div><details><input id="identify-exclude-start"><input id="identify-exclude-end"></details><p data-identify-recording-source hidden></p><button data-identify-import></button><button data-identify-run></button><button data-identify-fit></button><button data-identify-accept></button><button data-identify-record-validation></button><div class="summary-panel"><div data-identify-state></div></div></section></main>');
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window };
    Object.assign(globalThis, { document: dom.window.document, window: dom.window });
    try {
      const posted: any[] = [];
      initializeStepResponseControls((kind, payload) => posted.push({ kind, payload }));
      const field = dom.window.document.querySelector('[data-step-field="delay.minimum"]') as HTMLInputElement;
      field.value = "";
      field.dispatchEvent(new dom.window.Event("input", { bubbles: true }));
      const latest = posted[posted.length - 1];
      const form = latest.payload.form;
      assert.strictEqual(latest.kind, "control-studio.identify.edit");
      assert.strictEqual(form.values["delay.minimum"], "");
      assert.deepStrictEqual(Object.keys(form.values).sort(), [...ALL_IDENTIFICATION_FORM_KEYS].sort());
      assert.strictEqual((dom.window.document.querySelector("[data-identify-run]") as HTMLButtonElement).hidden, false,
        "a retained native recording must keep Check visible while raw text is edited");
      assert.strictEqual((dom.window.document.querySelector("[data-identify-run]") as HTMLButtonElement).disabled, false);
      assert.ok(dom.window.document.querySelector("[data-identify-run]").classList.contains("primary"));
      const formDraft = { ...form, values: { ...form.values, "delay.minimum": "1.00" } };
      dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
        kind: "control-studio.workflow-result", phase: "restored", projectId: "p", loopId: "l", editorSessionId: "s", expectedLoopRevision: "r",
        payload: { result: { identificationStudy: { form: formDraft, current: { requestedKind: "fopdt", stepResponseSettings: settings } } } },
      } }));
      assert.strictEqual((dom.window.document.querySelector('[data-step-field="delay.minimum"]') as HTMLInputElement).value, "1.00",
        "a restored raw form must take precedence over derived numeric settings");
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });

  test("retains measured windows and the explicit consistent-candidate grid in the existing workflow", () => {
    const workflow = new IdentificationWorkflow();
    const data = record();
    const qualification = (workflow.qualify as Function)(data, [], "Simulation", "fopdt", "estimation", settings);
    assert.strictEqual(qualification.suitable, true);
    const review = workflow.fit() as any;
    assert.ok(review.stepResponse, "the requested measured-step analysis must be retained by the existing workflow");
    assert.strictEqual(review.stepResponse.baseline.mean, 35);
    assert.ok(review.stepResponse.final.mean > 36.99);
    assert.strictEqual(review.stepResponse.inputChange, 10);
    assert.ok(review.stepResponse.consistency.count > 0);
    assert.ok(review.stepResponse.consistency.gain.minimum <= .2 && review.stepResponse.consistency.gain.maximum >= .2);
    assert.strictEqual(review.stepResponse.best.delaySeconds, 1);
    assert.strictEqual(review.stepResponse.best.timeConstantSeconds, 5);
    assert.strictEqual(review.stage, "validation_required");
    assert.strictEqual(workflow.acceptedModel, undefined);
  });

  test("keeps a drifting final window as a reported partial response and rejects invalid hold samples without deleting the source record", () => {
    for (const defect of ["drift", "quality"]) {
      const workflow = new IdentificationWorkflow(), data = record();
      if (defect === "drift") data.forEach(sample => { if (sample.timeSeconds >= 35) sample.output += .03 * (sample.timeSeconds - 35); });
      else data[100].quality = "bad";
      const before = JSON.stringify(data);
      const qualification = (workflow.qualify as Function)(data, [], "Simulation", "fopdt", "estimation", settings);
      if (defect === "drift") {
        // Only the baseline must be steady (numerical annex N2.4); the final window need only lie inside the held step (N2.6).
        // A final window that is still moving is a partial response (A10.3): it is reported as unsteady, and its bounds do not apply (N2.4).
        assert.strictEqual(qualification.suitable, true, "a drifting final window must not refuse the bounded step");
        assert.ok(qualification.stepResponse.final.range > settings.maximumWindowRange, "the drifting final window stays reported as unsteady");
        assert.doesNotThrow(() => workflow.fit(), "a partial response with a moving final window is still fitted (A10.3)");
      } else {
        assert.strictEqual(qualification.suitable, false, `${defect} must prevent bounded step qualification`);
        assert.throws(() => workflow.fit());
      }
      assert.strictEqual(JSON.stringify(data), before);
    }
  });

  test("retains rejected independent predictions through display and reopen without accepting the model", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(record(), [], "Simulation", "fopdt", "estimation", settings);
    const frozen = workflow.fit().selected;
    const holdout = record(12).map(sample => ({ ...sample, output: 35 + (sample.output - 35) * 1.2 }));
    assert.strictEqual(workflow.qualify(holdout, [], "Simulation", "fopdt", "validation", settings).suitable, true);
    try { workflow.fit(); } catch { /* The old path discarded scored failure evidence. */ }
    const review = workflow.currentState.review;
    assert.ok(review?.fit.points.length, "A rejected independent prediction must remain inspectable");
    assert.strictEqual(workflow.currentState.phase, "failed");
    assert.strictEqual(workflow.acceptedModel, undefined);
    assert.throws(() => workflow.accept(), /validation|bound|review/i);
    assert.strictEqual(review.selected.gain, frozen.gain);
    assert.strictEqual(review.selected.timeConstantSeconds, frozen.timeConstantSeconds);
    assert.strictEqual(review.selected.deadTimeSeconds, frozen.deadTimeSeconds);
    assert.ok(review.fit.points.some(point => Math.abs(point.residual) > settings.measurementBound));
    const saved = workflow.snapshot();
    const restored = new IdentificationWorkflow(); restored.restore(saved);
    assert.strictEqual(restored.currentState.phase, "failed");
    assert.deepStrictEqual(restored.currentState.review?.fit.points, review.fit.points);
    assert.throws(() => restored.accept(), /validation|bound|review/i);
    const payload = identificationPresentationState(restored, "%", "degC")!;
    assert.strictEqual(payload.status, "failed");
    assert.match(String(payload.reason), /independent measurement bound/);
    assert.ok((payload.presentation as any).predictedResponse.length);
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const identification = new ThreePageIdentification(root, () => undefined);
      identification.setSelectedMethod(true);
      identification.receive("identify", payload);
      const details = root.querySelector("[data-step-response-evidence]");
      assert.strictEqual(details.hidden, false);
      assert.match(details.textContent, /Prediction RMSE/);
      assert.match(details.textContent, /Maximum absolute residual/);
      assert.strictEqual(identification.primaryActionLabel, "Check recording");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("scores a changed validation step with frozen estimation parameters before explicit acceptance", () => {
    const workflow = new IdentificationWorkflow();
    (workflow.qualify as Function)(record(), [], "Simulation", "fopdt", "estimation", settings);
    const preview = workflow.fit();
    (workflow.qualify as Function)(record(12), [], "Simulation", "fopdt", "validation", settings);
    const review = workflow.fit() as any;
    assert.strictEqual(review.selected.gain, preview.selected.gain, "independent validation must not refit the estimation gain");
    assert.ok(review.stepResponse && (preview as any).stepResponse);
    assert.deepStrictEqual(review.stepResponse.best, (preview as any).stepResponse.best);
    assert.strictEqual(review.selected.deadTimeSeconds, preview.selected.deadTimeSeconds);
    assert.strictEqual(review.stage, "acceptance_required");
    assert.strictEqual(workflow.acceptedModel, undefined);
    assert.strictEqual(workflow.accept().model, review.selected);
  });

  test("validation chart references describe the plotted validation record while preserving the estimation grid", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(record(), [], "Simulation", "fopdt", "estimation", settings); workflow.fit();
    workflow.qualify(record(12), [], "Simulation", "fopdt", "validation", settings);
    const review = workflow.fit();
    const presentation = identifiedModelPresentation(review.selected, review.fit, "%", "degC");
    assert.strictEqual(presentation.stepResponse?.inputChange, 12,
      "the plotted validation trace must use its own measurement reference levels");
    assert.strictEqual(review.fit.stepResponse?.inputChange, 10, "the frozen estimation grid keeps its original source");
  });

  test("the validation measurement table describes current data and labels the retained estimation fit", () => {
    const workflow = new IdentificationWorkflow();
    workflow.qualify(record(), [], "Simulation", "fopdt", "estimation", settings); workflow.fit();
    workflow.qualify(record(12), [], "Simulation", "fopdt", "validation", settings);
    const review = workflow.fit() as any;
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-loop-id="l" data-editor-session-id="s" data-loop-revision="r"><section data-workflow-pane="Identify"><div class="field"><select id="identify-model"></select></div><div class="summary-panel"></div></section></main>');
    const previous = { document: (globalThis as any).document, window: (globalThis as any).window };
    Object.assign(globalThis, { document: dom.window.document, window: dom.window });
    try {
      initializeStepResponseControls();
      dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
        kind: "control-studio.engineering.result", projectId: "p", loopId: "l", editorSessionId: "s", expectedLoopRevision: "r", stage: "identify",
        payload: { status: "complete", phase: "acceptance_required", ...review },
      } }));
      const rows = [...dom.window.document.querySelectorAll('[data-step-response-report] tr')] as any[];
      assert.strictEqual(rows.find(row => row.querySelector('th').textContent === "Applied change")?.querySelector('td').textContent, "12 %");
      assert.ok(rows.some(row => /Estimation gain/.test(row.querySelector('th').textContent)),
        "retained estimation parameters must be labelled separately from current validation measurements");
    } finally { Object.assign(globalThis, previous); dom.window.close(); }
  });
});
