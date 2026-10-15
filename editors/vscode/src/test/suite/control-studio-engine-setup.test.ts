import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopMessageScope } from "../../controlStudio/messages";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { SetupDiscoverRequest } from "../../controlStudio/generated/SetupDiscoverRequest";
import type { SetupDiscoverResult } from "../../controlStudio/generated/SetupDiscoverResult";
import type { SetupSaveRequest } from "../../controlStudio/generated/SetupSaveRequest";
import type { SetupSaveResult } from "../../controlStudio/generated/SetupSaveResult";
import { engineAnswer, refusalText, saveSetupDraft, setupSaveOutcome } from "../../controlStudio/loopSetup/setupEngine";
import { ThreePageBindingRoute } from "../../controlStudio/threePageBindingRoute";

/**
 * The loop setup's engine route (specification 35, B1.2, B1.3 and §0.1a `setupDiscover` and `setupSave`): the three-page binding list
 * and selection, and the outcome of a setup save. A fake engine answers each call, so these tests prove the wiring and what is shown;
 * the rules themselves are Rust tests.
 */

const SCOPE: LoopMessageScope = {
  projectId: "project:tank",
  editorSessionId: "session-1",
  loopId: "loop:tank",
  loopRevision: "sha256:loop-1",
};

const LOOP = { id: "loop:tank", name: "Tank level", projectId: SCOPE.projectId, runtimeId: "runtime:local" } as unknown as ControlLoopDefinition;

/** The loop file the engine names for the selection: its definition as the file holds it. */
const LOOP_FILE = { id: "loop:tank", name: "Tank level", projectId: SCOPE.projectId, runtimeId: "runtime:local" };

const SETTINGS_REFUSAL: Failure = {
  message: "The loop has no such controller.",
  cause: "settings",
  nextAction: "open-loop-setup",
  nextActionLabel: "Loop setup…",
};

const LISTED_LOOP = {
  loopId: "loop:tank",
  name: "Tank level",
  configurationPath: "/project/control-loop.json",
  controller: { instance: "Main.TankController", type: "Control.PID", tasks: [] },
  inputs: [],
  outputs: [],
  controllerCompatibility: { kind: "unconfigured" },
};

const REJECTION = { configurationPath: "/project/control-loops/old.json", reason: "The loop is of another runtime." };

const SELECTION = {
  loopId: "loop:tank",
  controllerInstance: "Main.TankController",
  inputIds: ["manipulatedVariable"],
  outputIds: ["processVariable"],
};

interface OpenedSession {
  request: unknown;
  sessionId: string;
  family?: string;
}

/** A discovery the engine answers, with the fields the test names. */
function discovered(fields: Record<string, unknown> = {}): EngineAnswer<SetupDiscoverResult> {
  return { result: { loops: [], rejected: [], ...fields } } as unknown as EngineAnswer<SetupDiscoverResult>;
}

/** A fake engine that records each discovery it is asked for and answers it with `answer`. */
function fakeEngine(answer: (request: SetupDiscoverRequest) => EngineAnswer<SetupDiscoverResult>) {
  const requests: SetupDiscoverRequest[] = [];
  const engine = {
    setupDiscover: async (request: SetupDiscoverRequest) => {
      requests.push(request);
      return answer(request);
    },
  } as unknown as ControlStudioEngine;
  return { engine, requests };
}

function routeWith(engine: ControlStudioEngine, opened: OpenedSession[] = []): ThreePageBindingRoute {
  return new ThreePageBindingRoute(LOOP, SCOPE, undefined, () => true, () => undefined,
    async (request, sessionId, family) => {
      opened.push({ request, sessionId, family });
    },
    { engine, projectRoot: "/project" });
}

/** A page message of the loop's scope, as the three-page panel receives it. */
function message(kind: string, payload: Record<string, unknown>, scope: LoopMessageScope = SCOPE): Record<string, unknown> {
  return {
    schemaVersion: 1,
    kind,
    messageId: `${kind}-${JSON.stringify(payload)}`,
    projectId: scope.projectId,
    editorSessionId: scope.editorSessionId,
    loopId: scope.loopId,
    expectedLoopRevision: scope.loopRevision,
    payload,
  };
}

