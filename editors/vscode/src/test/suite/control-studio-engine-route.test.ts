import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";

/**
 * The engine binary the test runner names (`ST_CONTROL_STUDIO_TEST_BIN`), else the one the build left in the cargo
 * target folder (CARGO_TARGET_DIR, else `target`).
 */
function engineBinary(): string {
  const named = process.env.ST_CONTROL_STUDIO_TEST_BIN?.trim();
  if (named) {
    if (!fs.existsSync(named)) throw new Error(`ST_CONTROL_STUDIO_TEST_BIN not found at ${named}`);
    return named;
  }
  const repoRoot = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  const targetDir = process.env.CARGO_TARGET_DIR?.trim();
  const folder = targetDir ? path.resolve(repoRoot, targetDir) : path.join(repoRoot, "target");
  const name = process.platform === "win32" ? "trust-control-studio.exe" : "trust-control-studio";
  const binary = path.join(folder, "debug", name);
  if (!fs.existsSync(binary)) {
    throw new Error(`${name} is not built at ${binary}: run cargo build -p trust-control-studio first.`);
  }
  return binary;
}

/** A core loop of the control course: the engine runs it, so the route serves it. */
function coreLoopFile(): string {
  const repoRoot = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  return path.join(repoRoot, "examples/control/06-pid-tuning-methods/30-model-based-pi-tuning/control-loop.json");
}

/** A loop with a declared analysis: the engine parks it, so it stays on the TypeScript route until V7. */
const PARKED_LOOP_FILE = path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json");

interface LoopIdentity {
  readonly id: string;
  readonly runtimeId: string;
  readonly projectId: string;
  readonly revision: string;
}

/**
 * An engine process without a binary: it answers each method the test names, records each request, and relays the update
 * notifications the test sends. The route's own functions run unchanged over it.
 */
function fakeEngine(answers: Record<string, unknown>): {
  readonly engine: EngineProcess;
  readonly requests: Array<{ method: string; params: unknown }>;
  notify(params: unknown): void;
} {
  const requests: Array<{ method: string; params: unknown }> = [];
  const listeners: Array<(params: unknown) => void> = [];
  // A loop without a model offers no analysis tool: the open answer asks for the tools, so each fake answers them (B9.9).
  const scripted: Record<string, unknown> = { "controlStudio/analyze": { result: { tools: [] } }, ...answers };
  const connection = {
    sendRequest: async (method: string, params: unknown) => {
      requests.push({ method, params });
      const answer = scripted[method];
      // An answer the test computes, such as one it holds back until it releases it.
      return typeof answer === "function" ? (answer as () => unknown)() : answer;
    },
  };
  const engine = {
    connection: async () => connection,
    onNotification: (_method: string, listener: (params: unknown) => void) => {
      listeners.push(listener);
      return { dispose: () => undefined };
    },
    dispose: () => undefined,
  };
  return {
    engine: engine as unknown as EngineProcess,
    requests,
    notify: (params) => {
      for (const listener of listeners) listener(params);
    },
  };
}

