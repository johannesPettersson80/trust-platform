import * as assert from "assert";
import type { ControlStudioEngine } from "../../controlStudio/engine/engineClient";
import { scopeFields, type EngineLoop, type EngineMethod, type EngineRouteContext } from "../../controlStudio/engine/engineRouteContext";
import { createInputsRoute, engineHoldsChange, inputsAfterChange, type InputsStageRoute } from "../../controlStudio/engine/routes/inputsRoute";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { InputsRequest, InputsSection } from "../../controlStudio/generated/InputsRequest";
import type { InputsResult } from "../../controlStudio/generated/InputsResult";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { ENGINE_INPUTS_EVENT, inputsReceiver } from "../../controlStudio/webview/engineInputs";
import type { EngineHostPorts } from "../../controlStudio/webview/engineOpened";
import { ThreePageController } from "../../controlStudio/webview/threePageController";

const { JSDOM } = require("jsdom");

/** The loop the engine serves in these tests, and the page scope of its messages. */
const loop: EngineLoop = { id: "control-pid-commissioning", runtimeId: "simulator" };
const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-test", loopId: loop.id, loopRevision: "revision-test" };
const projectRoot = "/project";

type Reply = { result: InputsResult } | { failure: Failure };

/** The engine's answer to a change that leaves the validations current (A8.4). */
const current: InputsResult = { invalidated: { validation: false }, stages: [] };

function acquisition(durationSeconds: number): NonNullable<InputsSection["acquisition"]> {
  return { method: "automatic", durationSeconds, amplitude: 0, reuseRecordings: false };
}

function requirements(): NonNullable<InputsSection["requirements"]> {
  return { referenceChange: 5, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 40, durationSeconds: 90 };
}

