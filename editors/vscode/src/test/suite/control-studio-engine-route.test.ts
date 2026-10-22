import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";
import { commandMessage, fakeEngine, openedAnswer, tuneMessage } from "./control-studio-engine-route-support";

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

/** The sentence the engine's parameter tool answers when an ended project test left its experiment settings released (`SETTINGS_RELEASED`, project_test.rs). */
const SETTINGS_RELEASED = "The previous project test restored its declared values. Edit and review them before the next trial.";

/** A loop with a declared analysis: the engine refuses it (`controller-capability`), so the page shows the refusal. */
const PARKED_LOOP_FILE = path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json");

interface LoopIdentity {
  readonly id: string;
  readonly runtimeId: string;
  readonly projectId: string;
  readonly revision: string;
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
   * A refused loop's page is shown the engine's refusal of the open, once, with its sentence and its next action; the analysis offers
   * no tool for it (B9.9), and nothing else is shown. Returns the refusal.
   */
  function assertRefusalShown(): Record<string, unknown> {
    const refusals = posted.filter((message) => message.kind === "control-studio.engine.failure");
    assert.strictEqual(refusals.length, 1, "the page is shown the engine's refusal once");
    assert.strictEqual(refusals[0].method, "open", "the refusal answers the open");
    const failure = refusals[0].failure as { cause: string; message: string; nextAction: string };
    assert.strictEqual(failure.cause, "controller-capability", "the engine refuses the loop's controller");
    assert.ok(failure.message.length > 0 && typeof failure.nextAction === "string", "the refusal has its sentence and its next action");
    assert.ok(posted.every((message) => message.kind === "control-studio.engine.failure" || message.kind === "control-studio.analysis.tools"),
      "the page hears the refusal and the analysis tools only");
    return failure as unknown as Record<string, unknown>;
  }

  /** The route takes one page message of a refused loop and answers it with the refusal, and nothing else is shown for it. */
  async function assertTakenWithRefusal(take: () => Promise<boolean>, refusal: Record<string, unknown>, what: string): Promise<void> {
    posted = [];
    assert.strictEqual(await take(), true, `${what} is answered with the refusal`);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.failure"], `${what} shows the refusal and nothing else`);
    assert.strictEqual(posted[0].method, "open", `${what} shows the refusal of the open`);
    assert.deepStrictEqual(posted[0].failure, refusal, `${what} shows the engine's sentence and next action`);
  }

