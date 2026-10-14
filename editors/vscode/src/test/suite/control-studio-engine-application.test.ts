import * as assert from "assert";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext, EngineStageRoute } from "../../controlStudio/engine/engineRouteContext";
import { createApplicationRoute } from "../../controlStudio/engine/routes/applicationRoute";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { applicationReceiver } from "../../controlStudio/webview/engineApplication";
import type { EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import type { StudioFailure } from "../../controlStudio/webview/studioFailure";
import { ThreePageApplication } from "../../controlStudio/webview/threePageApplication";

const { JSDOM } = require("jsdom");

/** The loop the engine serves in the route tests. */
const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-test", loopId: "heater-pid", loopRevision: "revision-test" };
const loop: EngineLoop = { id: "heater-pid", runtimeId: "runtime-test" };

/** The review of an Apply the engine answers (§0.1b `apply`): one parameter, the operating point and the approval sentence. */
const applyReview: Record<string, unknown> = {
  action: "apply",
  identity: "sha256:apply-review",
  description: "Apply writes the validated candidate to Heater.PID.",
  foundMode: "auto",
  physical: false,
  operatingPoint: { inputs: [0.4], outputs: [21.5] },
  parameters: [{ symbol: "Kp", unit: "%", running: 2, next: 3 }],
};

/** The review of a Restore the engine answers: the mode it returns to and the values it returns. */
const restoreReview: Record<string, unknown> = {
  action: "restore",
  identity: "sha256:restore-review",
  description: "Restore returns the previous settings.",
  foundMode: "manual",
  physical: false,
  parameters: [{ symbol: "Kp", unit: "", running: 3, next: 2 }],
};

/** The review of a Keep the engine answers: the record, the values, the file it changes and the field with no setting. */
const keepReview: Record<string, unknown> = {
  record: "Main.TIC101_Parameters",
  values: { Kp: 3 },
  kept: ["Kp"],
  unknown: ["Ti"],
  files: [{ path: "parameters.st", before: "Kp := 2.0;", after: "Kp := 3.0;" }],
  identity: "sha256:keep-review",
};

/** The outcome of a confirmed Apply that passed, with Restore and Keep offered (B6.3, B6.5). */
const applyOutcome: Record<string, unknown> = {
  checkpointId: "checkpoint:1",
  keepAvailable: true,
  restoreAvailable: true,
  restored: false,
  validationIdentity: "validation:1",
  result: { status: "passed", reason: "The response met its requirements." },
};

/** The outcome of a confirmed Restore: the previous settings are back, so neither Restore nor Keep is offered (B6.4, B6.5). */
const restoreOutcome: Record<string, unknown> = {
  restored: true,
  reason: "The previous settings are back.",
  parameters: { active_revision: "active:1" },
  restoreAvailable: false,
  keepAvailable: false,
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
    // The Abort of a served loop is consumed even when no operation runs: the TypeScript session never takes it.
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

  test("only Apply resumes a journaled observation; a Restore that asks for one is sent without it", async () => {
    const h = harness();
    h.engine.answer("apply", { result: { operationId: "op-observe" } });
    h.engine.answer("restore", { result: { operationId: "op-restore" } });
    await h.route.handle(pageMessage("control-studio.application.apply", { resume: true }), scope);
    assert.strictEqual(h.engine.calls[0].request.resume, true);
    await h.route.handle(pageMessage("control-studio.application.restore", { resume: true }), scope);
    assert.ok(!("resume" in h.engine.calls[1].request), "a Restore has nothing to resume");
  });

  test("a Restore is reviewed without an identity and its review is posted as the restore result", async () => {
    const h = harness();
    h.engine.answer("restore", { result: restoreReview });
    await h.route.handle(pageMessage("control-studio.application.restore"), scope);
    assert.ok(!("review" in h.engine.calls[0].request));
    assert.strictEqual(h.posted[0].method, "restore");
    assert.deepStrictEqual(h.posted[0].result, restoreReview);
  });

  test("the recovery's Review restore asks the engine's Restore review, and its Confirm names that review (B6.4)", async () => {
    const h = harness();
    h.engine.answer("restore", { result: restoreReview });
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.engineer.rollback"), scope), true);
    assert.ok(!("review" in h.engine.calls[0].request), "the review is asked for without an identity");
    assert.strictEqual(h.posted[0].method, "restore");
    h.engine.answer("restore", { result: { operationId: "op-restore" } });
    assert.strictEqual(await h.route.handle(pageMessage("control-studio.engineer.confirm"), scope), true);
    assert.strictEqual(h.engine.calls[1].request.review, "sha256:restore-review");
    assert.deepStrictEqual(h.follows, ["op-restore"]);
  });

  test("a Confirm with no Restore review answered asks for the review instead of confirming a guess", async () => {
    const h = harness();
    h.engine.answer("restore", { result: restoreReview });
    await h.route.handle(pageMessage("control-studio.engineer.confirm"), scope);
    assert.ok(!("review" in h.engine.calls[0].request));
  });

  test("Retry recovery and Review restarted runtime ask the engine's Restore review, and a parked loop keeps them", async () => {
    const parked = harness({ parked: true });
    assert.strictEqual(await parked.route.handle(pageMessage("control-studio.engineer.retry-recovery"), scope), false);
    assert.strictEqual(parked.engine.calls.length, 0);
    const served = harness();
    served.engine.answer("restore", { result: restoreReview });
    await served.route.handle(pageMessage("control-studio.engineer.retry-recovery"), scope);
    await served.route.handle(pageMessage("control-studio.engineer.review-runtime"), scope);
    assert.deepStrictEqual(served.engine.calls.map(call => call.method), ["restore", "restore"]);
  });
});

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
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true }, state.ports);
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

  test("on a simulated target the press of Apply is its confirmation: the review's identity is sent at once, and no review is shown", () => {
    // A2.5, B9.3: a simulated target shows no approval block; pressing the action confirms it. The engine still refuses the
    // confirmation when the runtime changed since the review.
    const state = pageState();
    try {
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
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
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true }, state.ports);
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
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
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
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
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
      applicationReceiver.result?.(state.root, { record: "Main.TIC101_Parameters", kept: ["Kp"], unknown: ["Ti"], reason: "Kept Kp." }, state.ports);
      assert.strictEqual(state.keep.hidden, true, "Keep is not offered again once the values are kept");
    } finally { state.restore(); }
  });

  test("a restored Restore offers neither Restore nor Keep in project", () => {
    const state = pageState();
    try {
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      applicationReceiver.result?.(state.root, applyOutcome, state.ports);
      state.apply.click();
      // On a simulated target the press of Restore confirms its review at once (A2.5).
      applicationReceiver.result?.(state.root, restoreReview, state.ports);
      assert.deepStrictEqual(state.posts[state.posts.length - 1], { kind: "control-studio.application.restore", payload: { review: "sha256:restore-review", physicalApproved: false } });
      applicationReceiver.result?.(state.root, restoreOutcome, state.ports);
      assert.strictEqual(state.keep.hidden, true);
      assert.strictEqual(state.apply.textContent, "Apply");
      assert.strictEqual(state.apply.disabled, true);
    } finally { state.restore(); }
  });

  test("a Keep request the engine refuses leaves the Keep button free again", () => {
    const state = pageState();
    try {
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
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
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      state.apply.click();
      assert.strictEqual(state.apply.textContent, "Applying…");
      applicationReceiver.update?.(state.root, { command: "apply", state: "running", progress: "Waiting for the process to settle." }, state.ports);
      assert.ok(state.notices.includes("Waiting for the process to settle."), state.notices.join(" | "));
      assert.strictEqual(state.apply.textContent, "Applying…", "a running Apply keeps its button down");
      applicationReceiver.update?.(state.root, { command: "apply", state: "cancelled" }, state.ports);
      assert.strictEqual(state.apply.textContent, "Apply", "a cancelled Apply no longer reads Applying…");
      assert.strictEqual(state.apply.disabled, false, "the cancelled Apply can be asked again");
      assert.ok(state.notices.includes("The operation was cancelled."), state.notices.join(" | "));
    } finally { state.restore(); }
  });

  test("a cancelled Apply whose settings the engine wrote back ends as cancelled, with the engine's words", () => {
    const state = pageState();
    try {
      state.application.receive({ status: "verified", stageEvidence: { validationPassed: true } });
      state.apply.click();
      applicationReceiver.result?.(state.root, {
        ...applyOutcome,
        restoreAvailable: false,
        keepAvailable: false,
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
      applicationReceiver.opened(state.root, { kind: "opened", restoreAvailable: true, keepAvailable: false, applyObservationResumable: true }, state.ports);
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
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, approval: applyApproval }, state.ports);
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
      applicationReceiver.result?.(state.root, { ...applyReview, physical: true, approval: applyApproval }, state.ports);
      state.root.querySelector<HTMLButtonElement>("[data-engine-cancel-approval]")!.click();
      assert.strictEqual(state.panel()!.hidden, true);
      assert.strictEqual(state.posts.length, 0);
    } finally { state.restore(); }
  });

  test("a failure whose next action is Restore opens the recovery action that asks the engine for the review", () => {
    const state = pageState();
    try {
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => {}, receive: (stage, payload) => { received.push({ stage, payload }); } };
      applicationReceiver.failure?.(state.root, {
        message: "The Restore did not confirm.", cause: "runtime", nextAction: "restore",
      }, ports);
      assert.strictEqual(received[0].stage, "engineer");
      assert.strictEqual(received[0].payload.commissioningCheckpoint, true);
      assert.deepStrictEqual(received[0].payload.failure, { message: "The Restore did not confirm.", nextAction: { kind: "restore", label: "" } });
    } finally { state.restore(); }
  });
});