suite("Control Studio engine setup", () => {
  let folder = "";

  setup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-setup-"));
  });

  teardown(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  test("lists the loops the engine discovers for the binding page", async () => {
    const { engine, requests } = fakeEngine(() => discovered({ loops: [LISTED_LOOP], rejected: [REJECTION] }));
    const posted: Array<Record<string, unknown>> = [];
    const handled = await routeWith(engine).handle(message("control-studio.binding.list", {}), async payload => {
      posted.push(payload);
    });
    assert.strictEqual(handled, true);
    assert.deepStrictEqual(requests, [{ projectRoot: "/project", runtimeId: "runtime:local" }]);
    assert.deepStrictEqual(posted, [{ status: "bindings", targets: [LISTED_LOOP], rejected: [REJECTION] }]);
  });

  test("posts the engine's refusal of a listing with its next action", async () => {
    const { engine } = fakeEngine(() => ({ failure: SETTINGS_REFUSAL }));
    const posted: Array<Record<string, unknown>> = [];
    await routeWith(engine).handle(message("control-studio.binding.list", {}), async payload => {
      posted.push(payload);
    });
    assert.deepStrictEqual(posted, [{ status: "failed", reason: "The loop has no such controller. Next: Loop setup…" }]);
  });

  test("leaves the messages of the other stages to their routes, and asks the engine nothing for them", async () => {
    const { engine, requests } = fakeEngine(() => discovered());
    const handled = await routeWith(engine).handle(message("control-studio.identify.run", { model: "first_order" }), async () => undefined);
    assert.strictEqual(handled, false);
    assert.deepStrictEqual(requests, []);
  });

  test("consumes a binding message of another loop without asking the engine", async () => {
    const { engine, requests } = fakeEngine(() => discovered());
    const handled = await routeWith(engine).handle(message("control-studio.binding.list", {}, { ...SCOPE, loopId: "loop:other" }), async () => undefined);
    assert.strictEqual(handled, true);
    assert.deepStrictEqual(requests, []);
  });

  test("selects a binding through the engine: the selection and the family go to it, and the file it names opens as a session", async () => {
    fs.mkdirSync(path.join(folder, "control-loops"), { recursive: true });
    const file = path.join(folder, "control-loops", "tank.json");
    fs.writeFileSync(file, JSON.stringify(LOOP_FILE));
    const { engine, requests } = fakeEngine(() => discovered({ selectedFile: file }));
    const opened: OpenedSession[] = [];
    const posted: Array<Record<string, unknown>> = [];
    const handled = await routeWith(engine, opened).handle(
      message("control-studio.binding.select", { selection: SELECTION, family: "PI", page: "tune" }),
      async payload => {
        posted.push(payload);
      }
    );
    assert.strictEqual(handled, true);
    assert.deepStrictEqual(requests, [{ projectRoot: "/project", runtimeId: "runtime:local", selection: SELECTION, family: "PI" }]);
    assert.strictEqual(opened.length, 1);
    assert.strictEqual(opened[0].family, "PI");
    assert.ok(opened[0].sessionId.length > 0, "the new session has its own identifier");
    assert.deepStrictEqual(opened[0].request, {
      schemaVersion: 1,
      source: "ConfiguredLoop",
      scope: { projectId: SCOPE.projectId, runtimeId: "runtime:local" },
      loopDefinition: LOOP_FILE,
    });
    assert.deepStrictEqual(posted, []);
  });

  test("posts a selection the engine refuses to the page with its next action, and opens nothing", async () => {
    const { engine } = fakeEngine(() => ({ failure: SETTINGS_REFUSAL }));
    const opened: OpenedSession[] = [];
    const posted: Array<Record<string, unknown>> = [];
    await routeWith(engine, opened).handle(message("control-studio.binding.select", { selection: SELECTION, page: "model" }), async payload => {
      posted.push(payload);
    });
    assert.deepStrictEqual(posted, [{ status: "failed", page: "model", reason: "The loop has no such controller. Next: Loop setup…" }]);
    assert.deepStrictEqual(opened, []);
  });

  test("reads the engine's save answer as the file it wrote, a parked structure, or a refusal", () => {
    const saved: EngineAnswer<SetupSaveResult> = {
      result: { loops: [{ id: "loop:tank", name: "Tank level", file: "control-loops/tank-level.json" }] },
    };
    assert.deepStrictEqual(setupSaveOutcome(saved), { kind: "saved", file: "control-loops/tank-level.json" });
    assert.deepStrictEqual(setupSaveOutcome({ failure: { ...SETTINGS_REFUSAL, cause: "controller-capability", nextAction: "none" } }), { kind: "parked" });
    assert.deepStrictEqual(setupSaveOutcome({ failure: SETTINGS_REFUSAL }), { kind: "refused", failure: SETTINGS_REFUSAL });
    assert.deepStrictEqual(setupSaveOutcome({ result: { loops: [] } }), {
      kind: "refused",
      failure: { message: "The engine wrote no loop.", cause: "unclassified", nextAction: "none" },
    });
  });

  test("shows a refusal with the words of its next action, or with its message alone", () => {
    assert.strictEqual(refusalText(SETTINGS_REFUSAL), "The loop has no such controller. Next: Loop setup…");
    assert.strictEqual(refusalText({ message: "The file is missing.", cause: "unclassified", nextAction: "none" }), "The file is missing.");
  });

  test("saves the feedforward draft the setup form posts unchanged, and a refusal of it is a refusal, never a parked structure (C8)", async () => {
    // The form's draft for a feedforward: the keys are the fields of the engine's FeedforwardLoopDraft (setup/save.rs `parse_setup_draft`).
    const draft = {
      structure: "feedforward", name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: 0, maximum: 80 },
      manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
      disturbance: { symbol: "P1.LoadKw", unit: "kW", minimum: 0, maximum: 50 }, disturbanceCommand: "P1.LoadKw", disturbanceStep: 10,
      feedforward: { gain: "P1.FeedforwardGain", lead: "P1.LeadS", lag: "P1.LagS", filtered: "P1.FilteredLoadKw" },
    };
    assert.deepStrictEqual(Object.keys(draft).sort(), ["action", "disturbance", "disturbanceCommand", "disturbanceStep", "feedforward", "instance",
      "manipulatedVariable", "manualOutput", "mode", "name", "parameterRecord", "processVariable", "setpoint", "structure"],
      "the posted keys are the fields the engine's draft reads, and no other");
    const requests: SetupSaveRequest[] = [];
    const refusal: Failure = { message: "The disturbance step must be positive and at most 50 kW.", cause: "implementation", nextAction: "open-loop-setup" };
    let answer: EngineAnswer<SetupSaveResult> = { result: { loops: [{ id: "loop:water-temperature", name: "Water temperature", file: "control-loops/water-temperature.json" }] } };
    const engine = { setupSave: async (request: SetupSaveRequest) => { requests.push(request); return answer; } } as unknown as ControlStudioEngine;
    const request: SetupSaveRequest = { projectRoot: "/project", runtimeId: "runtime:local", draft };
    assert.deepStrictEqual(await saveSetupDraft(engine, request), { kind: "saved", file: "control-loops/water-temperature.json" });
    assert.deepStrictEqual(requests, [request], "the engine receives the draft as the form posted it");
    answer = { failure: refusal };
    assert.deepStrictEqual(await saveSetupDraft(engine, request), { kind: "refused", failure: refusal },
      "the engine's refusal of a feedforward is shown; the TypeScript generator is not asked");
  });

  test("answers a call that could not be made with an unclassified failure that names its reason", async () => {
    const answer = await engineAnswer<SetupDiscoverResult>(() => Promise.reject(new Error("The engine did not start.")));
    assert.deepStrictEqual(answer, { failure: { message: "The engine did not start.", cause: "unclassified", nextAction: "none" } });
  });
});
