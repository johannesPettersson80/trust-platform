import { initializeExperimentWorkflow } from "../../controlStudio/webview/experimentWorkflow";
import { initializeRecordingControls } from "../../controlStudio/webview/recordingControls";
import { setWorkflowActionAvailability } from "../../controlStudio/webview/workflowActionAvailability";
import * as assert from "assert";
const { JSDOM } = require("jsdom");

suite("Control Studio Test presentation", () => {
  test("distinguishes the last Test outcome from a newly reviewed test without discarding its recording", () => {
    const { dom, root, workflow } = testFixture(setWorkflowActionAvailability);
    root.insertAdjacentHTML("beforeend", `<button data-control-studio-mode aria-label="Test, Ready" data-status="ready">Test</button>
      <button data-result-save disabled>Save result</button><p data-control-studio-recording-evidence></p>`);
    try {
      workflow.render({ phase: "terminal", payload: { status: "aborted", stop_reason: "user_abort", restoration: "restored",
        recordingEvidence: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 15,
          reason: "Complete recording — runtime captured 15 scans. Stop: stopped." },
        identification: { qualification: "not_qualified" } } });
      const notice = root.querySelector("[data-control-studio-recording-evidence]")!;
      const tab = root.querySelector<HTMLElement>('[aria-label^="Test,"]')!;
      assert.match(notice.textContent!, /^Last test: Aborted\./);
      assert.match(notice.textContent!, /Recording retained.*Not qualified for Identify/);
      assert.doesNotMatch(notice.textContent!, /runtime captured|Stop:|scans/);
      assert.strictEqual(tab.dataset.statusLabel, "Last aborted");
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("new-reviewed-run") } });
      assert.strictEqual(tab.dataset.status, "ready", "a new reviewed plan must not display the old Test warning");
      assert.strictEqual(tab.dataset.statusLabel, "Ready");
      assert.match(notice.textContent!, /^Last test: Aborted\./, "review must retain the historical outcome");
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-result-save]")!.disabled, false);
      workflow.render({ phase: "running", payload: { experiment_id: "new-run" } });
      assert.match(notice.textContent!, /^Test recording in progress/);
      assert.strictEqual(tab.dataset.statusLabel, "In progress");
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("keeps unconfirmed output restoration explicit beside retained recordings", () => {
    const { dom, root, workflow } = testFixture();
    root.insertAdjacentHTML("beforeend", `<p data-control-studio-recording-evidence></p>`);
    try {
      workflow.render({ phase: "terminal", payload: { status: "failed_to_restore", restoration: "failed", stop_reason: "user_abort",
        recordingEvidence: { method: "runtime_scan_buffer", completeness: "incomplete", reason: "Stop acknowledgement missing." },
        identification: { qualification: "not_qualified" } } });
      assert.match(root.querySelector("[data-control-studio-recording-evidence]")!.textContent!, /^Test recovery required\./);
      assert.match(root.querySelector("[data-test-state]")!.textContent!, /Failed to restore/);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("saves a retained aborted record while identification and validation stay unqualified", () => {
    const { dom, root, workflow } = testFixture(setWorkflowActionAvailability);
    root.insertAdjacentHTML("beforeend", `<button data-result-save disabled>Save result</button>
      <button data-result-export-json disabled>Export result</button><button data-identify-run disabled>Identify</button>
      <button data-validate-run disabled>Validate</button><button data-control-studio-mode aria-label="Validate, Needs data">Validate</button>
      <p data-control-studio-recording-evidence></p>`);
    try {
      workflow.render({ phase: "terminal", payload: { status: "aborted", stop_reason: "user_abort", restoration: "restored",
        recordingEvidence: { method: "runtime_scan_buffer", completeness: "complete", reason: "Complete recording — 15 native scans." },
        identification: { qualification: "not_qualified" } } });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-result-save]")!.disabled, false);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-result-export-json]")!.disabled, false);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-identify-run]")!.disabled, true);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-validate-run]")!.disabled, true);
      assert.strictEqual(root.querySelector<HTMLElement>('[aria-label^="Validate,"]')!.dataset.status, "ready");
      assert.match(root.querySelector("[data-test-state]")!.textContent!, /not qualified for Identify/);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });
  test("reviews Boolean lesson activation without internal authority units", () => {
    const { dom, root, workflow } = testFixture();
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: {
        ...testPlan("lesson"), affectedSignal: "P1.ExperimentEnabled", from: false, to: true,
        unit: "authority", minimum: 0, maximum: 1, restorationValue: false,
      } } });
      assert.strictEqual(root.querySelector("[data-test-signal]")!.textContent, "P1.ExperimentEnabled · Disabled → Enabled");
      assert.strictEqual(root.querySelector("[data-test-bound]")!.textContent, "Disabled … Enabled");
      assert.strictEqual(root.querySelector("[data-test-restoration]")!.textContent, "Disabled");
    } finally { (globalThis as any).document = dom.previousDocument; }
  });
  test("preserves Test acquisition ownership across late manual recording notices", () => {
    const { dom, root, workflow } = testFixture();
    root.insertAdjacentHTML("beforeend", `<button data-control-studio-record-start>Start recording</button>
      <button data-control-studio-record-stop>Stop recording</button>
      <p data-control-studio-recording-evidence>No recording captured yet.</p>
      <button data-control-studio-mode data-status="ready" aria-label="Test, Ready">Test</button>`);
    const posts: string[] = [];
    const recording = initializeRecordingControls({ post: kind => posts.push(kind),
      setText: (selector, value) => { const element = root.querySelector(selector); if (element) element.textContent = String(value); },
      setWorkflowActionAvailability: () => undefined });
    try {
      workflow.render({ phase: "running", payload: { experiment_id: "test-owned" } });
      recording.receive({ state: "recording", reason: "Late manual recording notice" });
      assert.match(root.querySelector("[data-control-studio-recording-evidence]")!.textContent!, /Test recording in progress/);
      for (const selector of ["[data-control-studio-record-start]", "[data-control-studio-record-stop]"]) {
        const button = root.querySelector<HTMLButtonElement>(selector)!;
        assert.strictEqual(button.hidden, true);
        // A queued activation must also be rejected by the bound handler.
        button.disabled = false; button.click();
      }
      assert.deepStrictEqual(posts, [], "Test owns the active acquisition; manual commands cannot steal it");
      workflow.render({ phase: "terminal", payload: { status: "completed", restoration: "restored" } });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-control-studio-record-start]")!.hidden, false);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-control-studio-record-stop]")!.disabled, true);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });
  test("makes Confirm the sole reviewed Test action and lets Change test unlock the form", () => {
    const dom = new JSDOM(`<main data-control-studio>
      <button data-test-review class="primary">Review test</button>
      <button data-test-change-review class="secondary" hidden>Change test</button>
      <button data-test-approve class="primary" hidden>Confirm and start</button>
      <button data-test-abort class="danger" hidden disabled>Abort test</button>
      <input id="test-duration" value="30">
      <div data-test-preview hidden></div><p data-test-state></p>
      <p data-test-signal></p><p data-test-bound></p><p data-test-timeout></p>
      <p data-test-restoration></p><p data-test-stops></p>
    </main>`);
    const previousDocument = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    const root: HTMLElement = dom.window.document.querySelector("[data-control-studio]")!;
    const reviewed: unknown[] = [];
    let locked: boolean | undefined;
    const workflow = initializeExperimentWorkflow({
      root,
      setText: (selector, value) => { const element = dom.window.document.querySelector(selector); if (element) element.textContent = String(value); },
      setWorkflowActionAvailability: () => undefined,
      setExperimentFormLocked: value => { locked = value; },
      lastSubmittedForm: () => ({ kind: "observation", amplitude: 0, durationSeconds: 30 }),
      setReviewedForm: value => { reviewed.push(value); },
      saturationEvidence: { clear: () => undefined, receive: () => undefined },
      antiWindupControls: { receive: () => undefined },
      relayTuningControls: { receive: () => undefined, reset: () => undefined },
    });
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: {
        id: "plan-1", source: "simulation", affectedSignal: "PV", from: 0, to: 0,
        unit: "°C", minimum: 0, maximum: 100, durationMs: 30_000,
        restorationValue: 0, stopConditions: ["Abort"],
      } } });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-review]")?.hidden, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-change-review]")?.hidden, false);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-approve]")?.hidden, false);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-approve]")?.classList.contains("primary"), true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-abort]")?.hidden, true);
      assert.strictEqual(locked, true);
      root.dataset.experimentPlanId = "plan-1";
      root.dataset.experimentSource = "simulation";
      root.querySelector<HTMLButtonElement>("[data-test-change-review]")?.click();
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-review]")?.hidden, false);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-change-review]")?.hidden, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-approve]")?.hidden, true);
      assert.strictEqual(root.dataset.experimentPlanId, undefined);
      assert.strictEqual(root.dataset.experimentSource, undefined);
      assert.strictEqual(locked, false);
      assert.strictEqual(reviewed[reviewed.length - 1], undefined);
      (workflow as any).beginStart?.();
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-approve]")?.hidden, true);
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: {
        id: "plan-2", source: "simulation", affectedSignal: "PV", from: 0, to: 0,
        unit: "°C", minimum: 0, maximum: 100, durationMs: 30_000,
        restorationValue: 0, stopConditions: ["Abort"],
      } } });
      (workflow as any).beginStart?.();
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-approve]")?.disabled, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-change-review]")?.hidden, true);
      workflow.render({ phase: "running", payload: { experiment_id: "run-1" } });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-abort]")?.hidden, false);
      workflow.render({ phase: "terminal", payload: { status: "completed", stop_reason: "duration_complete", restoration: "restored" } });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-review]")?.hidden, false);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-abort]")?.hidden, true);
    } finally {
      (globalThis as any).document = previousDocument;
    }
  });

  test("locks Change test and Confirm before pending start", () => {
    const { dom, root, workflow } = testFixture();
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("plan-pending") } });
      (workflow as any).beginStart?.();
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-approve]")?.disabled, true);
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-change-review]")?.hidden, true);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("restores Review after definite terminal Test completion", () => {
    const { dom, root, workflow } = testFixture();
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("plan-terminal") } });
      workflow.render({ phase: "terminal", payload: { status: "completed", stop_reason: "duration_complete", restoration: "restored" } });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-review]")?.hidden, false);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("shows Abort only while Test is running", () => {
    const { dom, root, workflow } = testFixture();
    try {
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-abort]")?.hidden, true);
      workflow.render({ phase: "running", payload: { experiment_id: "run-visible" } });
      assert.strictEqual(root.querySelector<HTMLElement>("[data-test-abort]")?.hidden, false);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("enables a newly reviewed confirmation after the previous experiment completed", () => {
    const { dom, root, workflow } = testFixture();
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("first") } });
      (workflow as any).beginStart?.();
      workflow.render({ phase: "running", payload: { experiment_id: "first" } });
      workflow.render({ phase: "terminal", payload: { status: "completed", restoration: "restored" } });
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("second") } });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-approve]")?.disabled, false,
        "a completed experiment must not permanently disable later reviewed confirmations");
    } finally { (globalThis as any).document = dom.previousDocument; }
  });

  test("removes approval after failure while keeping unknown recovery locked", () => {
    const { dom, root, workflow } = testFixture();
    try {
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("failed-start") } });
      (workflow as any).beginStart?.();
      workflow.render({ phase: "failed", payload: { reason: "Runtime rejected the test before starting." } });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-approve]")?.hidden, true);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-review]")?.disabled, false);
      workflow.render({ phase: "review", payload: { kind: "accepted", plan: testPlan("unknown-start") } });
      (workflow as any).beginStart?.();
      workflow.render({ phase: "failed", payload: { reason: "Test recording recovery remains unconfirmed" } });
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-approve]")?.hidden, true);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-review]")?.disabled, true);
      assert.strictEqual(root.querySelector<HTMLButtonElement>("[data-test-change-review]")?.disabled, true);
    } finally { (globalThis as any).document = dom.previousDocument; }
  });
});