  test("opens a loop the engine runs, and answers with its stages and page", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    assert.strictEqual(await startRoute().open(loop, scope), true);
    assert.strictEqual(posted.length, 4, "an open answers with the opened loop, then with the study's recorded inputs, its Recorded-data fit and the analysis tools");
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
    // The open answer carries the Recorded-data fit the study keeps: a new study keeps none, so it is the engine's start state (B3.3, A8.6).
    assert.strictEqual(posted[2].kind, "control-studio.engine.result");
    assert.strictEqual(posted[2].method, "identify");
    assert.strictEqual((posted[2].result as { primaryAction: string }).primaryAction, "openRecording", "the engine's start state, forwarded as it came");
    assert.strictEqual(posted[3].kind, "control-studio.analysis.tools");
    assert.ok(Array.isArray(posted[3].tools), "the analysis tools that apply reach the page, and no failure does");
    assertNoToken();
  });

  test("stores the page the engineer viewed, so that the next open names it", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    assert.strictEqual(await current.navigation(selectMessage(scope, "tune"), scope), true);
    assert.strictEqual(posted.length, 4, "the page is stored without a message back");
    assert.strictEqual(await current.open(loop, scope), true);
    assert.strictEqual(posted.length, 8, "the next open answers as the first did: the opened loop, the recorded inputs, the Recorded-data fit and the analysis tools");
    assert.strictEqual(posted[4].kind, "control-studio.engine.opened");
    assert.strictEqual((posted[4].result as { page: string }).page, "Tune");
    assertNoToken();
  });

  test("shows the engine's refusal of a loop it refuses, and leaves the page's own navigation to the page", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    assertRefusalShown();
    posted = [];
    assert.strictEqual(await current.navigation(selectMessage(scope, "tune"), scope), false, "the page's view state is not the engine's");
    assert.deepStrictEqual(posted, [], "the navigation is not answered");
    posted = [];
    assert.strictEqual(await current.open(loop, scope), false, "the refused loop is asked again on a second open");
    assertRefusalShown();
  });

  test("shows an engine that cannot start as its start failure, and serves no loop", async function () {
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

  test("refuses a served loop's Recorded-data fit check that names no recording, while the study keeps no opened one, with the engine's sentence (B3.3, B3.6)", async function () {
    this.timeout(30_000);
    const loop = projectLoop(coreLoopFile());
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), true);
    posted = [];
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.run", { form: MANUAL_FORM }), scope), true,
      "the engine serves the page's check, so no other route takes it");
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
    assert.strictEqual(posted.length, 2, "the reset's stages, then the Recorded-data fit of the new study that the reset answers with");
    assert.strictEqual(posted[0].kind, "control-studio.engine.result");
    assert.strictEqual(posted[0].method, "identify");
    const result = posted[0].result as { stages: Array<{ stage: string }> };
    assert.deepStrictEqual(
      result.stages.map((stage) => stage.stage),
      ["Monitor", "Test", "Identify", "Tune", "Validate", "Apply"]
    );
    assert.strictEqual(posted[1].method, "identify");
    assert.strictEqual((posted[1].result as { primaryAction: string }).primaryAction, "openRecording", "the start state of the new study's fit, from the engine's answer");
    assertNoToken();
  });

  test("answers the Identify commands of a refused loop with its refusal, out of the engine", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startIdentifyRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    const refusal = assertRefusalShown();
    await assertTakenWithRefusal(() => current.identify(commandMessage(scope, "control-studio.identify.run", { model: "fopdt" }), scope), refusal, "a check");
    await assertTakenWithRefusal(() => current.identify(commandMessage(scope, "control-studio.study.reset", {}), scope), refusal, "a reset");
    // The Recorded-data fit's steps are answered the same way (B3.3).
    await assertTakenWithRefusal(() => current.identify(commandMessage(scope, "control-studio.identify.edit", { form: {} }), scope), refusal, "an edit");
    await assertTakenWithRefusal(() => current.identify(commandMessage(scope, "control-studio.identify.fit", {}), scope), refusal, "a fit");
    await assertTakenWithRefusal(() => current.identify(commandMessage(scope, "control-studio.identify.accept", {}), scope), refusal, "an accept");
    await assertTakenWithRefusal(() => current.importFile(commandMessage(scope, "control-studio.identify.import", { json: "{}" }), scope), refusal, "a file");
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
    return { kind: "manualIdentification", status: "complete", nextFitRole: "estimation", hasRecording: true, primaryAction: "checkRecording",
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
      { kind: "operationStarted", operationId: "operation-fit" },
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
      { action: "manualQualify", form: MANUAL_FORM },
      { action: "manualFit" },
      { action: "manualChooseStructure", choice: "first_order" },
      { action: "manualAccept" },
    ], "each page command is the engine's manual action; the check names no recording, and the engine reads the study's last opened one (B3.6)");
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

  test("a reopened loop shows the Recorded-data fit that its open answer carries, whatever method the page shows, and asks the engine for no replay (A8.6, B3.3)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: "revision-test" };
    const opened = openedAnswer([]).result as Record<string, unknown>;
    const kept = manualAnswerOf({ phase: "accepted", primaryAction: "modelReady", primaryActionLabel: "Model ready" });
    for (const identifyMethod of ["recorded-fit", "automatic", undefined]) {
      posted = [];
      const fake = fakeEngine({
        "controlStudio/open": { result: { ...opened, manualIdentification: kept, ...(identifyMethod ? { pageView: { identifyMethod } } : {}) } },
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
      assert.deepStrictEqual(identifyActionsOf(fake), [], `the route asks for no replay (page method ${identifyMethod ?? "none"}): the open answer carries the state`);
      assert.strictEqual(await current.navigation(commandMessage(scope, "control-studio.navigation.select", { stage: "model", view: { acquisitionMethod: "recorded-fit" } }), scope), true);
      assert.deepStrictEqual(identifyActionsOf(fake), [], "choosing the Recorded-data fit later asks for no replay either");
      const shown = posted.filter((message) => message.kind === "control-studio.engine.result" && message.method === "identify");
      assert.strictEqual(shown.length, 1, "the state reaches the page once, as the fit's answers do");
      assert.deepStrictEqual(shown[0].result, kept, "as the engine gave it");
      current.dispose();
    }
  });

  test("a reopened loop shows the engine's answers of an operation that runs after the Recorded-data fit's state, which they replace (A8.6)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: "revision-test" };
    const opened = openedAnswer(["operation-fit"]).result as Record<string, unknown>;
    const stored = manualAnswerOf({ phase: "qualified", primaryAction: "fitPlant", primaryActionLabel: "Fit plant model" });
    const fake = fakeEngine({
      "controlStudio/open": { result: { ...opened, manualIdentification: stored } },
      "controlStudio/status": { result: { operationId: "operation-fit", command: "identify", state: "running", progress: "Fitting…" } },
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
    const order = posted.map((message) => (message.kind === "control-studio.engine.result" ? `result:${String(message.method)}` : String(message.kind)));
    assert.ok(order.indexOf("result:identify") < order.indexOf("control-studio.engine.update"),
      "the stored state comes first, so that the running operation's update is not undone by it");
    fake.notify({ operationId: "operation-fit", command: "identify", state: "finished", result: manualAnswerOf({ phase: "validation_required", primaryAction: "openValidationRecording", primaryActionLabel: "Open validation recording" }) });
    const answers = posted.filter((message) => message.kind === "control-studio.engine.result" && message.method === "identify");
    assert.strictEqual((answers[answers.length - 1].result as { primaryActionLabel: string }).primaryActionLabel, "Open validation recording");
  });

  test("answers a recording the engine keeps but cannot yet check as recording-needed, with its reason and channels (B3.6)", async function () {
    this.timeout(30_000);
    const { current, scope } = await servedWith({ result: { kind: "importedRecording", status: "recording-needed", recordingId: "rec-1", source: "Simulation", samples: 3,
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
    const identification = { kind: "identification", modelType: "First-order model with delay", equation: "G(s) = 2 / (5s + 1)", keyNumbers: "K = 2", provisional: false,
      plot: { timeSeconds: [0, 1], traces: [] }, stages: [], stageEvidence: {} };
    const { current, scope } = await servedWith({ result: { kind: "importedRecording", status: "checked", recordingId: "rec-2", recordingCatalog: RECORDING_CATALOG,
      identification, validation: "The model was checked against an independent recording." } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.result", "control-studio.identification-recording"]);
    assert.strictEqual(posted[0].method, "identify");
    assert.deepStrictEqual(posted[0].result, identification);
    assert.deepStrictEqual(posted[1].payload, { status: "checked", recordingId: "rec-2", recordingCatalog: RECORDING_CATALOG,
      validation: "The model was checked against an independent recording." });
    assertNoToken();
  });

  test("follows the fit of a checked recording that the engine names in its import's answer, and Cancel stops it (B3.6)", async function () {
    this.timeout(30_000);
    const { current, fake, scope } = await servedWith({ result: { kind: "importedRecording", status: "checked", recordingId: "rec-3", recordingCatalog: RECORDING_CATALOG,
      operationId: "operation-21" } }, { "controlStudio/cancel": { result: {} } });
    assert.strictEqual(await current.importFile(commandMessage(scope, "control-studio.recording.import", { json: "{}" }), scope), true);
    assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.identification-recording"]);
    assert.deepStrictEqual(identifyActionsOf(fake).map((request) => request.action), ["importRecording"],
      "the route makes no second call: the engine fitted the recording within the import's answer");
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-21"], "the page's Cancel stops the fit the import started");
  });

  test("answers a refit of the kept recording with the selection the page names, as an import answers (B3.6)", async function () {
    this.timeout(30_000);
    const identification = { kind: "identification", modelType: "First-order model with delay", equation: "G(s) = 2 / (5s + 1)", keyNumbers: "K = 2", provisional: false,
      plot: { timeSeconds: [0, 1], traces: [] }, stages: [], stageEvidence: {} };
    let answered = 0;
    const { current, fake, scope } = await servedWith(() => {
      answered += 1;
      return answered === 1 ? { result: { kind: "importedRecording", status: "recording-needed", recordingId: "rec-4", recordingCatalog: RECORDING_CATALOG,
        reason: "Open an independent recording to check the plant model." } }
        : { result: { kind: "importedRecording", status: "checked", recordingId: "rec-4", recordingCatalog: RECORDING_CATALOG, identification,
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
      return answered === 1 ? { result: { kind: "importedRecording", status: "recording-needed", recordingId: "rec-5", recordingCatalog: RECORDING_CATALOG,
        reason: "Open an independent recording to check the plant model." } } : { result: { kind: "operationStarted", operationId: "operation-22" } };
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
    assert.strictEqual((posted[0].failure as { message: string }).message, "The preview needs the controller settings, each by its symbol and as a number.");
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

  test("answers the Tune and the preview of a refused loop with its refusal, out of the engine", async function () {
    this.timeout(30_000);
    const loop = projectLoop(PARKED_LOOP_FILE);
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), false);
    const refusal = assertRefusalShown();
    await assertTakenWithRefusal(() => current.tune(tuneMessage(scope), scope), refusal, "a Tune");
    const preview = commandMessage(scope, "control-studio.engineer.manual-preview", { settings: { "P1.Parameters.ProportionalGain": 2 } });
    await assertTakenWithRefusal(() => current.preview(preview, scope), refusal, "a preview");
  });

  test("leaves the flows of other stages, and an Abort with no Tune running, of a served loop to the other routes", async function () {
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

  test("a reopened loop shows the analysis the Tune page showed last, as the open answer names it (B9.9, A8.6)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const answer = openedAnswer([]) as { result: Record<string, unknown> };
    const shown = { tool: "frequency", inputs: { grid: { minimumFrequency: "0.1", maximumFrequency: "10", points: "40" } } };
    answer.result.pageAnalysis = shown;
    const fake = fakeEngine({ "controlStudio/open": answer });
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
    const restore = posted.find((message) => message.kind === "control-studio.analysis.restore");
    assert.deepStrictEqual([restore?.tool, restore?.inputs], [shown.tool, shown.inputs], "the page gets the tool and the inputs the open answer names");
  });

  test("the page's navigation records the analysis the page names with the Tune page, as the page named it, and none with another page (B9.9)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const fake = fakeEngine({ "controlStudio/open": openedAnswer([]), "controlStudio/page": { result: {} } });
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
    // The route keeps no analysis of its own: what the engine records is the page's message, and the engine's own record of a calculation.
    const named = { tool: "frequency", inputs: { grid: { minimumFrequency: "0.1", maximumFrequency: "10", points: "40" } } };
    assert.strictEqual(await current.navigation(commandMessage(scope, "control-studio.navigation.select", { stage: "tune", analysis: named }), scope), true);
    assert.strictEqual(await current.navigation(commandMessage(scope, "control-studio.navigation.select", { stage: "verify", analysis: named }), scope), true);
    const records = fake.requests.filter(request => request.method === "controlStudio/page").map(request => request.params);
    const base = { loopId: "tank", projectRoot: folder, runtimeId: "simulator" };
    assert.deepStrictEqual(records, [{ ...base, page: "Tune", analysis: named }, { ...base, page: "Validate" }]);
  });

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
    loop: { id: string; runtimeId: string; projectTest?: boolean }
  ): Promise<{ current: EnginePanelRoute; fake: ReturnType<typeof fakeEngine>; scope: LoopMessageScope }> {
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: loop.id, loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([operationId], [], undefined, loop.projectTest === true),
      "controlStudio/status": statusOf(operationId, command, state),
      "controlStudio/cancel": statusOf(operationId, command, "running"),
      // The parameter tool's answer to the question whether the ended test left its settings released (`released`, C12).
      "controlStudio/parameters": { result: { settingsReleased: SETTINGS_RELEASED } },
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
      projectTest: true,
    });
    const running = posted.filter((message) => message.kind === "control-studio.experiment.result" && message.phase === "running");
    assert.strictEqual(running.length, 1, "the test section shows the reopened test as running");
    assert.deepStrictEqual(running[0].payload, { experiment_id: "operation-13" });
    assert.strictEqual(await current.handle(commandMessage(scope, "control-studio.experiment.abort", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-13"], "Abort stops the test's campaign");
    fake.notify({ operationId: "operation-13", command: "identify", state: "cancelled" });
    // An aborted test has no result to say whether it released the experiment settings, so the route asks the parameter tool (`released`)
    // and the test's end follows the answer (C12): it is awaited, not read at once.
    const ended = () => posted.filter((message) => message.kind === "control-studio.experiment.result" && message.phase === "engine-ended");
    for (let waited = 0; ended().length === 0 && waited < 2000; waited += 10) await new Promise((resolve) => setTimeout(resolve, 10));
    assert.deepStrictEqual(ended().map((message) => message.payload), [{ status: "cancelled", settingsReleased: SETTINGS_RELEASED }],
      "the campaign's end is the test's end, with the engine's word that the settings are released");
    assert.deepStrictEqual(
      fake.requests.filter((sent) => sent.method === "controlStudio/parameters").map((sent) => (sent.params as { action: string }).action),
      ["released"], "the route asks whether the aborted test left its settings released");
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
      [{ status: "unknown" }],
      "the route ends the page's busy state with no sentence of its own"
    );
    assert.deepStrictEqual(cancelsOf(fake), [], "the test's Abort sends nothing: the campaign is not the test's");
    assert.strictEqual(await current.identify(commandMessage(scope, "control-studio.identify.cancel", {}), scope), true);
    assert.deepStrictEqual(cancelsOf(fake), ["operation-16"], "the Identify page's Cancel still stops it");
  });
});
