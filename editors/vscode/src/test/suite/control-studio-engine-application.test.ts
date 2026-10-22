import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext, EngineStageRoute } from "../../controlStudio/engine/engineRouteContext";
import { createApplicationRoute } from "../../controlStudio/engine/routes/applicationRoute";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { applicationReceiver } from "../../controlStudio/webview/engineApplication";
import { applyStageStatuses } from "../../controlStudio/webview/engineShared";
import type { EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { ENGINE_STAGE_RECEIVERS } from "../../controlStudio/webview/engineStages";
import type { StudioFailure } from "../../controlStudio/webview/studioFailure";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";

const { JSDOM } = require("jsdom");

/** The loop the engine serves in the route tests. */
const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-test", loopId: "heater-pid", loopRevision: "revision-test" };
const loop: EngineLoop = { id: "heater-pid", runtimeId: "runtime-test" };

/** The review of an Apply the engine answers (§0.1b `apply`): one parameter, the operating point and the approval sentence. */
const applyReview: Record<string, unknown> = {
  kind: "applicationReview",
  action: "apply",
  identity: "sha256:apply-review",
  description: "Apply writes the validated candidate to Heater.PID.",
  foundMode: "auto",
  physical: false,
  confirmation: "press",
  operatingPoint: { inputs: [0.4], outputs: [21.5] },
  parameters: [{ symbol: "Kp", unit: "%", running: 2, next: 3 }],
};

/** The review of a Restore the engine answers: the mode it returns to and the values it returns. */
const restoreReview: Record<string, unknown> = {
  kind: "applicationReview",
  action: "restore",
  identity: "sha256:restore-review",
  description: "Restore returns the previous settings.",
  foundMode: "manual",
  physical: false,
  confirmation: "press",
  parameters: [{ symbol: "Kp", unit: "", running: 3, next: 2 }],
};

/** The review of a Keep the engine answers: the record, the values, the file it changes and the field with no setting. */
const keepReview: Record<string, unknown> = {
  kind: "keepReview",
  record: "Main.TIC101_Parameters",
  values: { Kp: 3 },
  kept: ["Kp"],
  unknown: ["Ti"],
  files: [{ path: "parameters.st", before: "Kp := 2.0;", after: "Kp := 3.0;" }],
  description: "Keep in project writes these running values as the initial values of Main.TIC101_Parameters in the project, so a restart or a new download keeps them.",
  identity: "sha256:keep-review",
  confirmation: "explicit",
};

/** The outcome of a confirmed Apply that passed, with Restore and Keep offered, Keep as the primary action, and no second Apply (B6.3, B6.5, B7). */
const applyOutcome: Record<string, unknown> = {
  kind: "applyOutcome",
  checkpointId: "checkpoint:1",
  keepAvailable: true,
  restoreAvailable: true,
  applyAvailable: false,
  primaryAction: "keep",
  restored: false,
  validationIdentity: "validation:1",
  result: { status: "passed", reason: "The response met its requirements." },
};

/** The outcome of a confirmed Restore: the previous settings are back, so neither Restore nor Keep is offered (B6.4, B6.5). */
const restoreOutcome: Record<string, unknown> = {
  kind: "restoreOutcome",
  restored: true,
  reason: "The previous settings are back.",
  parameters: { active_revision: "active:1" },
  restoreAvailable: false,
  keepAvailable: false,
  applyAvailable: false,
};

/** The approval the engine fills for a physical Apply: the loop, each signal with its limits, the duration, the stop conditions (A2.5). */
const applyApproval: Record<string, unknown> = {
  loopId: "heater-pid",
  signals: [{ symbol: "Kp", unit: "%", running: 2, next: 3, minimum: 0, maximum: 10 }],
  durationSeconds: 12,
  stopConditions: ["A measured output leaves its permitted range: the previous settings are restored at once."],
  restoration: "Restore previous settings returns the parameter record and the mode found before Apply.",
};

/** The engine's methods the route calls: each records its request and answers the test's answer. */
class FakeEngine {
  readonly calls: Array<{ method: string; request: Record<string, unknown> }> = [];
  readonly answers = new Map<string, EngineAnswer<unknown>>();
  readonly listeners = new Map<string, (update: Record<string, unknown>) => void>();

  answer(method: string, answer: EngineAnswer<unknown>): void {
    this.answers.set(method, answer);
  }

  client(): ControlStudioEngine {
    const call = (method: string) => async (request: Record<string, unknown>) => {
      this.calls.push({ method, request });
      return this.answers.get(method) ?? { result: {} };
    };
    const engine = {
      apply: call("apply"),
      restore: call("restore"),
      keep: call("keep"),
      cancel: call("cancel"),
      onUpdate: (operationId: string, listener: (update: Record<string, unknown>) => void) => {
        this.listeners.set(operationId, listener);
        return { dispose: () => { this.listeners.delete(operationId); } };
      },
    };
    return engine as unknown as ControlStudioEngine;
  }
}

interface Harness {
  readonly engine: FakeEngine;
  readonly route: EngineStageRoute;
  /** The messages the route posted to the page, failures included. */
  readonly posted: Array<Record<string, unknown>>;
  /** The operations the route followed, in order. */
  readonly follows: string[];
}

/** A route over a fake engine. `parked` is a loop the engine does not serve; `connection` is the study's runtime connection. */
function harness(options: { parked?: boolean; connection?: { endpoint: string; token?: string } } = {}): Harness {
  const engine = new FakeEngine();
  const posted: Array<Record<string, unknown>> = [];
  const follows: string[] = [];
  const context: EngineRouteContext = {
    client: engine.client(),
    projectRoot: "/project",
    storageFolder: "/journals",
    served: () => (options.parked ? undefined : loop),
    post: async (_scope, message) => {
      posted.push(message);
    },
    postFailure: async (_scope, method, failure) => {
      posted.push({ kind: "control-studio.engine.failure", method, failure });
    },
    ask: async <T>(_scope: LoopMessageScope, method: EngineMethod, run: () => Promise<EngineAnswer<T>>) => {
      try {
        return await run();
      } catch (error) {
        posted.push({ kind: "control-studio.engine.failure", method, failure: { message: String(error) } });
        return undefined;
      }
    },
    connection: async () => options.connection,
    follow: (operationId) => {
      follows.push(operationId);
    },
  };
  return { engine, route: createApplicationRoute(context), posted, follows };
}

/** A page message of the served loop's scope. */
function pageMessage(kind: string, payload: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    schemaVersion: 1,
    kind,
    messageId: "message-test",
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
    payload,
  };
}

