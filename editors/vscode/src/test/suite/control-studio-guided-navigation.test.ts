/// <reference lib="dom" />
import * as navigation from "../../controlStudio/webview/engineerNavigation";
import { restoredEngineerStage, storeEngineerStage } from "../../controlStudio/engineerNavigationStorage";
import * as assert from "assert";
import { initializeMultiSelect } from "../../webview/multiSelect";
import { bindThreePageHostState } from "../../controlStudio/webview/threePageHostState";
import { engineerWorkflowControls } from "../../controlStudio/engineerWorkflowPresentation";
const { JSDOM } = require("jsdom");

suite("Control Studio guided navigation", () => {
  test("retains the experiment view with navigation without retaining runtime actions", async () => {
    const loop = { projectId: "project", runtimeId: "sim", id: "loop", revision: "loop-v1" } as any;
    const entries = new Map<string, any>();
    const context = { workspaceState: { get: (key: string) => entries.get(key),
      update: async (key: string, value: unknown) => { entries.set(key, value); } } } as any;
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "loop-v1" };
    const message = { kind: "control-studio.navigation.select", schemaVersion: 1, ...scope,
      expectedLoopRevision: scope.loopRevision, payload: { stage: "model", view: {
        acquisitionMethod: "project", experimentSignalIds: ["pv", "bias"], experimentId: "must-not-replay" } } };
    assert.strictEqual(await storeEngineerStage(context, loop, message, scope), true);
    assert.deepStrictEqual([...entries.values()][0].view,
      { acquisitionMethod: "project", experimentSignalIds: ["pv", "bias"] }, "Only display choices belong in navigation persistence");
    assert.strictEqual(await storeEngineerStage(context, loop, { ...message, payload: { stage: "tune" } }, scope), true);
    assert.strictEqual(restoredEngineerStage(context, loop), "tune");
    assert.deepStrictEqual([...entries.values()][0].view.experimentSignalIds, ["pv", "bias"]);
  });

  test("restores declared view controls and notifies the page without starting an experiment", async () => {
    const dom = new JSDOM(`<main data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">
      <select data-engineer-acquisition-method><option value="automatic">Automatic</option><option value="project">Project experiment</option></select>
      <select data-three-page-experiment-signals multiple><option value="sp" selected>Setpoint</option><option value="pv" selected>Process value</option><option value="bias">Sensor difference</option></select></main>`);
    Object.assign(globalThis, { window: dom.window, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main");
      const dropdown = initializeMultiSelect(root.querySelector("[data-three-page-experiment-signals]"));
      const selections: any[] = []; const envelopes: any[] = []; let changes = 0;
      root.addEventListener("change", () => changes++);
      bindThreePageHostState(root, () => assert.fail("Restoring navigation cannot start engineering work"),
        (page, persist) => selections.push({ page, persist }), message => envelopes.push(message));
      dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: { schemaVersion: 1,
        kind: "control-studio.navigation.restore", projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r",
        stage: "model", view: { acquisitionMethod: "project", experimentSignalIds: ["pv", "bias", "undeclared"] } } }));
      assert.strictEqual(root.querySelector("[data-engineer-acquisition-method]").value, "project");
      assert.deepStrictEqual(Array.from(root.querySelector("[data-three-page-experiment-signals]").selectedOptions, (option: any) => option.value), ["pv", "bias"]);
      assert.deepStrictEqual(selections, [{ page: "model", persist: false }]);
      assert.strictEqual(envelopes.length, 1, "The existing workflow adapter must synchronize the restored controls");
      assert.strictEqual(changes, 0, "Restoration must not replay change handlers that invalidate or write");
      await Promise.resolve();
      assert.strictEqual(dropdown.trigger.textContent, "Process value, Sensor difference", "The visible dropdown must match the restored chart signals");
    } finally { dom.window.close(); }
  });
  test("restoring hidden project signal choices does not replace the identified response", () => {
    const dom = new JSDOM(`<main data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">
      <select data-engineer-acquisition-method><option value="project">Project experiment</option><option value="recorded-fit">Recorded-data fit</option></select>
      <select data-three-page-experiment-signals multiple><option value="pv">Temperature</option></select></main>`);
    Object.assign(globalThis, { window: dom.window, CustomEvent: dom.window.CustomEvent });
    try {
      const root = dom.window.document.querySelector("main")!;
      let projectProjections = 0;
      dom.window.addEventListener("trust-control-studio-experiment-signals", () => projectProjections++);
      bindThreePageHostState(root, () => undefined, () => undefined);
      const restore = (acquisitionMethod: string) => dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
        schemaVersion: 1, kind: "control-studio.navigation.restore", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r",
        stage: "model", view: { acquisitionMethod, experimentSignalIds: ["pv"] } } }));
      restore("recorded-fit");
      assert.strictEqual(projectProjections, 0, "The hidden experiment selector must not replace a restored model-fit plot");
      assert.strictEqual((root.querySelector("[data-three-page-experiment-signals]") as HTMLSelectElement).value, "pv");
      restore("project");
      assert.strictEqual(projectProjections, 1, "The visible project experiment still restores its selected raw signals");
    } finally { dom.window.close(); }
  });
  const originalGlobals = new Map<string, PropertyDescriptor | undefined>();
  setup(() => { for (const key of ["window", "HTMLElement", "CustomEvent"]) originalGlobals.set(key, Object.getOwnPropertyDescriptor(globalThis, key)); });
  teardown(() => { for (const [key, descriptor] of originalGlobals) {
    if (descriptor) Object.defineProperty(globalThis, key, descriptor);
    else delete (globalThis as any)[key];
  } });
  test("keeps failed verification evidence visible and offers a direct return to tuning", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "candidate", stageEvidence: { hasModel: true, hasCandidate: true } });
    section.querySelector("[data-engineer-verify]").click();
    update({ status: "verification-failed", failure: { cause: "settings", nextAction: "retune" },
      stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: false } });
    assert.strictEqual(section.dataset.selectedStage, "verify", "show the failed actual response before asking for another design");
    const next = section.querySelector("[data-engineer-prerequisite-action]");
    assert.ok(!next.hidden && next.classList.contains("trust-button--primary"));
    next.click();
    assert.strictEqual(section.dataset.selectedStage, "tune");
    assert.ok(section.querySelector("[data-engineer-tune]").classList.contains("trust-button--primary"), "the next action must agree with the recovery instruction");
    dom.window.close();
  });
  test("shows the manual P gain editor only when the engineer chooses a specific gain", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "model-ready", stageEvidence: { hasModel: true } });
    section.querySelector("[data-engineer-family]").value = "P";
    section.querySelector('[data-engineer-stage-tab="tune"]').click();
    const mode = section.querySelector("[data-engineer-gain-mode]");
    const input = section.querySelector("[data-engineer-gain-value]");
    assert.ok(input.closest("label").hidden, "Automatic gain selection must not display an unrelated fixed gain value");
    mode.value = "manual"; mode.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
    assert.ok(!input.closest("label").hidden && !input.disabled, "specific gain exposes an editable value");
    input.value = "2";
    mode.value = "automatic"; mode.dispatchEvent(new dom.window.Event("change", { bubbles: true }));
    assert.ok(input.closest("label").hidden);
    assert.strictEqual(input.value, "2", "retain the engineer's manual value when switching modes");
    dom.window.close();
  });
  test("clears a previous validation error when a replacement model has no fit evidence", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "model-ready", model: { source: "fitted", revision: "old" }, identificationPresentation: { validation: { normalizedError: 0.028 } }, stageEvidence: { hasModel: true } });
    assert.match(section.querySelector("[data-engineer-readiness]").textContent, /2\.8%/);
    update({ status: "model-ready", model: { source: "fitted", revision: "new" }, stageEvidence: { hasModel: true } });
    assert.doesNotMatch(section.querySelector("[data-engineer-readiness]").textContent, /Validation error/);
    dom.window.close();
  });
  test("shows the pending confirmation without duplicate review actions", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "review", rollback: true, stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } });
    assert.strictEqual(section.querySelector("[data-engineer-readiness]").textContent, "Ready to restore");
    assert.ok(section.querySelector("[data-engineer-confirm]").classList.contains("trust-button--primary"));
    assert.ok(section.querySelector("[data-engineer-apply]").hidden, "a pending review needs its confirmation, not another disabled review button");
    assert.ok(section.querySelector("[data-engineer-rollback]").hidden, "do not offer to review the rollback while its confirmation is pending");
    dom.window.close();
  });
  test("labels ordinary review as ready to apply while preserving its confirmation action", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "review", rollback: false, stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } });
    assert.strictEqual(section.querySelector("[data-engineer-readiness]").textContent, "Ready to apply");
    assert.ok(section.querySelector("[data-engineer-confirm]").classList.contains("trust-button--primary"));
    assert.ok(section.querySelector("[data-engineer-apply]").hidden, "review must wait for confirmation rather than exposing Apply again");
    dom.window.close();
  });
  test("keeps completed tuning and verification visible until the next explicit action", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("°C", true, "%", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "model-ready", stageEvidence: { hasModel: true } });
    section.querySelector("[data-engineer-use-model]").click();
    update({ status: "candidate", stageEvidence: { hasModel: true, hasCandidate: true } });
    assert.strictEqual(section.dataset.selectedStage, "tune", "keep the tuned response visible");
    assert.ok(section.querySelector("[data-engineer-verify]").classList.contains("trust-button--primary"));
    section.querySelector("[data-engineer-verify]").click();
    assert.strictEqual(section.dataset.selectedStage, "verify");
    update({ status: "verified", stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } });
    assert.strictEqual(section.dataset.selectedStage, "verify", "keep actual verification results visible");
    assert.ok(section.querySelector("[data-engineer-apply]").classList.contains("trust-button--primary"));
    section.querySelector("[data-engineer-apply]").click();
    assert.strictEqual(section.dataset.selectedStage, "apply");
    dom.window.close();
  });
  test("does not let delayed restoration overwrite a stage the engineer already selected", () => {
    const dom = new JSDOM(`<div data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">${engineerWorkflowControls("m", true, "V", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    navigation.initializeEngineerNavigation(section, () => {});
    const restore = (stage: string) => dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: {
      kind: "control-studio.navigation.restore", projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r", stage,
    } }));
    restore("verify");
    assert.strictEqual(section.dataset.selectedStage, "verify");
    section.querySelector("[data-engineer-stage-tab=tune]").click();
    restore("model");
    assert.strictEqual(section.dataset.selectedStage, "tune", "a delayed stored stage cannot replace the current user choice");
    dom.window.close();
  });
  test("does not offer proportional settings for incompatible or project-owned adapters", () => {
    for (const projectOwned of [false, true]) {
      const loop = projectOwned ? JSON.parse(require("fs").readFileSync(require("path").resolve(__dirname,
        "../../../src/test/fixtures/control-studio-engineer/control-loop.json"), "utf8")) : undefined;
      if (loop) loop.controller.project = {};
      const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("m", true, "V", false, loop)}</div>`);
      Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
      const section = dom.window.document.querySelector("[data-engineer-workflow]");
      if (!projectOwned) section.querySelector('[data-engineer-family] option[value=P]').disabled = true;
      const update = navigation.initializeEngineerNavigation(section, () => {});
      update({ status: "model-ready", stageEvidence: { hasModel: true } });
      section.querySelector('[data-engineer-stage-tab=tune]').click();
      assert.strictEqual(section.querySelector('[data-engineer-gain-controls]').hidden, true,
        projectOwned ? "project-owned tuning uses its declared parameters" : "an adapter without P has no proportional gain control");
      dom.window.close();
    }
  });
  test("keeps tuning blocked when an inspectable model has failed qualification", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("m", true, "V", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    update({ status: "model-ready", model: { source: "fitted" }, stageEvidence: { hasModel: true } });
    update({ status: "unavailable", model: { source: "provided" }, stageEvidence: { hasModel: false } });
    assert.strictEqual(section.querySelector("[data-engineer-tune]").disabled, true);
    assert.strictEqual(section.querySelector("[data-engineer-use-model]").hidden, true);
    assert.ok(section.querySelector("[data-engineer-find]").classList.contains("trust-button--primary"));
    dom.window.close();
  });
  test("keeps ordinary history in Details and exposes recovery inline", () => {
    const dom = new JSDOM(`<div data-control-studio>${engineerWorkflowControls("m", true, "V", true)}</div>`);
    Object.assign(globalThis, { window: dom.window, HTMLElement: dom.window.HTMLElement, CustomEvent: dom.window.CustomEvent });
    const section = dom.window.document.querySelector("[data-engineer-workflow]");
    const update = navigation.initializeEngineerNavigation(section, () => {});
    const notice = section.querySelector("[data-engineer-status]");
    notice.textContent = "Historical study restored";
    update({ status: "restored", stageEvidence: { hasModel: true } });
    assert.ok(notice.closest("[data-engineer-results]"));
    assert.strictEqual(notice.closest("details").open, false);
    notice.textContent = "Recovery remains unconfirmed";
    update({ status: "recovery-required", cancellationUnconfirmed: true });
    assert.strictEqual(notice.parentElement, section);
    assert.strictEqual(notice.getAttribute("role"), "alert");
    update({ status: "restored", commissioning: { recoveryConfirmed: false } });
    assert.strictEqual(notice.parentElement, section, "Restored unresolved recovery must not be hidden in Details");
    assert.strictEqual(section.querySelector("[data-engineer-apply]").disabled, true);
  });

  test("persists only a scoped stage selection and restores it for the same loop revision", async () => {
    const loop = { projectId: "project", runtimeId: "sim", id: "loop", revision: "loop-v1" } as any;
    const entries = new Map<string, unknown>();
    const context = { workspaceState: {
      get: (key: string) => entries.get(key),
      update: async (key: string, value: unknown) => { entries.set(key, value); },
    } } as any;
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "loop-v1" };
    assert.strictEqual(restoredEngineerStage(context, loop), "model");
    assert.strictEqual(await storeEngineerStage(context, loop, {
      kind: "control-studio.navigation.select", schemaVersion: 1, projectId: "project", editorSessionId: "editor",
      loopId: "loop", expectedLoopRevision: "loop-v1", payload: { stage: "verify" },
    }, scope), true);
    assert.strictEqual(restoredEngineerStage(context, loop), "verify");
    assert.strictEqual(await storeEngineerStage(context, loop, {
      kind: "control-studio.navigation.select", schemaVersion: 1, projectId: "other", editorSessionId: "editor",
      loopId: "loop", expectedLoopRevision: "loop-v1", payload: { stage: "apply" },
    }, scope), false);
    assert.strictEqual(restoredEngineerStage(context, loop), "verify", "a wrong-scope navigation message must not overwrite the saved stage");
  });

  test("keeps model alternatives directly accessible and routes model recovery", () => {
    const api = navigation;
    assert.strictEqual(typeof api.initializeEngineerNavigation, "function", "guided workflow must preserve access to earlier choices");
    const dom = new JSDOM(`<div data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">${engineerWorkflowControls("m", true, "V", true)}</div>`);
    const doc = dom.window.document;
    (globalThis as any).window = dom.window;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    (globalThis as any).CustomEvent = dom.window.CustomEvent;
    const section = doc.querySelector("[data-engineer-workflow]");
    const find = section.querySelector("[data-engineer-find]");
    const supply = section.querySelector("[data-engineer-supply]");
    const updateNavigation = api.initializeEngineerNavigation(section, () => {});
    updateNavigation({ status: "model-ready", model: { revision: "model-1" }, stageEvidence: { hasModel: true } });
    assert.ok(!section.querySelector("[data-engineer-options]").contains(find));
    assert.ok(!section.querySelector("[data-engineer-options]").contains(supply));
    assert.ok(section.querySelector("[data-engineer-actions]").contains(find));
    assert.ok(section.querySelector("[data-engineer-actions]").contains(supply));
    assert.strictEqual(section.querySelectorAll("[data-engineer-find]").length, 1);
    assert.ok(section.querySelector("[data-engineer-use-model]").classList.contains("trust-button--primary"));
    section.querySelector("[data-engineer-use-model]").click();
    assert.strictEqual(section.dataset.selectedStage, "tune");
    updateNavigation({ status: "candidate", stageEvidence: { hasModel: true, hasCandidate: true } });
    assert.ok(section.querySelector("[data-engineer-verify]").classList.contains("trust-button--primary"));
    section.querySelector("[data-engineer-verify]").click();
    updateNavigation({ status: "verified", stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: true } });
    assert.ok(section.querySelector("[data-engineer-apply]").classList.contains("trust-button--primary"));
    updateNavigation({ status: "applied", stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false } });
    assert.strictEqual(section.querySelector(".trust-button--primary").textContent, "View results", "After application the next action opens the retained results");
    assert.ok(!section.querySelector("[data-engineer-actions]").contains(section.querySelector("[data-engineer-prerequisite-action]:not([hidden])")));
    updateNavigation({ status: "failed", failure: { nextAction: "find-model" }, stageEvidence: { hasModel: false, hasCandidate: false } });
    assert.ok(section.querySelector("[data-engineer-actions]").contains(find));
    assert.ok(find.classList.contains("trust-button--primary"));
    assert.strictEqual(section.querySelectorAll(".trust-button--primary").length, 1);
    const apply = section.querySelector("[data-engineer-apply]");
    apply.disabled = false;
    updateNavigation({ status: "running" });
    assert.strictEqual(apply.disabled, true, "starting a new operation removes the old Apply affordance");
  });

  test("keeps one primary next action after edits and during cancellation reconciliation", () => {
    const api = navigation;
    const dom = new JSDOM(`<div data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r">${engineerWorkflowControls("m", true, "V", true)}</div>`);
    const doc = dom.window.document;
    (globalThis as any).window = dom.window;
    (globalThis as any).HTMLElement = dom.window.HTMLElement;
    (globalThis as any).CustomEvent = dom.window.CustomEvent;
    const section = doc.querySelector("[data-engineer-workflow]");
    const updateNavigation = api.initializeEngineerNavigation(section, () => {});
    updateNavigation({ status: "edited", stageEvidence: { hasModel: true, hasCandidate: false } });
    assert.ok(section.querySelector("[data-engineer-use-model]").classList.contains("trust-button--primary"),
      "edited model inputs must expose Use model before Tune");
    section.querySelector("[data-engineer-use-model]").click();
    assert.ok(section.querySelector("[data-engineer-tune]").classList.contains("trust-button--primary"),
      "Use model must direct the engineer to Tune");
    assert.strictEqual(section.querySelectorAll(".trust-button--primary").length, 1);
    updateNavigation({ status: "restored", stageEvidence: { hasModel: true, hasCandidate: false } });
    assert.ok(section.querySelector("[data-engineer-tune]").classList.contains("trust-button--primary"),
      "a restored study with no current candidate must direct the engineer to Tune");
    assert.strictEqual(section.querySelectorAll(".trust-button--primary").length, 1);
    updateNavigation({ status: "recovery-required", cancellationUnconfirmed: true });
    assert.ok(section.querySelector("[data-engineer-cancel]").classList.contains("trust-button--primary"),
      "an unconfirmed cancellation must expose Cancel to reconcile the owned operation");
    assert.strictEqual(section.querySelectorAll(".trust-button--primary").length, 1);
  });
});