function testPlan(id: string): Record<string, unknown> {
  return { id, source: "simulation", affectedSignal: "PV", from: 0, to: 0,
    unit: "°C", minimum: 0, maximum: 100, durationMs: 30_000,
    restorationValue: 0, stopConditions: ["Abort"] };
}

function testFixture(availability: typeof setWorkflowActionAvailability = () => undefined): { dom: any; root: HTMLElement; workflow: ReturnType<typeof initializeExperimentWorkflow> } {
  const dom = new JSDOM(`<main data-control-studio>
    <button data-test-review class="primary">Review test</button>
    <button data-test-change-review class="secondary" hidden>Change test</button>
    <button data-test-approve class="primary" hidden>Confirm and start</button>
    <button data-test-abort class="danger" hidden disabled>Abort test</button>
    <div data-test-preview hidden></div><p data-test-state></p>
    <p data-test-signal></p><p data-test-bound></p><p data-test-timeout></p>
    <p data-test-restoration></p><p data-test-stops></p>
  </main>`);
  const previousDocument = (globalThis as any).document;
  (globalThis as any).document = dom.window.document;
  const root: HTMLElement = dom.window.document.querySelector("[data-control-studio]")!;
  const workflow = initializeExperimentWorkflow({
    root,
    setText: (selector, value) => { const element = dom.window.document.querySelector(selector); if (element) element.textContent = String(value); },
    setWorkflowActionAvailability: availability,
    setExperimentFormLocked: () => undefined,
    lastSubmittedForm: () => ({ kind: "observation", amplitude: 0, durationSeconds: 30 }),
    setReviewedForm: () => undefined,
    saturationEvidence: { clear: () => undefined, receive: () => undefined },
    antiWindupControls: { receive: () => undefined },
    relayTuningControls: { receive: () => undefined, reset: () => undefined },
  });
  return { dom: { ...dom, previousDocument }, root, workflow };
}
