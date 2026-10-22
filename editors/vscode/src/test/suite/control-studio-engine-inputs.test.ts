import * as assert from "assert";
import type { ControlStudioEngine } from "../../controlStudio/engine/engineClient";
import { scopeFields, type EngineLoop, type EngineMethod, type EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createInputsRoute, type InputsStageRoute } from "../../controlStudio/engine/routes/inputsRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { InputsRequest, InputsSection } from "../../controlStudio/generated/InputsRequest";
import type { InputsResult } from "../../controlStudio/generated/InputsResult";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { ENGINE_INPUTS_ANSWERED_EVENT, ENGINE_INPUTS_EVENT, inputsReceiver } from "../../controlStudio/webview/engineInputs";
import { receiveEngineMessage, type EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { ThreePageController } from "../../controlStudio/webview/threePageController";

const { JSDOM } = require("jsdom");

/** The loop the engine serves in these tests, and the page scope of its messages. */
const loop: EngineLoop = { id: "control-pid-commissioning", runtimeId: "simulator" };
const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-test", loopId: loop.id, loopRevision: "revision-test" };
const projectRoot = "/project";
/** The journal folder the panel names; the engine reads there whether an Apply's checkpoint waits for its Restore (B6.4). */
const storageFolder = "/journals";

type Reply = { result: InputsResult } | { failure: Failure };

/** The evidence behind the stages of a bound loop with nothing done yet; these tests read the inputs, not the stages. */
const stageEvidence: InputsResult["stageEvidence"] = {
  loopBound: true, hasModel: false, hasDeclaredAnalysis: false, hasManualParameters: false, hasCandidate: false, hasRecording: false,
  identificationFailed: false, validationHasResult: false, validationPassed: false, validationInconclusive: false, controllerStagesApply: true,
};

/** The engine's answer to a change that leaves the validations current (A8.4): the inputs the engine stored, and whether it changed them. */
function answered(inputs: InputsSection, changed = true, validation = false): InputsResult {
  return { invalidated: { validation }, stages: [], stageEvidence, inputs, changed };
}

const current = answered({}, false);

function acquisition(durationSeconds: number): NonNullable<InputsSection["acquisition"]> {
  return { method: "automatic", durationSeconds, amplitude: 0, reuseRecordings: false };
}

function requirements(): Record<string, unknown> {
  return { referenceChange: 5, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 40, durationSeconds: 90 };
}

/** The inputs the engine recorded for an earlier open of the loop. */
function recorded(): InputsSection {
  return { requirements: { referenceChange: 5, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 40, durationSeconds: 90 }, method: "SIMC" };
}

/** A page message of the served loop's scope that carries `payload`, as the controls of the three-page panel send it. */
function invalidate(payload: Record<string, unknown>, kind = "control-studio.engineer.invalidate"): Record<string, unknown> {
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

interface Harness {
  readonly route: InputsStageRoute;
  /** The requests the fake engine received, in order. */
  readonly requests: InputsRequest[];
  /** The messages the route posted to the page, with their scope fields. */
  readonly posted: Array<Record<string, unknown>>;
  /** The failures the route posted to the page, with the method that failed. */
  readonly failures: Array<{ method: EngineMethod; failure: Failure }>;
  /** Makes the loop a parked one, which the engine does not serve. */
  park(): void;
}

/** A route over a fake engine, which answers each request with `answer`. */
function harness(answer: (request: InputsRequest) => Reply | Promise<Reply> = () => ({ result: current })): Harness {
  const requests: InputsRequest[] = [];
  const posted: Array<Record<string, unknown>> = [];
  const failures: Array<{ method: EngineMethod; failure: Failure }> = [];
  const state: { served: EngineLoop | undefined } = { served: loop };
  const client = {
    inputs: async (request: InputsRequest) => {
      requests.push(structuredClone(request));
      return answer(request);
    },
  } as unknown as ControlStudioEngine;
  const context: EngineRouteContext = {
    client,
    projectRoot,
    storageFolder,
    served: () => state.served,
    post: async (messageScope, message) => {
      posted.push({ ...scopeFields(messageScope), ...message });
    },
    postFailure: async (_messageScope, method, failure) => {
      failures.push({ method, failure });
    },
    ask: async <T>(_messageScope: LoopMessageScope, _method: EngineMethod, run: () => Promise<{ result: T } | { failure: Failure }>) => run(),
    connection: async () => undefined,
    follow: () => undefined,
  };
  return {
    route: createInputsRoute(context),
    requests,
    posted,
    failures,
    park: () => {
      state.served = undefined;
    },
  };
}

suite("Control Studio engine inputs route", () => {
  test("sends a change as the page sent it, and posts the engine's answer", async () => {
    const h = harness();
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope), true);
    assert.deepStrictEqual(h.requests, [
      { change: { acquisitionSettings: acquisition(30) }, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId, storageFolder },
    ]);
    assert.deepStrictEqual(h.failures, []);
    assert.strictEqual(h.posted.length, 1);
    assert.strictEqual(h.posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(h.posted[0].method, "inputs");
    assert.strictEqual(h.posted[0].projectId, scope.projectId);
    assert.deepStrictEqual(h.posted[0].result, current);
  });

  test("sends a requirement change with the scenarios the page declared, and reports the validations it invalidated", async () => {
    const h = harness(() => ({ result: answered({ requirements: recorded().requirements, scenarios: [] }, true, true) }));
    await h.route.handle(invalidate({ ...requirements(), scenarioDefinitions: [] }), scope);
    assert.deepStrictEqual(h.requests[0].change, { ...requirements(), scenarioDefinitions: [] });
    assert.strictEqual((h.posted[0].result as InputsResult).invalidated.validation, true);
  });

  test("posts a refused change as the engine's failure and no result, and the next change as the page sent it", async () => {
    const refusal: Failure = {
      message: "The settling band must be below the error limit.",
      cause: "settings",
      nextAction: "open-requirements",
      nextActionLabel: "Open requirements",
    };
    const h = harness((request): Reply => (request.change?.settlingBand === 0.9 ? { failure: refusal } : { result: answered({ acquisition: acquisition(30) }) }));
    assert.strictEqual(await h.route.handle(invalidate({ ...requirements(), settlingBand: 0.9 }), scope), true);
    assert.deepStrictEqual(h.failures, [{ method: "inputs", failure: refusal }]);
    assert.strictEqual(h.posted.length, 0, "a refused change posts no result");
    await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope);
    assert.deepStrictEqual(h.posted.map((message) => message.result), [answered({ acquisition: acquisition(30) })],
      "the engine's answer to the next change is posted as it is");
  });

  test("sends the changes one at a time, so that each answer is in place before the next change", async () => {
    let unblock: () => void = () => undefined;
    const held = new Promise<void>((resolve) => {
      unblock = () => resolve();
    });
    const h = harness(async (): Promise<Reply> => {
      await held;
      return { result: current };
    });
    const first = h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope);
    const second = h.route.handle(invalidate({ ...requirements() }), scope);
    unblock();
    assert.deepStrictEqual(await Promise.all([first, second]), [true, true]);
    assert.deepStrictEqual(
      h.requests.map((request) => Object.keys(request.change ?? {})),
      [["acquisitionSettings"], ["referenceChange", "maximumError", "settlingBand", "settlingTimeSeconds", "durationSeconds"]]
    );
  });

  test("answers an open with the recorded inputs, which the page's editors take", async () => {
    const h = harness();
    await h.route.opened(recorded(), scope);
    assert.strictEqual(h.posted.length, 1);
    assert.strictEqual(h.posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(h.posted[0].method, "inputs");
    assert.deepStrictEqual(h.posted[0].result, { inputs: recorded() });
    assert.deepStrictEqual(h.requests, [], "an open records nothing");
  });

  test("keeps a parked loop's changes off the engine", async () => {
    const h = harness();
    h.park();
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope), false);
    assert.deepStrictEqual(h.requests, []);
    assert.deepStrictEqual(h.posted, []);
  });

  test("sends a channel mapping as the loop's channel selection, and the rest of the change as it is (A8.1)", async () => {
    const h = harness();
    const mapping = { inputs: [{ modelChannelId: "u1", signalId: "manipulatedVariable" }], outputs: [{ modelChannelId: "y1", signalId: "processVariable" }] };
    assert.strictEqual(await h.route.handle(invalidate({ channelMapping: mapping }), scope), true);
    assert.deepStrictEqual(h.requests, [{ change: {}, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId, storageFolder, channelSelection: mapping }]);
    await h.route.handle(invalidate({ channelMapping: mapping, acquisitionSettings: acquisition(30) }), scope);
    assert.deepStrictEqual(h.requests[1], { change: { acquisitionSettings: acquisition(30) }, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId, storageFolder, channelSelection: mapping });
  });

  test("forwards a binding change and a tuning method the engine does not run, and leaves other stages to the other routes", async () => {
    const h = harness((): Reply => ({ failure: { message: "The engine does not run the tuning method the page chose.", cause: "settings", nextAction: "none" } }));
    assert.strictEqual(await h.route.handle(invalidate({ bindingInvalid: true, bindingInputs: [], bindingOutputs: [] }), scope), true);
    assert.strictEqual(await h.route.handle(invalidate({ tuningPreferences: { method: "state-design" } }), scope), true);
    assert.deepStrictEqual(h.requests.map((request) => request.change), [
      { bindingInvalid: true, bindingInputs: [], bindingOutputs: [] },
      { tuningPreferences: { method: "state-design" } },
    ]);
    assert.strictEqual(h.failures.length, 2, "the engine's refusal of the method is posted");
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }, "control-studio.engineer.tune"), scope), false);
    assert.strictEqual(h.requests.length, 2);
  });

  test("shows a change that invalidated the validations as a stale validation, and no other answer", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = {
        select: () => undefined,
        receive: (stage, payload) => {
          received.push({ stage, payload });
        },
      };
      inputsReceiver.result?.(root, { invalidated: { validation: false }, stages: [] }, ports);
      inputsReceiver.result?.(root, { invalidated: { validation: true }, stages: [] }, ports);
      assert.deepStrictEqual(received.map((item) => [item.stage, item.payload.status]), [["engineer", "edited"]]);
    } finally {
      dom.window.close();
    }
  });

  test("hands the recorded inputs of an open to the page's editors as the inputs event", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      let detail: unknown;
      dom.window.addEventListener(ENGINE_INPUTS_EVENT, (event: { detail: unknown }) => {
        detail = event.detail;
      });
      inputsReceiver.result?.(root, { inputs: recorded() }, { select: () => undefined, receive: () => undefined });
      assert.deepStrictEqual(detail, recorded());
    } finally {
      dom.window.close();
    }
  });

  test("tells the page each answer to a change of the inputs, a refusal included, and none for the echo of the recorded inputs after an open", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const told: Array<{ refused: boolean; limits: boolean }> = [];
      dom.window.addEventListener(ENGINE_INPUTS_ANSWERED_EVENT, (event: { detail: { refused: boolean; limits: boolean } }) => {
        told.push(event.detail);
      });
      const ports: EngineHostPorts = { select: () => undefined, receive: () => undefined };
      inputsReceiver.result?.(root, { inputs: recorded() }, ports);
      inputsReceiver.result?.(root, answered(recorded()), ports);
      inputsReceiver.failure?.(root, { message: "A relay needs its declared direction.", cause: "settings", nextAction: "none" }, ports);
      inputsReceiver.failure?.(root, { message: "The duration is too long.", cause: "settings", nextAction: "open-limits", field: "durationSeconds" }, ports);
      assert.deepStrictEqual(told, [
        { refused: false, limits: false },
        { refused: true, limits: false },
        { refused: true, limits: true },
      ]);
    } finally {
      dom.window.close();
    }
  });

  test("places the refusal of a Channels… selection on the Tune page, and every other failure on the page of its operation (Tune page annex)", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => undefined, receive: (stage, payload) => { received.push({ stage, payload }); } };
      const refusal = { message: "Choose a different signal; this signal is already assigned to another channel.", cause: "settings",
        nextAction: "open-channels", nextActionLabel: "Channels…" };
      /** The page the flow failure names for the failure of `method`, or undefined when it names none. */
      const pageOf = (method: string, failure: Record<string, unknown>): unknown => {
        received.length = 0;
        receiveEngineMessage(root, { kind: "control-studio.engine.failure", method, failure }, ports);
        const flow = received.filter(item => item.stage === "flow");
        assert.strictEqual(flow.length, 1, "the failure still reaches the flow once, so that every listener hears it");
        assert.strictEqual(flow[0].payload.status, "failed");
        return flow[0].payload.page;
      };
      assert.strictEqual(pageOf("inputs", refusal), "tune", "a refused change of the mapping belongs where Channels… is");
      // Each editor's refusal belongs on the page the editor is on, whichever page was shown when the change was sent: Requirements… is on Tune,
      // Limits… on Identify (wrong behaviour caught: a refused limit sent after a Tune run shown on the Tune page).
      assert.strictEqual(pageOf("inputs", { ...refusal, nextAction: "open-requirements", nextActionLabel: "Requirements…" }), "tune");
      assert.strictEqual(pageOf("inputs", { ...refusal, nextAction: "open-limits", nextActionLabel: "Limits…" }), "model");
      // Wrong behaviours caught: moving every failure that names Channels… (a run that the stored mapping refuses keeps the page it was started on),
      // and moving a refusal that reopens no editor.
      assert.strictEqual(pageOf("identify", refusal), undefined);
      assert.strictEqual(pageOf("tune", refusal), undefined);
      assert.strictEqual(pageOf("inputs", { message: "A relay needs its declared direction.", cause: "settings", nextAction: "none" }), undefined);
    } finally {
      dom.window.close();
    }
  });

  test("says why a validation went away in the engine's words, and writes none when the engine sends none (A8.4, B9.9)", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => undefined, receive: (stage, payload) => { received.push({ stage, payload }); } };
      const reasons = (invalidated: InputsResult["invalidated"]): unknown[] => {
        received.length = 0;
        inputsReceiver.result?.(root, { ...answered(recorded(), true, true), invalidated }, ports);
        return received.filter(item => item.stage === "engineer").map(item => item.payload.reason);
      };
      // The sentence is the engine's: a page that wrote "Inputs changed. Tune and verify before applying." itself would show it whatever the engine said.
      assert.deepStrictEqual(reasons({ validation: true, reason: "The engine's sentence." }), ["The engine's sentence."]);
      assert.deepStrictEqual(reasons({ validation: true }), [undefined], "an invalidation without a sentence shows none");
      assert.deepStrictEqual(reasons({ validation: false }), [], "a change that left the validations current says nothing");
    } finally {
      dom.window.close();
    }
  });

  test("shows the engine's status sentence of an identification and of a reset, and writes none of its own (B3.8, A8.4, B9.9)", () => {
    const dom = new JSDOM("<main><p data-three-page-identification-status></p></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const status = root.querySelector("[data-three-page-identification-status]");
      const ports: EngineHostPorts = { select: () => undefined, receive: () => undefined };
      const shown = (result: Record<string, unknown>): string => {
        receiveEngineMessage(root, { kind: "control-studio.engine.result", method: "identify", result: { stages: [], ...result } }, ports);
        return status.textContent;
      };
      // Each sentence below is one the page would otherwise have written: the status line repeats the engine's words, whatever they are.
      assert.strictEqual(shown({ kind: "identification", modelType: "First-order model with delay", provisional: true, statusSentence: "The engine's words for a provisional model." }),
        "The engine's words for a provisional model.");
      assert.strictEqual(shown({ kind: "identification", modelType: "First-order model with delay", provisional: false, statusSentence: "The engine's words for a model." }),
        "The engine's words for a model.");
      assert.strictEqual(shown({ kind: "reset", statusSentence: "The engine's words for a new study." }), "The engine's words for a new study.",
        "a reset answer clears the model and shows the engine's sentence");
      assert.strictEqual(shown({ kind: "identification", modelType: "First-order model with delay" }), "", "an answer without a sentence leaves the status line empty");
    } finally {
      dom.window.close();
    }
  });

  test("hands the analyses the engine lists as stale to the Tune page's analysis, and decides none itself (B9.9)", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => undefined, receive: (stage, payload) => { received.push({ stage, payload }); } };
      const stale = (result: Partial<InputsResult>): unknown[] => {
        received.length = 0;
        inputsReceiver.result?.(root, { ...answered(recorded()), ...result }, ports);
        return received.filter(item => "staleAnalyses" in item.payload).map(item => item.payload.staleAnalyses);
      };
      // The engine names the recorded analysis its change made stale; the page passes the list on as it came.
      assert.deepStrictEqual(stale({ stale: ["poleZero", "frequency"] }), [["poleZero", "frequency"]]);
      // An answer that lists none, and the echo of the recorded inputs after an open, pass nothing: the page drops no answer by a rule of its own.
      assert.deepStrictEqual(stale({}), []);
      assert.deepStrictEqual(stale({ stale: [] }), []);
    } finally {
      dom.window.close();
    }
  });

  test("passes the engine's sentence of a cancelled operation to the flow, for every command, and writes none when the engine sends none", () => {
    const dom = new JSDOM("<main></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports: EngineHostPorts = { select: () => undefined, receive: (stage, payload) => { received.push({ stage, payload }); } };
      const flowOf = (update: Record<string, unknown>): Record<string, unknown> | undefined => {
        received.length = 0;
        receiveEngineMessage(root, { kind: "control-studio.engine.update", update: { state: "cancelled", ...update } }, ports);
        return received.find(item => item.stage === "flow")?.payload;
      };
      assert.deepStrictEqual(flowOf({ command: "tune", message: "The operation was cancelled." }),
        { status: "cancelled", flowStage: "Tune", reason: "The operation was cancelled." });
      assert.deepStrictEqual(flowOf({ command: "validate", message: "The operation was cancelled." }),
        { status: "cancelled", flowStage: "Validate", reason: "The operation was cancelled." });
      assert.deepStrictEqual(flowOf({ command: "identify", message: "Identification cancelled." }),
        { status: "cancelled", flowStage: "Identify", reason: "Identification cancelled." });
      assert.deepStrictEqual(flowOf({ command: "run", message: "The operation was cancelled." }),
        { status: "cancelled", flowStage: undefined, reason: "The operation was cancelled." });
      // A cancelled update without a sentence carries no reason: the page writes none (B9.4).
      assert.deepStrictEqual(flowOf({ command: "tune" }), { status: "cancelled", flowStage: "Tune" });
    } finally {
      dom.window.close();
    }
  });

  test("offers Channels… as the engine's answers say, and the echo of the recorded inputs after an open leaves it as the open answer set it (A8.1)", () => {
    const dom = new JSDOM("<main><button data-engineer-channels hidden>Channels…</button></main>");
    try {
      const root = dom.window.document.querySelector("main");
      const button = root.querySelector("[data-engineer-channels]") as HTMLButtonElement;
      const ports: EngineHostPorts = { select: () => undefined, receive: () => undefined };
      const rows = { inputs: [{ modelChannelId: "u1", signalId: "mv", choices: [] }], outputs: [] };
      inputsReceiver.opened?.(root, { channelChoices: rows }, ports);
      assert.strictEqual(button.hidden, false, "the open answer carries the rows");
      inputsReceiver.result?.(root, { inputs: recorded() }, ports);
      assert.strictEqual(button.hidden, false, "the echo carries the inputs alone, so it is no answer about the rows");
      inputsReceiver.result?.(root, answered(recorded(), false), ports);
      assert.strictEqual(button.hidden, true, "an answer of the engine without rows offers none");
      inputsReceiver.result?.(root, { ...answered(recorded(), false), channelChoices: rows }, ports);
      assert.strictEqual(button.hidden, false, "an answer with rows offers Channels… again");
    } finally {
      dom.window.close();
    }
  });

  test("sends a family that a binding selection chose before the loop opened as a change, and shows the engine's answer (B1.3)", async () => {
    const h = harness((request): Reply => ({ result: answered({ ...recorded(), structure: request.change?.family === "PI" ? "PI" : undefined }) }));
    h.route.bindingFamily("PI");
    await h.route.opened(recorded(), scope);
    assert.deepStrictEqual(h.requests, [{ change: { family: "PI" }, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId, storageFolder }]);
    const shown = h.posted[h.posted.length - 1];
    assert.strictEqual(shown.method, "inputs");
    assert.deepStrictEqual(shown.result, { inputs: { ...recorded(), structure: "PI" } }, "the page shows the family with the recorded inputs");
  });

  test("sends a binding family once, even when the panel resets its stages before it opens the loop", async () => {
    const h = harness();
    h.route.bindingFamily("P");
    h.route.reset?.();
    await h.route.opened(undefined, scope);
    assert.deepStrictEqual(h.requests.map((request) => request.change), [{ family: "P" }]);
    await h.route.opened(recorded(), scope);
    assert.strictEqual(h.requests.length, 1, "a later open does not send the family again");
  });

  test("sends a binding family the study already records, and the engine answers that nothing changed", async () => {
    const h = harness((): Reply => ({ result: answered({ ...recorded(), structure: "PI" }, false) }));
    h.route.bindingFamily("PI");
    await h.route.opened({ ...recorded(), structure: "PI" }, scope);
    assert.deepStrictEqual(h.requests.map((request) => request.change), [{ family: "PI" }]);
    assert.strictEqual((h.posted[0].result as InputsResult).changed, false);
  });

  test("shows the engine's refusal of a binding family, and the inputs the study did record", async () => {
    const refusal: Failure = { message: "The controller does not take this family.", cause: "settings", nextAction: "none" };
    const h = harness((): Reply => ({ failure: refusal }));
    h.route.bindingFamily("PID");
    await h.route.opened(recorded(), scope);
    assert.deepStrictEqual(h.failures, [{ method: "inputs", failure: refusal }]);
    assert.deepStrictEqual(h.posted.map((message) => message.result), [{ inputs: recorded() }], "the open shows what was recorded");
  });

  test("shows the controller family the study recorded, and Automatic when it records none", () => {
    const dom = new JSDOM(
      `<main><select data-studio-controller></select><select data-engineer-tuning-method><option value="automatic">Automatic</option><option value="SIMC">SIMC</option></select></main>`
    );
    const globals = globalThis as unknown as Record<string, unknown>;
    const previous = { document: globals.document, window: globals.window, Option: globals.Option };
    globals.document = dom.window.document;
    globals.window = dom.window;
    globals.Option = dom.window.Option;
    try {
      const root = dom.window.document.querySelector("main") as HTMLElement;
      const select = root.querySelector("[data-studio-controller]") as HTMLSelectElement;
      const controllerLoop = { controller: { instance: "P1.Controller", type: "Control.PID", law: { kind: "pid" } } } as unknown as ConstructorParameters<typeof ThreePageController>[1];
      // The controller the engine offers on open (`controllerOffer`, C2, C4): four families, labelled by their names.
      const families = { controllerOffer: { families: ["P", "PI", "PD", "PID"].map(family => ({ family, label: family })), recommended: null, familySettings: {}, settingRows: [] } };
      const controller = new ThreePageController(root, controllerLoop, () => undefined);
      controller.restoreChoice("PI");
      controller.receive(families);
      assert.strictEqual(select.value, "P1.Controller::PI", "the recorded family is selected");
      controller.restoreChoice(undefined);
      controller.receive(families);
      assert.strictEqual(select.value, "P1.Controller", "Automatic is selected when none is recorded");
    } finally {
      globals.document = previous.document;
      globals.window = previous.window;
      globals.Option = previous.Option;
      dom.window.close();
    }
  });
});
