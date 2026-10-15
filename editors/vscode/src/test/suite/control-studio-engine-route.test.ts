import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";
import { tuneCandidatePayload } from "../../controlStudio/webview/engineTune";

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

  test("refuses a served loop's Recorded-data fit check that names no kept recording at once, and sends nothing to the engine (B3.3)", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { form: MANUAL_FORM }), scope), true,
      "the engine serves the page's check, so the TypeScript route does not take it");
    const failures = posted.filter((message) => message.kind === "control-studio.engine.failure" && message.method === "identify");
    assert.strictEqual(failures.length, 1, "the page is told why the check cannot run");
    assert.strictEqual((failures[0].failure as { message: string }).message, "Open a recording before checking it.");
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
    // The Recorded-data fit's steps stay on the TypeScript route until V7.17 (B3.3).
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.edit", { form: {} }), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.fit", {}), scope), false);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.accept", {}), scope), false);
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.identify.import", { json: "{}" }), scope), false);
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

  /** A Recorded-data fit's answer as the engine gives it (`ManualIdentificationAnswer`, B3.3): by default a recording kept for its check. */
  function manualAnswerOf(changes: Record<string, unknown>): Record<string, unknown> {
    return { status: "complete", nextFitRole: "estimation", hasRecording: true, primaryAction: "checkRecording",
      primaryActionLabel: "Check recording", acceptanceReady: false, phase: "recording-selected", ...changes };
  }

  /** The form the page holds, as its text (`IdentificationFormDraft`, B3.3). */
  const MANUAL_FORM = { schemaVersion: 1, model: "first_order", heldStep: false, values: { measurementBound: "0.05" }, excludedStart: "", excludedEnd: "" };

  test("a served loop's Recorded-data fit takes each page command as the engine's manual action, and shows each answer (B3.3, B3.6)", async function () {
    this.timeout(30_000);
    const answers: Array<Record<string, unknown>> = [
      manualAnswerOf({ recordingId: "rec-7" }),
      manualAnswerOf({ recordingId: "rec-7", form: MANUAL_FORM }),
      manualAnswerOf({ recordingId: "rec-7", form: MANUAL_FORM, phase: "qualified", primaryAction: "fitPlant", primaryActionLabel: "Fit plant model" }),
      { operationId: "operation-fit" },
      manualAnswerOf({ recordingId: "rec-7", phase: "validation_required", primaryAction: "chooseStructure", primaryActionLabel: "Keep first-order model", structureChoice: "first_order" }),
      manualAnswerOf({ recordingId: "rec-7", phase: "accepted", primaryAction: "modelReady", primaryActionLabel: "Model ready", acceptanceReady: true }),
    ];
    // Each answer is the engine's `{ result }` envelope, as `controlStudio/identify` answers it (B3.3).
    const { current, fake, scope } = await servedWith(() => ({ result: answers.shift() }));
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.identify.import", { json: "{}" }), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.edit", { form: MANUAL_FORM }), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { form: MANUAL_FORM }), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.fit", {}), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.choose-structure", { choice: "first_order" }), scope), true);
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.accept", {}), scope), true);
    assert.deepStrictEqual(identifyActionsOf(fake), [
      { action: "manualImport", json: "{}" },
      { action: "manualEdit", form: MANUAL_FORM },
      { action: "manualQualify", recordingId: "rec-7", form: MANUAL_FORM },
      { action: "manualFit" },
      { action: "manualChooseStructure", choice: "first_order" },
      { action: "manualAccept" },
    ], "each page command is the engine's manual action, with the recording the engine named for the check");
    const shown = posted.filter((message) => message.kind === "control-studio.engine.result" && message.method === "identify");
    assert.strictEqual(shown.length, 5, "each manual answer reaches the page as the identify result; the operation's start is not an answer");
    assert.strictEqual((shown[4].result as { primaryAction: string }).primaryAction, "modelReady");
  });

  test("a served loop shows the engine's refusal of a file the page opened, and the command is not repeated (B3.6, B10.4)", async function () {
    this.timeout(30_000);
    const { current, fake, scope } = await servedWith({ failure: { message: "The file holds no complete recording.", cause: "modelOrData", nextAction: { kind: "none" } } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.identify.import", { json: "{}" }), scope), true);
    assert.strictEqual(identifyActionsOf(fake).length, 1, "the engine checks the file once");
    const failures = posted.filter((message) => message.kind === "control-studio.engine.failure" && message.method === "identify");
    assert.strictEqual(failures.length, 1);
    assert.strictEqual((failures[0].failure as { message: string }).message, "The file holds no complete recording.");
  });

  test("a reopened loop whose page shows the Recorded-data fit restores the kept fit once, and shows the operation's answer (A8.6)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: "revision-test" };
    const opened = openedAnswer([]).result as Record<string, unknown>;
    const fake = fakeEngine({
      "controlStudio/open": { result: { ...opened, pageView: { identifyMethod: "recorded-fit" } } },
      "controlStudio/identify": { result: { operationId: "operation-restore" } },
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
    posted = [];
    assert.strictEqual(await current.open(loop, scope), true);
    assert.deepStrictEqual(identifyActionsOf(fake), [{ action: "manualRestore" }], "the page's method is the Recorded-data fit, so the study's fit is restored");
    assert.strictEqual(await current.navigation(commandMessage(scope, "control-studio.navigation.select", { stage: "model", view: { acquisitionMethod: "recorded-fit" } }), scope), true);
    assert.strictEqual(identifyActionsOf(fake).length, 1, "the restoration runs once for the open loop");
    fake.notify({ operationId: "operation-restore", command: "identify", state: "finished", result: manualAnswerOf({ phase: "accepted", primaryAction: "modelReady", primaryActionLabel: "Model ready" }) });
    const shown = posted.filter((message) => message.kind === "control-studio.engine.result" && message.method === "identify");
    assert.strictEqual(shown.length, 1, "the restored fit's final update is the page's answer, not an identification");
    assert.strictEqual((shown[0].result as { primaryActionLabel: string }).primaryActionLabel, "Model ready");
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

  /**
   * The engine's answer to `controlStudio/open` for a served loop: the operations it runs, the journaled ones it reattaches, and the
   * study's tune section when the study holds a candidate.
   */
  function openedAnswer(running: string[], reattached: string[] = [], tune?: Record<string, unknown>): Record<string, unknown> {
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
        ...(tune ? { tune } : {}),
      },
    };
  }

  /** A program's own law's candidate as the engine answers it (C2): its structure, its gains and its values by variable. */
  function projectCandidateOf(structure: string): Record<string, unknown> {
    return { applied: false, source: "SIMC", structure, gains: { kp: 2, ki: 0.5, kd: 0 }, values: { "P1.Kp": 2, "P1.Ki": 0.5 } };
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

  test("a finished identification sends the kept recording's samples to the page as its recording evidence (B3.8, C12)", async () => {
    const { fake } = await reopenWith("operation-30", "identify", "running", { id: "tank", runtimeId: "simulator" });
    const samples = [{ elapsedSeconds: 0, scan: 1, quality: "good", sp: 2, pv: 4, requestedMv: null, appliedMv: 0, additional: [] }];
    fake.notify({ operationId: "operation-30", command: "identify", state: "finished", result: { recordingId: "rec-30", restoration: "restored", recordedSamples: samples } });
    const evidence = posted.filter((message) => message.kind === "control-studio.recording.evidence");
    assert.strictEqual(evidence.length, 1, "the kept recording reaches the page once");
    assert.deepStrictEqual(evidence[0].payload, { recording: { rawSamples: samples, evidence: { recordingId: "rec-30" } } },
      "the page draws the recorded signals the engine kept, and decides nothing about them");
    assert.strictEqual(posted.filter((message) => message.kind === "control-studio.engine.result" && message.method === "identify").length, 1);
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

  test("a program's own law answers Tune with projectCandidate, and the pins the page keeps then name its family (C2, B4.2)", async function () {
    this.timeout(30_000);
    const loop = { id: "temperature", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "temperature", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-21" } },
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
    assert.strictEqual(await current.tune(tuneMessage(scope, { family: "PI" }), scope), true);
    // The program's answer has no Control.PID candidate: the route keeps the family its projectCandidate names.
    fake.notify({ operationId: "operation-21", command: "tune", state: "finished", result: { projectCandidate: projectCandidateOf("PI") } });
    await new Promise((resolve) => setTimeout(resolve, 20));
    const shown = posted.find((message) => message.kind === "control-studio.engine.result" && message.method === "tune");
    assert.deepStrictEqual((shown?.result as { projectCandidate?: unknown } | undefined)?.projectCandidate, projectCandidateOf("PI"),
      "the page receives the program's candidate as the engine answered it");
    assert.strictEqual(await current.tune(tuneMessage(scope, { pinnedValues: { "P1.Kp": 2 } }), scope), true);
    const tunes = fake.requests.filter((sent) => sent.method === "controlStudio/tune");
    assert.strictEqual(tunes.length, 2, "both Tunes reach the engine");
    assert.deepStrictEqual((tunes[1].params as { pins?: unknown }).pins, { family: "PI", loopRevision: "revision-test", values: { "P1.Kp": 2 } },
      "the pins name the family of the program's candidate, because the page's choice is automatic");
  });

  test("a reopened program's law names its stored candidate's family for the pins the page keeps (C2, A8.6)", async function () {
    this.timeout(30_000);
    const loop = { id: "temperature", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "temperature", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([], [], { projectCandidate: projectCandidateOf("PI") }),
      "controlStudio/tune": { result: { operationId: "operation-22" } },
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
    assert.strictEqual(await current.tune(tuneMessage(scope, { pinnedValues: { "P1.Kp": 2 } }), scope), true);
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.deepStrictEqual((request?.params as { pins?: unknown } | undefined)?.pins, { family: "PI", loopRevision: "revision-test", values: { "P1.Kp": 2 } },
      "the stored candidate's family keys the pins of the reopened page's Tune");
  });

  test("the Tune page takes a program's candidate's family and its values by variable, and a Control.PID candidate's settings as before (C2, B4.3)", () => {
    const program = tuneCandidatePayload({ projectCandidate: projectCandidateOf("PI") });
    assert.deepStrictEqual([program.status, program.family, program.values], ["candidate", "PI", { "P1.Kp": 2, "P1.Ki": 0.5 }]);
    const pid = tuneCandidatePayload({ candidate: { structure: "PID" }, settings: { "P1.Kp": 3, "P1.Ki": 0.4, "P1.Kd": 0.1 } });
    assert.deepStrictEqual([pid.family, pid.values], ["PID", { "P1.Kp": 3, "P1.Ki": 0.4, "P1.Kd": 0.1 }]);
  });

  test("the Tune page takes the engine's feedforward answer under its design, as the answer gives it, and no design without one (C8)", () => {
    const feedforward = { outputPath: { gain: 0.2, timeConstantSeconds: 8, delaySeconds: 0 }, disturbancePath: { gain: -0.1, timeConstantSeconds: 3, delaySeconds: 0 },
      gain: 0.5, leadSeconds: 8, lagSeconds: 3, disturbanceStep: 10, feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
    const answered = tuneCandidatePayload({ projectCandidate: projectCandidateOf("PI"), feedforward });
    assert.deepStrictEqual((answered.design as { feedforward: unknown }).feedforward, feedforward, "the page's feedforward view reads the answer's own names");
    const plain = tuneCandidatePayload({ projectCandidate: projectCandidateOf("PI") });
    assert.strictEqual(plain.design, undefined, "an answer without a feedforward has no design for the page to read");
  });

  test("a served loop claims a Tune whose family the page does not name: the engine receives it with no structure (C2)", async function () {
    this.timeout(30_000);
    const loop = { id: "temperature", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "temperature", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-24" } },
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
    assert.strictEqual(await current.tune(tuneMessage(scope, { family: "PQ" }), scope), true,
      "the engine serves the loop, so its Tune is the engine's and never the TypeScript session's");
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.ok(request, "the Tune reaches the engine");
    assert.ok(!("structure" in (request.params as Record<string, unknown>)), "a family the page does not name is sent with no structure");
  });

  test("a served loop refuses, itself, a Tune it cannot send: pins with no family, or a method the engine does not run (C2)", async function () {
    this.timeout(30_000);
    const loop = { id: "temperature", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "temperature", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-25" } },
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
    posted = [];
    assert.strictEqual(await current.tune(tuneMessage(scope, { pinnedValues: { "P1.Kp": 2 } }), scope), true,
      "pins that name no family are refused by the route, and the Tune is still claimed");
    assert.strictEqual(await current.tune(tuneMessage(scope, { tuningPreferences: { method: "state-design", values: {} } }), scope), true,
      "a method the engine does not run is refused by the route, and the Tune is still claimed");
    assert.deepStrictEqual(fake.requests.filter((sent) => sent.method === "controlStudio/tune"), [], "neither refused Tune reaches the engine");
    assert.deepStrictEqual(posted.map((message) => [message.kind, message.method]),
      [["control-studio.engine.failure", "tune"], ["control-studio.engine.failure", "tune"]], "each refusal is posted to the page as a Tune failure");
  });

  test("a served state law's Tune is the engine's state-space method, and its pins are not structure pins (C4, D-B9)", async function () {
    this.timeout(30_000);
    const loop = { id: "control-state-feedback", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "control-state-feedback", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-state" } },
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
    posted = [];
    assert.strictEqual(await current.tune(tuneMessage(scope, {
      family: "LQR",
      tuningPreferences: { method: "state-space", values: {}, stateDesignDraft: { stateWeights: "[[400, 0], [0, 400]]" } },
      pinnedValues: { "P1.N": 0.1 },
    }), scope), true, "the engine serves the state law, so its Tune is the engine's");
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.ok(request, "the Tune reaches the engine");
    const params = request.params as Record<string, unknown>;
    assert.strictEqual(params.method, "state-space", "the method is the engine's name for the state-space design");
    assert.ok(!("pins" in params), "a state law's pins belong to its design, so the request carries no structure pins");
    assert.ok(!("structure" in params), "the state family is not a PID structure");
    assert.deepStrictEqual(posted.filter((message) => message.kind === "control-studio.engine.failure"), [],
      "the route refuses none of the state law's Tune, with the pins the page keeps");
  });
});
