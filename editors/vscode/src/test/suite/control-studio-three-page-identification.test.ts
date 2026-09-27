import * as assert from "assert";
import { bindRecordingOpen } from "../../controlStudio/webview/threePageRecording";

import { renderThreePageIdentificationTemplate } from "../../controlStudio/threePageIdentificationTemplate";
import { ThreePageIdentification } from "../../controlStudio/webview/threePageIdentification";

suite("Control Studio three-page recorded identification", () => {
  test("restores completed-study controls and imported reference settings without posting edits", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main"), messages: string[] = [];
      const identification = new ThreePageIdentification(root, kind => messages.push(kind));
      identification.setSelectedMethod(true);
      const reference = { requestedKind: "sopdt", stepResponseSettings: { measurementBound: .03,
        sensorTimeConstant: { minimum: .2, maximum: 8, step: .2 }, delay: { minimum: .8, maximum: 1.2, step: .05 } } };
      identification.receive("study", { phase: "validation_required", current: reference });
      assert.strictEqual(root.querySelector("[data-three-page-identification-model]").value, "sopdt");
      assert.strictEqual(root.querySelector("[data-three-page-identification-held-step]").checked, true);
      assert.strictEqual(root.querySelector('[data-three-page-identification-setting="delay.step"]').value, "0.05");
      assert.strictEqual(root.querySelector('[data-three-page-identification-setting="sensorTimeConstant.minimum"]').value, "0.2");
      identification.receive("recording", { status: "imported", reference: { ...reference, requestedKind: "fopdt" } });
      assert.strictEqual(root.querySelector("[data-three-page-identification-model]").value, "fopdt");
      assert.strictEqual(root.querySelector('[data-three-page-identification-setting="measurementBound"]').value, "0.03");
      identification.receive("study", { current: reference, form: { model: "fopdt", heldStep: true,
        values: { measurementBound: "" }, excludedStart: "", excludedEnd: "" } });
      assert.strictEqual(root.querySelector('[data-three-page-identification-setting="measurementBound"]').value, "",
        "Unfinished field edits take priority over the last qualified settings");
      assert.deepStrictEqual(messages, [], "Restoring display inputs must not requalify or write runtime values");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("reviews model comparison evidence before retaining the recommended structure", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main><button data-engineer-find></button>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
      const root = dom.window.document.querySelector("main");
      const identification = new ThreePageIdentification(root, (kind, payload) => messages.push({ kind, payload }));
      identification.setSelectedMethod(true);
      const candidate = (lag: boolean) => ({
        adequate: true, maximumAbsoluteResidual: lag ? .02 : .03, finalError: .01, rootMeanSquareError: .015,
        model: { gain: .2, timeConstantSeconds: 5, deadTimeSeconds: 1, secondTimeConstantSeconds: lag ? .1 : undefined },
        equation: lag ? "B equation" : "A equation",
        analysis: { consistency: { count: 3, gain: { minimum: .19, maximum: .21 } }, best: { baseline: 35 },
          settings: { measurementBound: .05 }, warnings: ["Search boundary reached."] },
      });
      const comparison = { candidateA: candidate(false), candidateB: candidate(true),
        requirements: { maximumAbsoluteResidual: .2, maximumFinalError: .1 }, rmsImprovement: .01,
        recommended: "first_order", physicalOrigin: "unresolved", provisional: true };
      const payload = { status: "complete", phase: "validation_required", presentation: {
        modelStructure: comparison, units: { input: "%", output: "degC" } } };
      identification.receive("identify", payload);
      const details = root.querySelector("[data-model-comparison-evidence]");
      assert.ok(details && !details.hidden, "The comparison needs reachable evidence without a second report page");
      assert.strictEqual(details.open, false, "Comparison detail must remain collapsed by default");
      details.querySelector("summary").click();
      assert.match(details.textContent, /Peak residual \(°C\)/);
      assert.match(details.textContent, /0.03/);
      assert.match(details.textContent, /0.02/);
      assert.match(details.textContent, /A equation/);
      assert.match(details.textContent, /B equation/);
      assert.match(details.textContent, /Unresolved/);
      assert.match(details.textContent, /Search boundary reached/);
      assert.strictEqual(identification.primaryActionLabel, "Keep first-order model");
      identification.primaryAction();
      assert.deepStrictEqual(messages.pop(), { kind: "control-studio.identify.choose-structure", payload: { choice: "first_order" } });
      identification.receive("identify", { ...payload, presentation: { ...payload.presentation,
        modelStructure: { ...comparison, choice: "first_order" } } });
      assert.strictEqual(identification.primaryActionLabel, "Open validation recording");
      assert.strictEqual(details.open, true, "Updating the choice must preserve the open review");
      identification.receive("identify", { status: "complete", phase: "editing", hasRecording: true });
      assert.strictEqual(details.hidden, true, "Changed inputs must remove stale comparison evidence");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("shows engineering units and only settings used by the selected model family", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main data-process-unit="degC" data-output-unit="%">${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const identification = new ThreePageIdentification(root, () => undefined);
      identification.setSelectedMethod(true); identification.receive("recording", { status: "imported" });
      root.querySelector("[data-three-page-identification-held-step]").click();
      assert.match(root.querySelector('[data-three-page-identification-label="measurementBound"]').textContent, /°C/,
        "Bounds must show their engineering unit rather than the PV role");
      const lag = root.querySelector('[data-three-page-identification-setting="sensorTimeConstant.minimum"]');
      assert.strictEqual(lag.closest("label").hidden, true, "Additional-lag settings do not belong in a first-order fit");
      const model = root.querySelector("[data-three-page-identification-model]"); model.value = "sopdt";
      model.dispatchEvent(new dom.window.Event("change"));
      assert.strictEqual(lag.closest("label").hidden, false);
      assert.match(root.querySelector('[data-three-page-identification-label="gainStep"]').textContent, /°C\/%/);
    } finally { Object.assign(globalThis, original); dom.window.close(); }
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

  test("calls the frozen-model comparison by its actual action", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: string[] = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main"), kind => messages.push(kind));
      identification.setSelectedMethod(true);
      identification.receive("identify", { status: "complete", phase: "qualified", nextFitRole: "validation" });
      assert.strictEqual(identification.primaryActionLabel, "Compare with model", "Validation compares a frozen model rather than fitting new coefficients");
      identification.primaryAction();
      assert.deepStrictEqual(messages, ["control-studio.identify.fit"], "The existing validation owner still performs the comparison");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("shows one recording opener and labels a retained failed recording accurately", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main><button data-engineer-find></button><button data-identify-import>Open recording…</button>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const identification = new ThreePageIdentification(root, () => undefined);
      const open = root.querySelector("[data-identify-import]");
      identification.setSelectedMethod(true);
      assert.strictEqual(open.hidden, true, "The primary action already opens the recording");
      identification.receive("recording", { status: "imported" });
      assert.strictEqual(open.hidden, false);
      identification.receive("identify", { status: "failed", reason: "Use a distinct validation step with a different applied amplitude." });
      assert.strictEqual(identification.primaryActionLabel, "Check recording", "The retained record remains available; the action must say what it does");
      identification.receive("identify", { status: "complete", phase: "validation_required" });
      assert.strictEqual(open.hidden, true, "Open validation recording already uses the same picker");
      identification.setSelectedMethod(false);
      assert.strictEqual(open.hidden, false);
    } finally { Object.assign(globalThis, original); dom.window.close(); }
  });

  test("keeps measured windows and fitted uncertainty available in collapsed fit details", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const identification = new ThreePageIdentification(root, () => assert.fail("Inspecting measured results must not start an operation"));
      identification.setSelectedMethod(true);
      const window = { startSeconds: 0, endSeconds: 5, count: 51, mean: 35, minimum: 34.99, maximum: 35.01, range: 0.02, slopePerSecond: 0 };
      const analysis = { baseline: window, final: { ...window, startSeconds: 30, endSeconds: 35, mean: 37 },
        inputChange: 10, stepTimeSeconds: 5, holdEndSeconds: 35, sampleTimeSeconds: 0.1, detectableOnsetSeconds: 1.2,
        onsetThreshold: 35.15, temperature63: 36.264, settings: { onsetSamples: 3, onsetThreshold: 0.15, measurementBound: 0.05 },
        best: { gain: 0.2, delaySeconds: 1, timeConstantSeconds: 5 },
        consistency: { count: 42, gain: { minimum: 0.19, maximum: 0.21 }, delaySeconds: { minimum: 0.9, maximum: 1.1 }, timeConstantSeconds: { minimum: 4.8, maximum: 5.2 } }, warnings: [] };
      identification.receive("identify", { status: "complete", phase: "validation_required", fit: { stepResponse: analysis },
        presentation: { units: { output: "degC", input: "%" } } });
      const details = root.querySelector("[data-step-response-evidence]");
      assert.ok(details && !details.hidden, "Measured qualification and uncertainty must be inspectable after fitting");
      assert.strictEqual(details.open, false, "Fit details must not clutter the default model result");
      assert.match(details.textContent, /42 candidates/);
      assert.match(details.textContent, /0.19–0.21/);
      assert.match(details.textContent, /Transport delay/);
      identification.receive("identify", { status: "complete", phase: "editing", hasRecording: true });
      assert.strictEqual(details.hidden, true, "Changed fit settings must not keep stale qualified measurements visible");
    } finally { Object.assign(globalThis, original); dom.window.close(); }
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

  test("qualifies a selected recording through the existing identify.run contract", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
      identification.setSelectedMethod(true);
      identification.receive("recording", { status: "imported" });
      const model = dom.window.document.querySelector("[data-three-page-identification-model]") as HTMLSelectElement;
      model.value = "first_order";
      (dom.window.document.querySelector("[data-three-page-identification-excluded-start]") as HTMLInputElement).value = "2";
      (dom.window.document.querySelector("[data-three-page-identification-excluded-end]") as HTMLInputElement).value = "3";
      assert.strictEqual(identification.primaryAction(), true);
      assert.deepStrictEqual(messages[0], {
        kind: "control-studio.identify.run",
        payload: { model: "first_order", excludedIntervals: [{ startSeconds: 2, endSeconds: 3 }] },
      });
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("routes fit, validation-recording, and acceptance through the existing states", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const opened: string[] = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }), () => opened.push("validation"));
      const comparison = { candidateA: {}, candidateB: {}, requirements: {}, rmsImprovement: 0.3,
        recommended: "first_order", physicalOrigin: "unresolved", provisional: true };
      identification.setSelectedMethod(true);
      identification.receive("identify", { status: "complete", phase: "qualified", nextFitRole: "estimation" });
      identification.primaryAction();
      identification.receive("identify", { status: "complete", phase: "validation_required", model: { kind: "first_order" }, presentation: { modelStructure: comparison } });
      identification.chooseStructure("first_order");
      identification.receive("identify", { status: "complete", phase: "validation_required", model: { kind: "first_order" } });
      identification.primaryAction();
      identification.receive("recording", { status: "imported" });
      identification.primaryAction();
      identification.receive("identify", { status: "complete", phase: "acceptance_required", model: { kind: "first_order" }, validationQualification: { suitable: true } });
      identification.primaryAction();
      assert.deepStrictEqual(messages.map(message => message.kind), ["control-studio.identify.fit", "control-studio.identify.choose-structure", "control-studio.identify.run", "control-studio.identify.accept"]);
      assert.deepStrictEqual(opened, ["validation"]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("keeps a running identify operation cancellable and preserves failures", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: string[] = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        kind => messages.push(kind));
      identification.setSelectedMethod(true);
      identification.receive("identify", { status: "running", reason: "Checking recording…" });
      assert.strictEqual(identification.busy, true);
      assert.strictEqual(identification.primaryActionLabel, "Cancel");
      assert.strictEqual(identification.primaryAction(), true);
      identification.receive("identify", { status: "failed", phase: "qualified", reason: "Recording is not independent." });
      assert.strictEqual(identification.busy, false);
      assert.match((dom.window.document.querySelector("[data-three-page-identification-status]") as HTMLElement).textContent ?? "", /not independent/);
      assert.deepStrictEqual(messages, ["control-studio.identify.cancel"]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("omits blank exclusions and rejects partial or zero-length intervals", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: Array<{ kind: string; payload: Record<string, unknown> }> = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        (kind, payload) => messages.push({ kind, payload: payload ?? {} }));
      identification.setSelectedMethod(true);
      identification.receive("recording", { status: "imported" });
      assert.strictEqual(identification.primaryAction(), true);
      assert.deepStrictEqual(messages.pop(), { kind: "control-studio.identify.run", payload: { model: "automatic" } });
      identification.receive("recording", { status: "imported" });
      const start = dom.window.document.querySelector("[data-three-page-identification-excluded-start]") as HTMLInputElement;
      const end = dom.window.document.querySelector("[data-three-page-identification-excluded-end]") as HTMLInputElement;
      start.value = "2";
      assert.strictEqual(identification.primaryAction(), false);
      assert.strictEqual(messages.length, 0);
      start.value = "2";
      end.value = "2";
      assert.strictEqual(identification.primaryAction(), false);
      assert.strictEqual(messages.length, 0);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("exposes only host-authorized structure choices and routes the selected choice", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: string[] = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        kind => messages.push(kind));
      identification.setSelectedMethod(true);
      const comparison = {
        candidateA: { adequate: false }, candidateB: { adequate: true },
        requirements: { maximumAbsoluteResidual: 0.2, maximumFinalError: 0.1 }, rmsImprovement: 0.3,
        recommended: "additional_lag", physicalOrigin: "unresolved", provisional: true,
      };
      identification.receive("identify", { status: "complete", phase: "validation_required",
        presentation: { modelStructure: comparison } });
      assert.strictEqual(identification.primaryActionLabel, "Keep additional-lag model");
      assert.strictEqual(identification.chooseStructure("first_order"), false);
      identification.primaryAction();
      assert.deepStrictEqual(messages, ["control-studio.identify.choose-structure"]);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("keeps acceptance disabled when validation or structure comparison is unqualified", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const messages: string[] = [];
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement,
        kind => messages.push(kind));
      identification.setSelectedMethod(true);
      identification.receive("identify", { status: "complete", phase: "acceptance_required",
        validationQualification: { suitable: false }, presentation: { modelStructure: {
          candidateA: {}, candidateB: {}, requirements: {}, rmsImprovement: 0,
          recommended: "neither", physicalOrigin: "unresolved", provisional: true,
        } } });
      assert.strictEqual(identification.primaryAction(), false);
      assert.strictEqual(identification.primaryActionLabel, "Validation not qualified");
      assert.deepStrictEqual(messages, []);
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });

  test("restores an editable form and its retained-recording state", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderThreePageIdentificationTemplate()}</main>`);
    const original = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent };
    Object.assign(globalThis, { window: dom.window, document: dom.window.document, CustomEvent: dom.window.CustomEvent });
    try {
      const identification = new ThreePageIdentification(dom.window.document.querySelector("main") as HTMLElement, () => undefined);
      identification.setSelectedMethod(true);
      identification.receive("identify", { status: "complete", phase: "editing", hasRecording: true,
        form: { schemaVersion: 1, model: "sopdt", heldStep: true, values: {}, excludedStart: "", excludedEnd: "" } });
      assert.strictEqual((dom.window.document.querySelector("[data-three-page-identification-model]") as HTMLSelectElement).value, "sopdt");
      assert.strictEqual((dom.window.document.querySelector("[data-three-page-identification-held-step]") as HTMLInputElement).checked, true);
      assert.strictEqual(identification.primaryActionLabel, "Check recording");
    } finally {
      Object.assign(globalThis, original);
      dom.window.close();
    }
  });
});