suite("Control Studio engine Apply, Restore and Keep route", () => {
  test("an Apply before Validate is refused with the engine's failure, its next action and its label", async () => {
    const h = harness();
    h.engine.answer("apply", {
      failure: { message: "The validated candidate is no longer current.", cause: "model-or-data", nextAction: "validate", nextActionLabel: "Validate" },
    });
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.application.apply"), scope), true);
    assert.strictEqual(h.posted.length, 1);
    assert.strictEqual(h.posted[0].kind, "control-studio.engine.failure");
    assert.strictEqual(h.posted[0].method, "apply");
    assert.deepStrictEqual(h.posted[0].failure, {
      message: "The validated candidate is no longer current.", cause: "model-or-data", nextAction: "validate", nextActionLabel: "Validate",
    });
    assert.deepStrictEqual(h.follows, []);
  });

  test("an Apply review without a runtime is sent without a connection and refused with connect-runtime", async () => {
    const h = harness();
    h.engine.answer("apply", {
      failure: { message: "Connect the runtime first.", cause: "runtime", nextAction: "connect-runtime", nextActionLabel: "Connect runtime" },
    });
    await h.route.handle(pageMessage("control-studio.application.apply"), scope);
    assert.strictEqual(h.engine.calls.length, 1);
    assert.ok(!("connection" in h.engine.calls[0].request), "the request names no connection");
    assert.strictEqual((h.posted[0].failure as { nextAction: string }).nextAction, "connect-runtime");
  });

  test("a confirmation with a stale review identity is sent with that identity and its refusal is posted", async () => {
    const h = harness();
    h.engine.answer("apply", { failure: { message: "The runtime changed since the review.", cause: "runtime", nextAction: "none" } });
    await h.route.handle(pageMessage("control-studio.application.apply", { review: "sha256:stale" }), scope);
    assert.strictEqual(h.engine.calls[0].request.review, "sha256:stale");
    assert.strictEqual(h.posted[0].method, "apply");
    assert.strictEqual((h.posted[0].failure as { nextAction: string }).nextAction, "none");
  });

  test("a loop the engine does not serve keeps its Apply: nothing is sent to the engine", async () => {
    const h = harness({ parked: true });
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.application.apply"), scope), false);
    assert.strictEqual(h.engine.calls.length, 0);
  });

  test("a message of another stage is not consumed and sends nothing", async () => {
    const h = harness();
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.navigation.select", { stage: "model" }), scope), false);
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.flow.start", { from: "verify" }), scope), false);
    assert.strictEqual(h.engine.calls.length, 0);
  });

  test("the review is posted as the apply result, and no message to the page carries the control token", async () => {
    const h = harness({ connection: { endpoint: "tcp://127.0.0.1:9", token: "control-token-secret" } });
    h.engine.answer("apply", { result: applyReview });
    await h.route.handle(pageMessage("control-studio.application.apply"), scope);
    assert.strictEqual(h.posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(h.posted[0].method, "apply");
    assert.deepStrictEqual(h.posted[0].result, applyReview);
    assert.ok(!JSON.stringify(h.posted).includes("control-token-secret"), "the token stays out of the page's messages");
    assert.strictEqual((h.engine.calls[0].request.connection as { token: string }).token, "control-token-secret");
  });

  test("a started Apply is followed, Abort cancels it, and Abort sends nothing once it has ended", async () => {
    const h = harness();
    h.engine.answer("apply", { result: { operationId: "op-7" } });
    h.engine.answer("cancel", { result: {} });
    await h.route.handle(pageMessage("control-studio.application.apply", { review: "sha256:apply-review" }), scope);
    assert.deepStrictEqual(h.follows, ["op-7"]);
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.engineer.cancel"), scope), true);
    assert.deepStrictEqual(h.engine.calls[h.engine.calls.length - 1], { method: "cancel", request: { operationId: "op-7" } });
    h.engine.listeners.get("op-7")?.({ operationId: "op-7", state: "finished" });
    const calls = h.engine.calls.length;
    // The Abort of a served loop is consumed even when no operation runs: no other route takes it.
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.engineer.cancel"), scope), true);
    assert.strictEqual(h.engine.calls.length, calls);
  });

  test("Keep reviews with the open editors' unsaved files, and its confirmation names the review's identity", async () => {
    const h = harness();
    h.engine.answer("keep", { result: keepReview });
    await h.route.handle(pageMessage("control-studio.application.keep"), scope);
    const review = h.engine.calls[0].request;
    assert.ok(Array.isArray(review.unsavedFiles), "the request carries the unsaved files");
    assert.ok(!("review" in review), "the review is asked for without an identity");
    assert.strictEqual(h.posted[0].method, "keep");
    assert.deepStrictEqual(h.posted[0].result, keepReview);
    h.engine.answer("keep", { result: { operationId: "op-keep" } });
    await h.route.handle(pageMessage("control-studio.application.keep", { review: "sha256:keep-review" }), scope);
    assert.strictEqual(h.engine.calls[1].request.review, "sha256:keep-review");
    assert.deepStrictEqual(h.follows, ["op-keep"]);
  });

  test("a Resume is sent as the page asks for it, and the engine's refusal of a Restore that asks for one is shown", async () => {
    // A8.5, A8.6: the engine follows a journaled observation for an Apply and refuses a Resume of a Restore; the route decides neither.
    const h = harness();
    h.engine.answer("apply", { result: { operationId: "op-observe" } });
    await h.route.handle(pageMessage("control-studio.application.apply", { resume: true }), scope);
    assert.strictEqual(h.engine.calls[0].request.resume, true);
    h.engine.answer("restore", { failure: { message: "Invalid request: resume follows a journaled Apply observation; Restore has nothing to resume.",
      cause: "implementation", nextAction: "none" } });
    await h.route.handle(pageMessage("control-studio.application.restore", { resume: true }), scope);
    assert.strictEqual(h.engine.calls[1].request.resume, true, "the route sends what the page asks for");
    assert.strictEqual(h.posted[h.posted.length - 1].kind, "control-studio.engine.failure");
    assert.strictEqual(h.posted[h.posted.length - 1].method, "restore");
    await h.route.handle(pageMessage("control-studio.application.restore"), scope);
    assert.ok(!("resume" in h.engine.calls[2].request), "a request without a Resume names none");
  });

  test("a Restore is reviewed without an identity and its review is posted as the restore result", async () => {
    const h = harness();
    h.engine.answer("restore", { result: restoreReview });
    await h.route.handle(pageMessage("control-studio.application.restore"), scope);
    assert.ok(!("review" in h.engine.calls[0].request));
    assert.strictEqual(h.posted[0].method, "restore");
    assert.deepStrictEqual(h.posted[0].result, restoreReview);
  });

  test("a confirmed Restore review is sent with its identity, and the Restore it starts is followed (B6.4)", async () => {
    // One Restore path: the press of Restore previous settings, or a failure's next action Restore, asks for the review without an
    // identity (above), and its confirmation names the review's identity.
    const h = harness();
    h.engine.answer("restore", { result: { operationId: "op-restore" } });
    await h.route.handle(pageMessage("control-studio.application.restore", { review: "sha256:restore-review" }), scope);
    assert.strictEqual(h.engine.calls[0].request.review, "sha256:restore-review");
    assert.deepStrictEqual(h.follows, ["op-restore"]);
  });
});

/** The offers of a Validate answer that offers Apply, as the Validate stage relays them to the Apply area (B5.4, B7). */
const OFFERED = { status: "offers", engineOutcome: { applyAvailable: true, primaryAction: "apply" } };

/** The Apply stage of an answer that offers Apply: it can start exactly when Apply is offered (B7, B9.4). */
const APPLY_READY = { stage: "Apply", status: "ready", label: "Ready", available: true };

/**
 * A Validate answer that offers Apply, as the page receives it: its stages first (engineShared.ts `applyStageStatuses`), whose Apply
 * stage can start, then its offers, which the Apply area takes.
 */
function offerApply(state: ReturnType<typeof pageState>): void {
  applyStageStatuses(state.root, [APPLY_READY]);
  state.application.receive(OFFERED);
}

/** The page of the three-page panel's Validate area, with the Apply and Keep controls and the application's own area. */
function pageState() {
  const dom = new JSDOM(`<main data-studio-three-page>
    <section data-studio-page="verify"><div data-studio-application hidden>
      <p data-studio-message="verify"></p><button class="trust-button trust-button--primary" data-engineer-apply>Apply</button>
      <button class="trust-button" data-engineer-keep hidden disabled>Keep in project</button>
    </div></section>
    <button data-engineer-find>Identify plant</button><button data-engineer-tune>Tune controller</button>
    <button data-engineer-verify>Validate controller</button>
  </main>`);
  const root = dom.window.document.querySelector("[data-studio-three-page]") as HTMLElement;
  const previous = {
    document: (globalThis as any).document,
    window: (globalThis as any).window,
    HTMLButtonElement: (globalThis as any).HTMLButtonElement,
    HTMLInputElement: (globalThis as any).HTMLInputElement,
  };
  Object.assign(globalThis, {
    document: dom.window.document,
    window: dom.window,
    HTMLButtonElement: dom.window.HTMLButtonElement,
    HTMLInputElement: dom.window.HTMLInputElement,
  });
  const posts: Array<{ kind: string; payload?: Record<string, unknown> }> = [];
  const failures: StudioFailure[] = [];
  const notices: string[] = [];
  const application = new ThreePageApplication(root, (kind, payload) => {
    posts.push({ kind, payload });
  }, () => false, message => {
    notices.push(message);
  }, failure => {
    failures.push(failure);
  });
  const ports: EngineHostPorts = {
    select: () => {},
    receive: (stage, payload) => {
      if (stage === "engineer") application.receive(payload);
    },
  };
  const restore = (): void => {
    Object.assign(globalThis, previous);
    dom.window.close();
  };
  return {
    root,
    application,
    ports,
    posts,
    failures,
    notices,
    restore,
    apply: root.querySelector<HTMLButtonElement>("[data-engineer-apply]")!,
    keep: root.querySelector<HTMLButtonElement>("[data-engineer-keep]")!,
    panel: () => root.querySelector<HTMLElement>(".studio-engine-review"),
    confirm: (action: string) => root.querySelector<HTMLButtonElement>(`[data-engine-confirm="${action}"]`),
    window: dom.window,
  };
}

suite("Control Studio engine Apply, Restore and Keep page", () => {
  test("a physical target's Apply review shows the mode found, each parameter's running and candidate value, the operating point and the description", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, confirmation: "approval" }, state.ports);
      const panel = state.panel()!;
      assert.strictEqual(panel.hidden, false);
      const text = panel.textContent ?? "";
      assert.ok(text.includes("Apply writes the validated candidate to Heater.PID."), text);
      assert.ok(text.includes("Mode found: auto"), text);
      assert.ok(text.includes("Kp: 2 now, writes 3 %"), text);
      assert.ok(text.includes("Validated operating point: inputs 0.4, outputs 21.5"), text);
      assert.strictEqual(state.confirm("apply")?.disabled, true, "the confirmation waits for the engineer's approval (A2.5)");
    } finally { state.restore(); }
  });

  test("a program's own law's Apply review names each variable it writes by its symbol, with the values the engine gives (C2, B6.2)", () => {
    const state = pageState();
    try {
      const review = { ...applyReview, physical: true, confirmation: "approval", parameters: [{ symbol: "P1.Kp", unit: "%", running: 2, next: 3 },
        { symbol: "P1.Ki", unit: "%", running: 0.5, next: 0.8 }] };
      applicationReceiver.result?.(state.root, review, state.ports);
      const text = state.panel()?.textContent ?? "";
      assert.ok(text.includes("P1.Kp: 2 now, writes 3 %"), text);
      assert.ok(text.includes("P1.Ki: 0.5 now, writes 0.8 %"), text);
    } finally { state.restore(); }
  });

  test("on a simulated target the press of Apply is its confirmation: the review's identity is sent at once, and no review is shown", () => {
    // A2.5, B9.3: a simulated target shows no approval block; pressing the action confirms it. The engine still refuses the
    // confirmation when the runtime changed since the review.
    const state = pageState();
    try {
      offerApply(state);
      state.apply.click();
      applicationReceiver.result?.(state.root, applyReview, state.ports);
      assert.deepStrictEqual(state.posts, [
        { kind: "control-studio.application.apply", payload: undefined },
        { kind: "control-studio.application.apply", payload: { review: "sha256:apply-review", physicalApproved: false } },
      ]);
      assert.ok(!state.panel() || state.panel()!.hidden, "no review waits on the screen");
      assert.strictEqual(state.confirm("apply"), null, "no confirmation is asked for");
      assert.strictEqual(state.apply.textContent, "Applying…");
    } finally { state.restore(); }
  });

  test("a physical target needs the engineer's approval before its confirmation is sent", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, confirmation: "approval" }, state.ports);
      const confirm = state.confirm("apply")!;
      assert.strictEqual(confirm.disabled, true);
      confirm.click();
      assert.strictEqual(state.posts.length, 0);
      const approval = state.root.querySelector<HTMLInputElement>("[data-engine-approval]")!;
      approval.checked = true;
      approval.dispatchEvent(new state.window.Event("change"));
      assert.strictEqual(confirm.disabled, false);
      confirm.click();
      assert.deepStrictEqual(state.posts[0].payload, { review: "sha256:apply-review", physicalApproved: true });
    } finally { state.restore(); }
  });

  test("a refused Apply shows its next action with the words the engine gives it", () => {
    const state = pageState();
    try {
      offerApply(state);
      state.apply.click();
      assert.deepStrictEqual(state.posts[0], { kind: "control-studio.application.apply", payload: undefined });
      applicationReceiver.failure?.(state.root, {
        message: "Validate the candidate first.", cause: "model-or-data", nextAction: "validate", nextActionLabel: "Validate",
      }, state.ports);
      assert.deepStrictEqual(state.failures[0], { message: "Validate the candidate first.", nextAction: { kind: "validate", label: "Validate" } });
      assert.strictEqual(state.apply.disabled, false, "the Apply button is free again");
    } finally { state.restore(); }
  });

  test("an Apply outcome offers Restore and Keep in project as the engine answers them, and Keep confirms through its review", () => {
    const state = pageState();
    try {
      offerApply(state);
      applicationReceiver.result?.(state.root, applyOutcome, state.ports);
      assert.strictEqual(state.keep.hidden, false);
      assert.strictEqual(state.apply.textContent, "Restore previous settings");
      state.keep.click();
      assert.deepStrictEqual(state.posts[0], { kind: "control-studio.application.keep", payload: undefined });
      applicationReceiver.result?.(state.root, keepReview, state.ports);
      assert.strictEqual(state.confirm("keep")?.textContent, "Confirm Keep in project");
      const text = state.panel()!.textContent ?? "";
      assert.ok(text.includes("Record: Main.TIC101_Parameters"), text);
      assert.ok(text.includes("Kp: 3"), text);
      assert.ok(text.includes("Changes parameters.st"), text);
      assert.ok(text.includes("Not kept, the equipment has no setting for: Ti"), text);
      state.confirm("keep")!.click();
      assert.deepStrictEqual(state.posts[1], { kind: "control-studio.application.keep", payload: { review: "sha256:keep-review", physicalApproved: false } });
      applicationReceiver.result?.(state.root, { kind: "keepResult", record: "Main.TIC101_Parameters", kept: ["Kp"], unknown: ["Ti"], reason: "Kept Kp.", restoreAvailable: true,
        keepAvailable: false, primaryAction: "restore" }, state.ports);
      assert.strictEqual(state.keep.hidden, true, "Keep is not offered again once the engine says the values are kept");
      assert.strictEqual(state.apply.textContent, "Restore previous settings", "the settings still stand, so the engine names Restore");
    } finally { state.restore(); }
  });

  test("a restored Restore offers neither Restore nor Keep in project", () => {
    const state = pageState();
    try {
      offerApply(state);
      applicationReceiver.result?.(state.root, applyOutcome, state.ports);
      state.apply.click();
      // On a simulated target the press of Restore confirms its review at once (A2.5).
      applicationReceiver.result?.(state.root, restoreReview, state.ports);
      assert.deepStrictEqual(state.posts[state.posts.length - 1], { kind: "control-studio.application.restore", payload: { review: "sha256:restore-review", physicalApproved: false } });
      // The Restore's end answer states the stages it leaves, and the page shows them before the receiver takes the answer (engineOpened.ts):
      // Apply cannot start until a new validation passes (B6.4, B7).
      applyStageStatuses(state.root, [{ ...APPLY_READY, available: false }]);
      applicationReceiver.result?.(state.root, restoreOutcome, state.ports);
      assert.strictEqual(state.keep.hidden, true);
      assert.strictEqual(state.apply.textContent, "Apply");
      assert.strictEqual(state.apply.disabled, true);
    } finally { state.restore(); }
  });

  test("a Keep request the engine refuses leaves the Keep button free again", () => {
    const state = pageState();
    try {
      offerApply(state);
      applicationReceiver.result?.(state.root, applyOutcome, state.ports);
      state.keep.click();
      assert.strictEqual(state.keep.disabled, true);
      applicationReceiver.failure?.(state.root, {
        message: "Save or discard the unsaved changes first.", cause: "settings", nextAction: "none",
      }, state.ports);
      assert.strictEqual(state.keep.disabled, false);
      assert.strictEqual(state.failures[0].message, "Save or discard the unsaved changes first.");
    } finally { state.restore(); }
  });

  test("an Abort of an Apply shows its progress meanwhile and ends its button when the engine reports the cancellation", () => {
    const state = pageState();
    try {
      offerApply(state);
      state.apply.click();
      assert.strictEqual(state.apply.textContent, "Applying…");
      applicationReceiver.update?.(state.root, { command: "apply", state: "running", progress: "Waiting for the process to settle." }, state.ports);
      assert.ok(state.notices.includes("Waiting for the process to settle."), state.notices.join(" | "));
      assert.strictEqual(state.apply.textContent, "Applying…", "a running Apply keeps its button down");
      applicationReceiver.update?.(state.root, { command: "apply", state: "cancelled", message: "The operation was cancelled." }, state.ports);
      assert.strictEqual(state.apply.textContent, "Apply", "a cancelled Apply no longer reads Applying…");
      assert.strictEqual(state.apply.disabled, false, "the cancelled Apply can be asked again");
      assert.ok(state.notices.includes("The operation was cancelled."), state.notices.join(" | "));
      state.apply.click();
      applicationReceiver.update?.(state.root, { command: "apply", state: "cancelled" }, state.ports);
      assert.strictEqual(state.notices[state.notices.length - 1], "", "the page writes no sentence of its own when the engine sends none");
    } finally { state.restore(); }
  });

  test("a cancelled Apply whose settings the engine wrote back ends as cancelled, with the engine's words", () => {
    const state = pageState();
    try {
      offerApply(state);
      state.apply.click();
      applicationReceiver.result?.(state.root, {
        ...applyOutcome,
        restoreAvailable: false,
        keepAvailable: false,
        // Nothing is offered once the previous settings are back, so the engine names no position (B7).
        primaryAction: undefined,
        result: { status: "cancelled", reason: "The Apply was cancelled, and the previous settings are back." },
      }, state.ports);
      assert.strictEqual(state.apply.textContent, "Apply");
      assert.strictEqual(state.failures.length, 0, "a cancellation is not a failure");
      assert.ok(state.notices.includes("The Apply was cancelled, and the previous settings are back."), state.notices.join(" | "));
    } finally { state.restore(); }
  });

  test("a reopened loop offers the Restore and Keep in project the engine answers, and resumes a journaled observation", () => {
    const state = pageState();
    try {
      // The Apply receiver's `opened` hook is what `engineOpened.ts` calls for the opened loop (A8.6).
      assert.ok(applicationReceiver.opened, "the Apply receiver shows an opened loop's offers");
      applicationReceiver.opened(state.root, { kind: "opened", applyAvailable: false, restoreAvailable: true, keepAvailable: false, applyObservationResumable: true,
        primaryAction: "restore" }, state.ports);
      assert.strictEqual(state.apply.textContent, "Restore previous settings");
      assert.strictEqual(state.apply.disabled, false);
      assert.strictEqual(state.keep.hidden, true, "Keep in project is not offered by the engine's answer");
      const resume = state.root.querySelector<HTMLButtonElement>("[data-engine-resume]");
      assert.ok(resume, "the interrupted observation offers Resume");
      resume!.click();
      assert.deepStrictEqual(state.posts[0], { kind: "control-studio.application.apply", payload: { resume: true } });
    } finally { state.restore(); }
  });

  test("a physical Apply's approval names the loop, each signal with its limits, the duration, the stop conditions and the restoration", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, confirmation: "approval", approval: applyApproval }, state.ports);
      const text = state.root.querySelector<HTMLElement>("[data-engine-approval-block]")?.textContent ?? "";
      assert.ok(text.includes("Loop: heater-pid"), text);
      assert.ok(text.includes("Writes Kp: 3 %, limits 0 to 10"), text);
      assert.ok(text.includes("Observed for 12 s"), text);
      assert.ok(text.includes("A measured output leaves its permitted range"), text);
      assert.ok(text.includes("Restoration: Restore previous settings returns"), text);
      assert.ok(state.root.querySelector("[data-engine-cancel-approval]"), "the approval offers Cancel");
    } finally { state.restore(); }
  });

  test("Cancel leaves a physical approval without sending anything to the engine", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, confirmation: "approval", approval: applyApproval }, state.ports);
      state.root.querySelector<HTMLButtonElement>("[data-engine-cancel-approval]")!.click();
      assert.strictEqual(state.panel()!.hidden, true);
      assert.strictEqual(state.posts.length, 0);
    } finally { state.restore(); }
  });

  test("a failure whose next action is Restore reaches the page with that action and no checkpoint of the page's own", () => {
    // B6.7: the failure's typed next action is the control that asks for the Restore review (threePageWorkflow.ts `runNextAction`); the
    // block of the stages is the engine's, in its stage statuses (B6.4), so the receiver derives nothing from the failure.
    const state = pageState();
    try {
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => {}, receive: (stage, payload) => { received.push({ stage, payload }); } };
      applicationReceiver.failure?.(state.root, {
        message: "The Restore did not confirm.", cause: "runtime", nextAction: "restore", nextActionLabel: "Restore previous settings",
      }, ports);
      assert.strictEqual(received[0].stage, "engineer");
      assert.deepStrictEqual(received[0].payload, { status: "failed", reason: "The Restore did not confirm.",
        failure: { message: "The Restore did not confirm.", nextAction: { kind: "restore", label: "Restore previous settings" } } });
    } finally { state.restore(); }
  });

  test("how a review is confirmed is the word the engine gives it: a press confirms at once, even when the review reports a physical target", () => {
    const state = pageState();
    try {
      offerApply(state);
      state.apply.click();
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, confirmation: "press" }, state.ports);
      assert.deepStrictEqual(state.posts[state.posts.length - 1], { kind: "control-studio.application.apply", payload: { review: "sha256:apply-review", physicalApproved: false } });
      assert.strictEqual(state.confirm("apply"), null, "the page reads the engine's word and asks for no approval of its own");
    } finally { state.restore(); }
  });

  test("how a review is confirmed is the word the engine gives it: an approval is asked for, even when the review reports a simulated target", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, { ...applyReview, physical: false, confirmation: "approval" }, state.ports);
      assert.strictEqual(state.confirm("apply")?.disabled, true, "the confirmation waits for the engineer's approval");
      assert.ok(state.root.querySelector("[data-engine-approval]"), "the approval box is shown");
      assert.strictEqual(state.posts.length, 0, "nothing is sent before the engineer approves");
    } finally { state.restore(); }
  });

  test("a Keep review is confirmed with its own button, which asks for no approval", () => {
    const state = pageState();
    try {
      applicationReceiver.result?.(state.root, keepReview, state.ports);
      assert.strictEqual(state.panel()?.querySelector("p")?.textContent, keepReview.description, "the heading is the engine's sentence (B6.5)");
      assert.strictEqual(state.confirm("keep")?.disabled, false);
      assert.strictEqual(state.root.querySelector("[data-engine-approval]"), null, "Keep never writes the controller");
      assert.strictEqual(state.posts.length, 0, "the Keep waits for its button");
    } finally { state.restore(); }
  });

  test("the engine's offers and the action that holds the position pass on unchanged, and the receiver adds nothing of its own", () => {
    const state = pageState();
    try {
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => {}, receive: (stage, payload) => { received.push({ stage, payload }); } };
      applicationReceiver.opened?.(state.root, { kind: "opened", applyAvailable: true, restoreAvailable: false, keepAvailable: false, applyObservationResumable: false,
        primaryAction: "apply" }, ports);
      assert.deepStrictEqual(received[0].payload, { status: "opened",
        engineOutcome: { applyAvailable: true, restoreAvailable: false, keepAvailable: false, primaryAction: "apply" }, applyObservationResumable: false });
      applicationReceiver.result?.(state.root, applyOutcome, ports);
      assert.deepStrictEqual(received[1].payload.engineOutcome, { applyAvailable: false, restoreAvailable: true, keepAvailable: true, primaryAction: "keep" });
      applicationReceiver.result?.(state.root, restoreOutcome, ports);
      assert.deepStrictEqual(received[2].payload, { status: "commissioned", applicationAction: "restore", reason: "The previous settings are back.",
        engineOutcome: { applyAvailable: false, restoreAvailable: false, keepAvailable: false } }, "a confirmed Restore names no position: nothing is offered");
      applicationReceiver.result?.(state.root, { ...restoreOutcome, restored: false, restoreAvailable: true, primaryAction: "restore" }, ports);
      assert.strictEqual(received[3].payload.status, "failed");
      assert.deepStrictEqual(received[3].payload.engineOutcome, { applyAvailable: false, restoreAvailable: true, keepAvailable: false, primaryAction: "restore" });
      applicationReceiver.result?.(state.root, { kind: "keepResult", record: "Main.TIC101_Parameters", kept: ["Kp"], unknown: [], reason: "Kept Kp.", restoreAvailable: true, keepAvailable: false,
        primaryAction: "restore" }, ports);
      assert.deepStrictEqual(received[4].payload.engineOutcome, { restoreAvailable: true, keepAvailable: false, primaryAction: "restore" },
        "a Keep states no Apply flag, and the position the engine names");
    } finally { state.restore(); }
  });

  test("a Validate answer's offers reach the Apply area with its table: Apply and the position as the engine states them (B5.4, B7)", () => {
    const state = pageState();
    try {
      const validate = ENGINE_STAGE_RECEIVERS.get("validate")!;
      const answer = { applyAvailable: true, primaryAction: "apply", stageEvidence: { hasCandidate: true, validationHasResult: true, validationPassed: true },
        stages: [APPLY_READY], table: [] };
      // The page shows an answer's stages before its stage receiver takes it (engineOpened.ts), as it does here.
      applyStageStatuses(state.root, answer.stages);
      validate.result?.(state.root, answer, state.ports);
      assert.deepStrictEqual([state.apply.textContent, state.apply.disabled], ["Apply", false]);
      const withdrawn = { ...answer, applyAvailable: false, primaryAction: "restore", stages: [{ ...APPLY_READY, available: false }] };
      applyStageStatuses(state.root, withdrawn.stages);
      validate.result?.(state.root, withdrawn, state.ports);
      assert.deepStrictEqual([state.apply.textContent, state.apply.disabled], ["Restore previous settings", true],
        "the engine names Restore; the page held no Restore offer from an earlier answer, so the action stays unavailable");
    } finally { state.restore(); }
  });

  test("an inputs answer's offers reach the Apply area, and the echo of the recorded inputs after an open states none (A8.4, B7)", () => {
    // A8.4, B7: the engine's answer to a change of the study inputs states Apply and the action that holds the position by the rule of
    // every answer that states them; the route's echo of the recorded inputs is no answer of the engine and states nothing.
    const state = pageState();
    try {
      const inputs = ENGINE_STAGE_RECEIVERS.get("inputs")!;
      const received: Array<Record<string, unknown>> = [];
      const ports: EngineHostPorts = { select: () => {}, receive: (stage, payload) => { if (stage === "engineer") received.push(payload); } };
      inputs.result?.(state.root, { inputs: {} }, ports);
      assert.deepStrictEqual(received, [], "the echo states no offers");
      inputs.result?.(state.root, { inputs: {}, changed: true, invalidated: { validation: false }, applyAvailable: false, primaryAction: "keep" }, ports);
      assert.deepStrictEqual(received, [{ status: "offers", engineOutcome: { applyAvailable: false, primaryAction: "keep" } }]);
    } finally { state.restore(); }
  });

  test("a commissioning run's three ways to end are the page's applied, cancelled and failed, and a word it does not know is failed", () => {
    const state = pageState();
    try {
      const received: Array<Record<string, unknown>> = [];
      const ports: EngineHostPorts = { select: () => {}, receive: (_stage, payload) => { received.push(payload); } };
      for (const status of ["passed", "cancelled", "failed", "unheard-of"]) {
        applicationReceiver.result?.(state.root, { ...applyOutcome, result: { status, reason: "The run ended.", failure: { message: "It failed.", cause: "settings", nextAction: "restore" } } }, ports);
      }
      assert.deepStrictEqual(received.map(payload => payload.status), ["applied", "cancelled", "failed", "failed"]);
      assert.deepStrictEqual(received.map(payload => "failure" in payload), [false, false, true, true], "only a failed run carries the failure");
    } finally { state.restore(); }
  });
});