suite("Control Studio engine route", () => {
  let folder = "";
  let engine: EngineProcess | undefined;
  let route: EnginePanelRoute | undefined;
  let posted: Array<Record<string, unknown>> = [];

  setup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-route-"));
    posted = [];
  });

  teardown(() => {
    route?.dispose();
    engine?.dispose();
    route = undefined;
    engine = undefined;
    fs.rmSync(folder, { recursive: true, force: true });
  });

  /** Copies a loop file to the project folder as its root loop, and returns the loop's identity. */
  function projectLoop(source: string): LoopIdentity {
    fs.copyFileSync(source, path.join(folder, "control-loop.json"));
    return JSON.parse(fs.readFileSync(source, "utf8")) as LoopIdentity;
  }

  function scopeOf(loop: LoopIdentity): LoopMessageScope {
    return { projectId: loop.projectId, editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: loop.revision };
  }

  /** A route over a real engine that the test owns; its messages go to `posted`. */
  function startRoute(): EnginePanelRoute {
    engine = startEngine({ executable: engineBinary() });
    route = createEnginePanelRoute({
      engine,
      projectRoot: folder,
      storageFolder: folder,
      post: (message) => {
        posted.push(message);
      },
    });
    return route;
  }

  function selectMessage(scope: LoopMessageScope, stage: string): Record<string, unknown> {
    return {
      schemaVersion: 1,
      kind: "control-studio.navigation.select",
      messageId: "message-test",
      projectId: scope.projectId,
      editorSessionId: scope.editorSessionId,
      loopId: scope.loopId,
      expectedLoopRevision: scope.loopRevision,
      payload: { stage, view: { acquisitionMethod: "automatic", experimentSignalIds: [] } },
    };
  }

  /** A page command or the study reset, with the scope fields every page message carries. */
  function commandMessage(scope: LoopMessageScope, kind: string, payload: Record<string, unknown>): Record<string, unknown> {
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

  /** A route for the Identify commands and the study reset, without a runtime connection (E11.1). */
  function startIdentifyRoute(): EnginePanelRoute {
    engine = startEngine({ executable: engineBinary() });
    route = createEnginePanelRoute({
      engine,
      projectRoot: folder,
      storageFolder: folder,
      post: (message) => {
        posted.push(message);
      },
    });
    return route;
  }

  /** No message to the page carries a control token (D13). */
  function assertNoToken(): void {
    for (const message of posted) {
      assert.doesNotMatch(JSON.stringify(message), /"(?:token|authToken|controlAuthToken)":/);
    }
  }

  /**
   * A parked loop's page hears its analysis tools and nothing else: the TypeScript route keeps the loop, and the engine's refusal of
   * the loop's own law is not a failure the page shows (B9.9).
   */
  function assertOnlyParkedTools(): void {
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.analysis.tools"], "a parked loop answers its analysis tools only");
    assert.ok(posted.every((message) => !("failure" in message)), "a parked refusal is not a failure the page shows");
  }

  test("opens a loop the engine runs, and answers with its stages and page", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    assert.strictEqual(await startRoute().open(loop, scope), true);
    assert.strictEqual(posted.length, 3, "an open answers with the opened loop, then with the study's recorded inputs and the analysis tools");
    assert.strictEqual(posted[0].kind, "control-studio.engine.opened");
    assert.strictEqual(posted[0].projectId, scope.projectId);
    assert.strictEqual(posted[0].editorSessionId, scope.editorSessionId);
    assert.strictEqual(posted[0].loopId, loop.id);
    assert.strictEqual(posted[0].expectedLoopRevision, loop.revision);
    const result = posted[0].result as { kind: string; page: string; stages: Array<{ stage: string }> };
    assert.strictEqual(result.kind, "opened");
    assert.strictEqual(result.page, "Identify", "a new study opens at Identify (A8.2)");
    assert.deepStrictEqual(
      result.stages.map((stage) => stage.stage),
      ["Monitor", "Test", "Identify", "Tune", "Validate", "Apply"]
    );
    assert.strictEqual(posted[1].kind, "control-studio.engine.result");
    assert.strictEqual(posted[1].method, "inputs");
    assert.strictEqual(posted[2].kind, "control-studio.analysis.tools");
    assert.ok(Array.isArray(posted[2].tools), "the analysis tools that apply reach the page, and no failure does");
    assertNoToken();
  });

  test("stores the page the engineer viewed, so that the next open names it", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    assert.strictEqual(await current.navigation(selectMessage(scope, "tune"), scope), true);
    assert.strictEqual(posted.length, 3, "the page is stored without a message back");
    assert.strictEqual(await current.open(loop, scope), true);
    assert.strictEqual(posted.length, 6, "the next open answers as the first did: the opened loop, the recorded inputs and the analysis tools");
    assert.strictEqual(posted[3].kind, "control-studio.engine.opened");
    assert.strictEqual((posted[3].result as { page: string }).page, "Tune");
    assertNoToken();
  });

  test("keeps a loop the engine parks on the TypeScript route", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    assert.strictEqual(await current.navigation(selectMessage(scope, "tune"), scope), false);
    assert.strictEqual(await current.open(loop, scope), false, "the parked loop stays out of the way");
    assertOnlyParkedTools();
  });

  test("shows an engine that cannot start as its start failure, and keeps the loop on the TypeScript route", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    engine = startEngine({ executable: path.join(folder, "missing-trust-control-studio") });
    route = createEnginePanelRoute({
      engine,
      projectRoot: folder,
      storageFolder: folder,
      post: (message) => {
        posted.push(message);
      },
    });
    assert.strictEqual(await route.open(loop, scopeOf(loop)), false);
    assert.strictEqual(posted.length, 1);
    assert.strictEqual(posted[0].kind, "control-studio.engine.failure");
    assert.strictEqual(posted[0].method, "start");
  });

  test("leaves an Identify run to the TypeScript route, since Identify plant is a run on the engine route and a model-only run is the Recorded-data fit's (B3.3)", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { model: "fopdt" }), scope), false);
    assert.deepStrictEqual(posted, [], "the engine answers nothing for a model-only run");
    assertNoToken();
  });

  test("answers a study reset at once, and posts the stages of the new study to the page", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.study.reset", {}), scope), true);
    assert.strictEqual(posted.length, 1);
    assert.strictEqual(posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(posted[0].method, "identify");
    const result = posted[0].result as { stages: Array<{ stage: string }> };
    assert.deepStrictEqual(
      result.stages.map((stage) => stage.stage),
      ["Monitor", "Test", "Identify", "Tune", "Validate", "Apply"]
    );
    assertNoToken();
  });

  test("keeps the Identify commands of a parked loop on the TypeScript route, out of the engine", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { model: "fopdt" }), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.study.reset", {}), scope), false);
    assertOnlyParkedTools();
  });

  /**
   * A route over a fake engine that serves the loop `tank` and answers `controlStudio/identify` with the answers the test names (a
   * function answers each call in turn). The messages of the open are cleared, so a test reads only the answers to its own request.
   */
  async function servedWith(identify: unknown, answers: Record<string, unknown> = {}): Promise<{ current: EnginePanelRoute; fake: ReturnType<typeof fakeEngine>; scope: LoopMessageScope }> {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const fake = fakeEngine({ "controlStudio/open": openedAnswer([]), "controlStudio/identify": identify, ...answers });
    const current = createEnginePanelRoute({
      engine: fake.engine,
      projectRoot: folder,
      storageFolder: "",
      post: (message) => {
        posted.push(message);
      },
    });
    route = current;
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    return { current, fake, scope };
  }

  /** The identify requests the route sent the fake engine, in order. */
  /** The actions of the `controlStudio/identify` requests the route sent: each request carries its action as `action` (§0.1a). */
  function identifyActionsOf(fake: ReturnType<typeof fakeEngine>): Array<Record<string, unknown>> {
    return fake.requests.filter((sent) => sent.method === "controlStudio/identify")
      .map((sent) => (sent.params as { action: Record<string, unknown> }).action);
  }

  /** The channels of a recording the study keeps: the loop's plant input and measured output, and an archived output of its own. */
  const RECORDING_CATALOG = {
    channels: [{ id: "P1.PV", unit: "degC" }, { id: "P1.RecordedTemperature", unit: "degC" }],
    selection: { inputs: ["P1.MV"], outputs: ["P1.PV"] },
    kind: "archive",
  };

  test("keeps the Recorded-data fit's file and its steps on the TypeScript route, on a loop the engine serves (B3.3)", async function () {
    this.timeout(30_000);
    const { current, fake, scope } = await servedWith({ result: {} });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.identify.import", { json: "{}" }), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { model: "fopdt" }), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.fit", {}), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.accept", {}), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.choose-structure", { choice: "first_order" }), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.edit", { form: {} }), scope), false);
    assert.deepStrictEqual(identifyActionsOf(fake), [], "the engine runs none of the manual steps");
    assert.deepStrictEqual(posted, [], "the engine answers none of them");
  });

  test("answers a recording the engine keeps but cannot yet check as recording-needed, with its reason and channels (B3.6)", async function () {
    this.timeout(30_000);
    const { current, scope } = await servedWith({ result: { status: "recording-needed", recordingId: "rec-1", source: "Simulation", samples: 3,
      recordingCatalog: RECORDING_CATALOG, reason: "Open an independent recording to check the plant model." } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.identification-recording"]);
    assert.strictEqual(posted[0].messageId, "message-test");
    assert.deepStrictEqual(posted[0].payload, { status: "recording-needed", recordingId: "rec-1", recordingCatalog: RECORDING_CATALOG,
      reason: "Open an independent recording to check the plant model." });
    assertNoToken();
  });

  test("answers a pair of recordings the engine fits as checked, with its identification as an Identify answer (B3.6, B5.2)", async function () {
    this.timeout(30_000);
    const identification = { modelType: "First-order model with delay", equation: "G(s) = 2 / (5s + 1)", keyNumbers: "K = 2", provisional: false,
      plot: { timeSeconds: [0, 1], traces: [] }, stages: [], stageEvidence: {} };
    const { current, scope } = await servedWith({ result: { status: "checked", recordingId: "rec-2", recordingCatalog: RECORDING_CATALOG,
      identification, validation: "The model was checked against an independent recording." } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.result", "control-studio.identification-recording"]);
    assert.strictEqual(posted[0].method, "identify");
    assert.deepStrictEqual(posted[0].result, identification);
    assert.deepStrictEqual(posted[1].payload, { status: "checked", recordingId: "rec-2", recordingCatalog: RECORDING_CATALOG,
      validation: "The model was checked against an independent recording." });
    assertNoToken();
  });

  test("fits a lone connected recording the engine keeps at once, with the selection it keeps (B3.6)", async function () {
    this.timeout(30_000);
    let answered = 0;
    const { current, fake, scope } = await servedWith(() => {
      answered += 1;
      return answered === 1 ? { result: { status: "checked", recordingId: "rec-3", recordingCatalog: RECORDING_CATALOG } }
        : { result: { operationId: "operation-21" } };
    });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.identification-recording"]);
    const requests = identifyActionsOf(fake);
    assert.deepStrictEqual(requests.map((request) => request.action), ["importRecording", "openRecording"]);
    assert.deepStrictEqual({ recordingId: requests[1].recordingId, inputs: requests[1].inputs, outputs: requests[1].outputs },
      { recordingId: "rec-3", inputs: ["P1.MV"], outputs: ["P1.PV"] }, "the fit names the recording and the selection the study keeps");
  });

  test("answers a refit of the kept recording with the selection the page names, as an import answers (B3.6)", async function () {
    this.timeout(30_000);
    const identification = { modelType: "First-order model with delay", equation: "G(s) = 2 / (5s + 1)", keyNumbers: "K = 2", provisional: false,
      plot: { timeSeconds: [0, 1], traces: [] }, stages: [], stageEvidence: {} };
    let answered = 0;
    const { current, fake, scope } = await servedWith(() => {
      answered += 1;
      return answered === 1 ? { result: { status: "recording-needed", recordingId: "rec-4", recordingCatalog: RECORDING_CATALOG,
        reason: "Open an independent recording to check the plant model." } }
        : { result: { status: "checked", recordingId: "rec-4", recordingCatalog: RECORDING_CATALOG, identification,
          validation: "The model was checked against an independent recording." } };
    });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    posted = [];
    const selection = { inputs: ["P1.MV"], outputs: ["P1.RecordedTemperature"] };
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.recording.refit", { recordingSelection: selection }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.result", "control-studio.identification-recording"]);
    const refit = identifyActionsOf(fake)[1];
    assert.deepStrictEqual({ action: refit.action, recordingId: refit.recordingId, inputs: refit.inputs, outputs: refit.outputs },
      { action: "openRecording", recordingId: "rec-4", inputs: ["P1.MV"], outputs: ["P1.RecordedTemperature"] });
  });

  test("answers a file the engine refuses as failed, with the engine's sentence, and keeps nothing (B10.4)", async function () {
    this.timeout(30_000);
    const { current, scope } = await servedWith({ failure: { message: "Choose a valid Control Studio recording or result JSON file.", cause: "unclassified", nextAction: "none" } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "not json" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.identification-recording"]);
    assert.deepStrictEqual(posted[0].payload, { status: "failed", reason: "Choose a valid Control Studio recording or result JSON file." });
  });

  test("stops the identification a refit started when the page aborts it, as the engine's cancel (B3.6)", async function () {
    this.timeout(30_000);
    let answered = 0;
    const { current, fake, scope } = await servedWith(() => {
      answered += 1;
      return answered === 1 ? { result: { status: "recording-needed", recordingId: "rec-5", recordingCatalog: RECORDING_CATALOG,
        reason: "Open an independent recording to check the plant model." } } : { result: { operationId: "operation-22" } };
    }, { "controlStudio/cancel": { result: {} } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.recording.refit", { recordingSelection: RECORDING_CATALOG.selection }), scope), true);
    assert.strictEqual(await current.tune(commandMessage(scope, "control-studio.flow.cancel", {}), scope), false, "no Tune runs to stop");
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.flow.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-22"], "the Abort stops the refit's identification");
  });

  test("sends a refit the route holds no recording for to the engine without a recordingId, and shows the engine's refusal (B3.6)", async function () {
    this.timeout(30_000);
    const refusal = { message: "The study keeps no opened recording to fit.", cause: "unclassified", nextAction: "none" };
    const { current, fake, scope } = await servedWith({ failure: refusal });
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.recording.refit", { recordingSelection: { inputs: ["P1.MV"], outputs: ["P1.PV"] } }), scope), true);
    const requests = identifyActionsOf(fake);
    assert.strictEqual(requests.length, 1, "the refit reaches the engine, which chooses the recording");
    assert.strictEqual(requests[0].action, "openRecording");
    assert.ok(!("recordingId" in requests[0]), "the route names no recording it does not hold");
    assert.deepStrictEqual({ inputs: requests[0].inputs, outputs: requests[0].outputs }, { inputs: ["P1.MV"], outputs: ["P1.PV"] }, "the selection still reaches the engine");
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.failure"]);
    assert.strictEqual(posted[0].method, "identify");
    assert.deepStrictEqual(posted[0].failure, refusal, "the engine's refusal is shown as any identify failure is");
    assertNoToken();
  });

  /** The page's Tune command: the flow starts at Tune with the page's method, family, pins and task (B4.1, B4.2). */
  function tuneMessage(scope: LoopMessageScope, payload: Record<string, unknown> = {}): Record<string, unknown> {
    return commandMessage(scope, "control-studio.flow.start", {
      from: "tune",
      family: "automatic",
      applicationMode: "auto",
      pinnedValues: null,
      tuningPreferences: { method: "automatic", values: {} },
      task: {},
      ...payload,
    });
  }

  test("answers a Tune before Identify with the engine's failure, and posts it to the page with its next action", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.tune(tuneMessage(scope), scope), true);
    assert.strictEqual(posted.length, 1);
    assert.strictEqual(posted[0].kind, "control-studio.engine.failure");
    assert.strictEqual(posted[0].method, "tune");
    const failure = posted[0].failure as { cause: string; nextAction: string; nextActionLabel?: string };
    assert.strictEqual(failure.cause, "model-or-data");
    assert.strictEqual(failure.nextAction, "identify");
    assert.strictEqual(failure.nextActionLabel, "Identify plant");
    assertNoToken();
  });

  test("refuses a manual preview with a setting that is not a number, and posts the engine's refusal to the page", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    const preview = commandMessage(scope, "control-studio.engineer.manual-preview", { settings: { "P1.Parameters.ProportionalGain": "two" } });
    assert.strictEqual(await current.preview(preview, scope), true);
    assert.strictEqual(posted.length, 1);
    assert.strictEqual(posted[0].kind, "control-studio.engine.failure");
    assert.strictEqual(posted[0].method, "preview");
    assert.match((posted[0].failure as { message: string }).message, /invalid arguments/);
    assertNoToken();
  });

  test("answers a manual preview of numbers without an Identify with the next action Identify", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    const preview = commandMessage(scope, "control-studio.engineer.manual-preview", { settings: { "P1.Parameters.ProportionalGain": 2 } });
    assert.strictEqual(await current.preview(preview, scope), true);
    assert.strictEqual(posted.length, 1);
    assert.strictEqual(posted[0].method, "preview");
    assert.strictEqual((posted[0].failure as { nextAction: string }).nextAction, "identify");
    assertNoToken();
  });

  test("keeps the Tune and the preview of a parked loop on the TypeScript route, out of the engine", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    assert.strictEqual(await current.tune(tuneMessage(scope), scope), false);
    const preview = commandMessage(scope, "control-studio.engineer.manual-preview", { settings: { "P1.Parameters.ProportionalGain": 2 } });
    assert.strictEqual(await current.preview(preview, scope), false);
    assertOnlyParkedTools();
  });

  test("leaves the flows of other stages, and an Abort with no Tune running, to the TypeScript route", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.tune(commandMessage(scope, "control-studio.flow.start", { from: "verify" }), scope), false);
    assert.strictEqual(await current.tune(commandMessage(scope, "control-studio.flow.cancel", {}), scope), false);
    assert.deepStrictEqual(posted, []);
  });

  /** The engine's answer to `controlStudio/open` for a served loop: the operations it runs and the journaled ones it reattaches. */
  function openedAnswer(running: string[], reattached: string[] = []): Record<string, unknown> {
    return {
      result: {
        kind: "opened",
        loop: { id: "tank" },
        page: "Identify",
        stages: [],
        reattached,
        running,
        inputs: {},
        restoreAvailable: false,
        keepAvailable: false,
        applyObservationResumable: false,
      },
    };
  }

  test("a reopened loop follows the operations the engine runs for it, and shows the latest update of one still running (A8.6)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer(["operation-7"], ["recording-9"]),
      "controlStudio/status": { result: { operationId: "operation-7", command: "identify", state: "running", progress: "Identifying", stage: null } },
    });
    const current = createEnginePanelRoute({
      engine: fake.engine,
      projectRoot: folder,
      storageFolder: "",
      post: (message) => {
        posted.push(message);
      },
    });
    route = current;
    assert.strictEqual(await current.open(loop, scope), true);
    const shown = posted.filter((message) => message.kind === "control-studio.engine.update");
    assert.strictEqual(shown.length, 1, "the running operation's latest update reaches the page");
    assert.strictEqual(shown[0].operationId, "operation-7");
    const before = posted.length;
    fake.notify({ operationId: "recording-9", command: "identify", state: "finished", result: {} });
    assert.strictEqual(posted.length, before, "a journaled recording is not an operation the page follows");
    fake.notify({ operationId: "operation-7", command: "identify", state: "cancelled" });
    assert.strictEqual(posted.length, before + 1, "the followed operation's updates reach the page");
  });

  /** The engine's answer to `controlStudio/status` for one operation of the loop, in the state the test names. */
  function statusOf(operationId: string, command: string, state: string): Record<string, unknown> {
    return { result: { operationId, command, state, progress: "Running", stage: null } };
  }

  /** The operations the route asked the engine to cancel, in order. */
  function cancelsOf(fake: ReturnType<typeof fakeEngine>): string[] {
    return fake.requests
      .filter((sent) => sent.method === "controlStudio/cancel")
      .map((sent) => (sent.params as { operationId: string }).operationId);
  }

  /**
   * Opens a loop whose engine reports one operation for it, as a reopened panel opens it (A8.6). The engine answers a Cancel with the
   * operation's state at that moment, which is running while it runs.
   */
  async function reopenWith(
    operationId: string,
    command: string,
    state: string,
    loop: { id: string; runtimeId: string; experiment?: { execution: string } }
  ): Promise<{ current: EnginePanelRoute; fake: ReturnType<typeof fakeEngine>; scope: LoopMessageScope }> {
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([operationId]),
      "controlStudio/status": statusOf(operationId, command, state),
      "controlStudio/cancel": statusOf(operationId, command, "running"),
    });
    const current = createEnginePanelRoute({
      engine: fake.engine,
      projectRoot: folder,
      storageFolder: "",
      post: (message) => {
        posted.push(message);
      },
    });
    route = current;
    assert.strictEqual(await current.open(loop, scope), true);
    return { current, fake, scope };
  }

  test("a reopened run is adopted by the automatic flow: its latest update reaches the page, and Abort stops it (A8.6)", async () => {
    const { current, fake, scope } = await reopenWith("operation-11", "run", "running", { id: "tank", runtimeId: "simulator" });
    assert.strictEqual(posted.filter((message) => message.kind === "control-studio.engine.update").length, 1, "the running run's latest update reaches the page");
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.flow.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-11"], "Abort stops the run the engine kept running");
  });

  test("a reopened Validate is adopted by the Validate route: Abort stops it, and its end leaves nothing to stop (A8.6)", async () => {
    const { current, fake, scope } = await reopenWith("operation-12", "validate", "running", { id: "tank", runtimeId: "simulator" });
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.flow.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-12"], "Abort stops the Validate the engine kept running");
    fake.notify({ operationId: "operation-12", command: "validate", state: "finished" });
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.flow.cancel", {}), scope), false, "a Validate that has ended is not the route's to stop");
    assert.deepStrictEqual(cancelsOf(fake), ["operation-12"], "no second cancel is sent");
  });

  test("a reopened Apply, Restore or Keep is adopted by the application route: the page's Abort stops it (A8.6)", async () => {
    for (const command of ["apply", "restore", "keep"]) {
      const { current, fake, scope } = await reopenWith(`operation-${command}`, command, "running", { id: "tank", runtimeId: "simulator" });
      assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.engineer.cancel", {}), scope), true);
      assert.deepStrictEqual(cancelsOf(fake), [`operation-${command}`], `Abort stops a reopened ${command}`);
      current.dispose();
      route = undefined;
    }
  });

  test("an operation the engine reports as ended is not adopted on reopening, so the page's Abort is left to the other routes (A8.6)", async () => {
    const { current, fake, scope } = await reopenWith("operation-15", "run", "finished", { id: "tank", runtimeId: "simulator" });
    assert.strictEqual(posted.filter((message) => message.kind === "control-studio.engine.update").length, 0, "an ended operation shows nothing on reopening");
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.flow.cancel", {}), scope), false);
    assert.deepStrictEqual(cancelsOf(fake), []);
  });

  test("on a project-scripted loop a reopened identification is the project test: the test section shows it running, and Abort stops it (C12, A8.6)", async () => {
    const { current, fake, scope } = await reopenWith("operation-13", "identify", "running", {
      id: "tank",
      runtimeId: "simulator",
      experiment: { execution: "project-scripted" },
    });
    const running = posted.filter((message) => message.kind === "control-studio.experiment.result" && message.phase === "running");
    assert.strictEqual(running.length, 1, "the test section shows the reopened test as running");
    assert.deepStrictEqual(running[0].payload, { experiment_id: "operation-13" });
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.experiment.abort", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-13"], "Abort stops the test's campaign");
    fake.notify({ operationId: "operation-13", command: "identify", state: "cancelled" });
    const ended = posted.filter((message) => message.kind === "control-studio.experiment.result" && message.phase === "engine-ended");
    assert.deepStrictEqual(ended.map((message) => message.payload), [{ status: "cancelled" }], "the campaign's end is the test's end");
  });

  test("on a loop without a project test a reopened identification is the Identify page's campaign, not the test's (C12, A8.6)", async () => {
    const { current, fake, scope } = await reopenWith("operation-16", "identify", "running", { id: "tank", runtimeId: "simulator" });
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.experiment.abort", {}), scope), true);
    assert.deepStrictEqual(
      posted.filter((message) => message.phase === "engine-ended").map((message) => message.payload),
      [{ status: "unknown", reason: "No test is running." }]
    );
    assert.deepStrictEqual(cancelsOf(fake), [], "the test's Abort sends nothing: the campaign is not the test's");
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-16"], "the Identify page's Cancel still stops it");
  });

  test("a Tune sends no response, so that the engine reads the one the study declares (B4.3)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-8" } },
    });
    const current = createEnginePanelRoute({
      engine: fake.engine,
      projectRoot: folder,
      storageFolder: "",
      post: (message) => {
        posted.push(message);
      },
    });
    route = current;
    assert.strictEqual(await current.open(loop, scope), true);
    const tune = commandMessage(scope, "control-studio.flow.start", {
      from: "tune",
      family: "automatic",
      applicationMode: "auto",
      pinnedValues: null,
      tuningPreferences: { method: "automatic", values: {} },
      task: { tuningObjective: "fast", automaticCriteria: { fields: [] } },
    });
    assert.strictEqual(await current.tune(tune, scope), true);
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.ok(request, "the Tune request reaches the engine");
    assert.ok(!("choice" in (request.params as Record<string, unknown>)), "the page sends no response of its own");
  });
  test("a Tune waits for the study inputs the page changed before it, so the engine records them first (§0.1a inputs)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    let release: () => void = () => undefined;
    const inputsAnswered = new Promise<void>((resolve) => { release = resolve; });
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/inputs": async () => {
        await inputsAnswered;
        return { result: { stages: [], invalidated: { validation: false } } };
      },
      "controlStudio/tune": { result: { operationId: "operation-9" } },
    });
    const current = createEnginePanelRoute({
      engine: fake.engine,
      projectRoot: folder,
      storageFolder: "",
      post: (message) => {
        posted.push(message);
      },
    });
    route = current;
    assert.strictEqual(await current.open(loop, scope), true);
    const settings = { "P1.Parameters.ProportionalGain": 0.5, "P1.Parameters.IntegralGain": 0.1 };
    const changed = current.handle(commandMessage(scope, "control-studio.engineer.invalidate", {
      tuningPreferences: { method: "manual", values: settings }, pinnedValues: null, family: "PI",
    }), scope);
    const tuned = current.tune(commandMessage(scope, "control-studio.flow.start", {
      from: "tune", family: "PI", applicationMode: "auto", pinnedValues: null, tuningPreferences: { method: "manual", values: settings }, task: {},
    }), scope);
    await new Promise((resolve) => setTimeout(resolve, 50));
    assert.deepStrictEqual(fake.requests.map((sent) => sent.method), ["controlStudio/open", "controlStudio/analyze", "controlStudio/inputs"],
      "the Tune waits while the change the page made before it is unanswered");
    release();
    assert.strictEqual(await changed, true);
    assert.strictEqual(await tuned, true);
    assert.deepStrictEqual(fake.requests.map((sent) => sent.method), ["controlStudio/open", "controlStudio/analyze", "controlStudio/inputs", "controlStudio/tune"],
      "the engine records the change, then starts the Tune");
  });
});