/** The inputs the engine recorded for an earlier open of the loop. */
function recorded(): InputsSection {
  return { requirements: requirements(), method: "SIMC" };
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
    storageFolder: "",
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
  test("sends a change as the whole section the engine holds, and posts the engine's answer", async () => {
    const h = harness();
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope), true);
    assert.deepStrictEqual(h.requests, [
      { inputs: { acquisition: acquisition(30) }, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId },
    ]);
    assert.deepStrictEqual(h.failures, []);
    assert.strictEqual(h.posted.length, 1);
    assert.strictEqual(h.posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(h.posted[0].method, "inputs");
    assert.strictEqual(h.posted[0].projectId, scope.projectId);
    assert.deepStrictEqual(h.posted[0].result, current);
  });

  test("keeps the inputs the engine recorded when a change names another input", async () => {
    const h = harness();
    await h.route.opened(recorded(), scope);
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope), true);
    assert.deepStrictEqual(h.requests[0].inputs, { ...recorded(), acquisition: acquisition(30) });
  });

  test("reports a requirement change as invalidating the validations, with the scenarios the page declared", async () => {
    const h = harness((): Reply => ({ result: { invalidated: { validation: true }, stages: [] } }));
    await h.route.handle(invalidate({ ...requirements(), scenarioDefinitions: [] }), scope);
    assert.deepStrictEqual(h.requests[0].inputs, { requirements: requirements(), scenarios: [] });
    assert.strictEqual((h.posted[0].result as InputsResult).invalidated.validation, true);
  });

  test("posts an engine refusal with its next action and label, and records nothing", async () => {
    const refusal: Failure = {
      message: "The settling band must be below the error limit.",
      cause: "settings",
      nextAction: "open-requirements",
      nextActionLabel: "Open requirements",
    };
    const h = harness((): Reply => ({ failure: refusal }));
    assert.strictEqual(await h.route.handle(invalidate({ ...requirements(), settlingBand: 0.9 }), scope), true);
    assert.deepStrictEqual(h.failures, [{ method: "inputs", failure: refusal }]);
    assert.deepStrictEqual(h.posted, []);
    await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope);
    assert.deepStrictEqual(h.requests[1].inputs, { acquisition: acquisition(30) }, "the refused requirement is not in the next change");
  });

  test("sends the changes one at a time, so that each merges into the answer before it", async () => {
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
    assert.strictEqual(h.requests.length, 2);
    assert.deepStrictEqual(h.requests[1].inputs, { acquisition: acquisition(30), requirements: requirements() }, "the second change keeps the first");
  });

  test("exposes the inputs the engine recorded, and only those: a refused change is not among them", async () => {
    const h = harness((request): Reply => (request.inputs.method === "AMIGO" ? { failure: { message: "Refused.", cause: "settings", nextAction: "none" } } : { result: current }));
    await h.route.opened(recorded(), scope);
    await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope);
    assert.deepStrictEqual(h.route.recorded(), { ...recorded(), acquisition: acquisition(30) });
    await h.route.handle(invalidate({ tuningPreferences: { method: "AMIGO" }, pinnedValues: null, family: "automatic" }), scope);
    assert.deepStrictEqual(h.route.recorded(), { ...recorded(), acquisition: acquisition(30) }, "the refused method is not recorded");
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

  test("forgets the recorded inputs when the panel opens another loop", async () => {
    const h = harness();
    await h.route.opened(recorded(), scope);
    h.route.reset?.();
    await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope);
    assert.deepStrictEqual(h.requests[0].inputs, { acquisition: acquisition(30) });
  });

  test("keeps a parked loop's changes off the engine", async () => {
    const h = harness();
    h.park();
    assert.strictEqual(await h.route.handle(invalidate({ acquisitionSettings: acquisition(30) }), scope), false);
    assert.deepStrictEqual(h.requests, []);
    assert.deepStrictEqual(h.posted, []);
  });

  test("leaves a channel mapping, a binding change, an unknown tuning method and other stages to the TypeScript route", async () => {
    const h = harness();
    const notHeld = [
      invalidate({ channelMapping: { inputs: [], outputs: [] } }),
      invalidate({ bindingInvalid: true, family: "PI" }),
      invalidate({ tuningPreferences: { method: "state-design" }, pinnedValues: null, family: "automatic" }),
      invalidate({ tuningPreferences: { values: {} } }),
      invalidate({ acquisitionSettings: acquisition(30) }, "control-studio.engineer.tune"),
    ];
    for (const message of notHeld) assert.strictEqual(await h.route.handle(message, scope), false);
    assert.deepStrictEqual(h.requests, []);
    assert.strictEqual(engineHoldsChange({ tuningPreferences: { method: "manual", values: {} } }), true);
  });

  test("maps the page's family, method, pins and manual settings to the engine's names", () => {
    const manual = inputsAfterChange(
      {},
      { family: "PID", tuningPreferences: { method: "manual", values: { ProportionalGain: 2.5 } }, pinnedValues: { ProportionalGain: 2.5 } },
      "revision-test"
    );
    assert.deepStrictEqual(manual, {
      structure: "PID",
      method: "manual",
      settings: { ProportionalGain: 2.5 },
      pins: { family: "PID", loopRevision: "revision-test", values: { ProportionalGain: 2.5 } },
    });
    const automatic = inputsAfterChange(manual, { family: "automatic", tuningPreferences: { method: "SIMC" }, pinnedValues: null }, "revision-test");
    assert.deepStrictEqual(automatic, { structure: undefined, method: "SIMC", settings: undefined, pins: undefined });
    const requirementChange = inputsAfterChange({}, { ...requirements(), scenarioDefinitions: [] }, "revision-test");
    assert.deepStrictEqual(requirementChange, { requirements: requirements(), scenarios: [] }, "the scenarios are not a requirement");
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

  test("records a family that a binding selection chose before the loop opened, as the loop's structure (B1.3)", async () => {
    const h = harness();
    h.route.bindingFamily("PI");
    await h.route.opened(recorded(), scope);
    assert.deepStrictEqual(h.requests, [
      { inputs: { ...recorded(), structure: "PI" }, loopId: loop.id, projectRoot, runtimeId: loop.runtimeId },
    ]);
    const shown = h.posted[h.posted.length - 1];
    assert.strictEqual(shown.method, "inputs");
    assert.deepStrictEqual(shown.result, { inputs: { ...recorded(), structure: "PI" } }, "the page shows the family with the recorded inputs");
  });

  test("keeps a family that a binding selection chose through the stages' reset that the panel makes before it opens the loop", async () => {
    const h = harness();
    h.route.bindingFamily("P");
    h.route.reset?.();
    await h.route.opened(undefined, scope);
    assert.deepStrictEqual(h.requests[0].inputs, { structure: "P" });
  });

  test("sends no change for a family the study already records, and applies a family once", async () => {
    const h = harness();
    h.route.bindingFamily("PI");
    await h.route.opened({ ...recorded(), structure: "PI" }, scope);
    assert.deepStrictEqual(h.requests, [], "the recorded family stands");
    await h.route.opened(recorded(), scope);
    assert.deepStrictEqual(h.requests, [], "a later open does not apply the family again");
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
      const families = { availableFamilies: ["P", "PI", "PD", "PID"] };
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
