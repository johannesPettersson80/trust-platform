import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";
import { cascadeLoop, gainScheduleLoop, onOffLoop, pidGroupLoop, smithLibraryLoop, smithLoop } from "./control-studio-structure-loop-fixtures";

/**
 * Specification 35 §0.1a: a loop the engine refuses (`controller-capability`) shows the engine's refusal and its next action on the
 * page. `open` answers false, the refusal is the page's answer to each stage message of that loop, and no engine stage takes one.
 * The page's navigation is its own view state, so the route leaves it alone.
 */

type LoopFile = Record<string, unknown>;

interface LoopIdentity {
  readonly id: string;
  readonly runtimeId: string;
  readonly projectId: string;
  readonly revision: string;
}

type Method = "navigation" | "identify" | "importFile" | "tune" | "preview" | "handle";

interface PageMessage {
  readonly method: Method;
  readonly message: Record<string, unknown>;
}

/** The engine binary the test runner names (`ST_CONTROL_STUDIO_TEST_BIN`), else the one the build left in the cargo target folder. */
function engineBinary(): string {
  const named = process.env.ST_CONTROL_STUDIO_TEST_BIN?.trim();
  if (named) {
    if (!fs.existsSync(named)) throw new Error(`ST_CONTROL_STUDIO_TEST_BIN not found at ${named}`);
    return named;
  }
  const repoRoot = repositoryRoot();
  const targetDir = process.env.CARGO_TARGET_DIR?.trim();
  const folder = targetDir ? path.resolve(repoRoot, targetDir) : path.join(repoRoot, "target");
  const name = process.platform === "win32" ? "trust-control-studio.exe" : "trust-control-studio";
  const binary = path.join(folder, "debug", name);
  if (!fs.existsSync(binary)) {
    throw new Error(`${name} is not built at ${binary}: run cargo build -p trust-control-studio first.`);
  }
  return binary;
}

function repositoryRoot(): string {
  return process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
}

/** The loop file of a course lesson under `examples/control`. */
function course(...parts: string[]): string {
  return path.join(repositoryRoot(), "examples/control", ...parts, "control-loop.json");
}

function readLoop(file: string): LoopFile {
  return JSON.parse(fs.readFileSync(file, "utf8")) as LoopFile;
}

/** The core example, which the engine runs: the loop the parked variants change. */
function coreLoop(): LoopFile {
  return readLoop(course("06-pid-tuning-methods", "30-model-based-pi-tuning"));
}

/**
 * The loops the engine refuses: the feature each one uses. A cascade written as two loop files (the top-level `cascade` key) is refused
 * (specification 35 C6, D-2); the one-loop cascade, the Smith predictor, the gain schedule, the ON/OFF controller and the PID group are
 * served (see SERVED_LOOPS).
 */
const REFUSED_LOOPS: ReadonlyArray<readonly [string, () => LoopFile]> = [
  ["a cascade written as two loop files", () => {
    const loop = coreLoop();
    loop.cascade = { role: "inner", partnerLoopId: "outer-loop" };
    return loop;
  }],
];

/**
 * The loops the engine serves now (specification 35 §0.1a, B1, C11 and C12): the schema-1 observation loops, read by the version-2 rules;
 * the declared analyses, with the parameter tool for lessons 44 and 45 that map their parameters; the measured-disturbance feedforward (C8,
 * lesson 84); the trajectory feedforward (lesson 90); the finite search of a program's own law (C5, lesson 89); and the measured MIMO paths
 * and the relative gain array of a two-input loop (C7, lessons 61 and 62); and the state-feedback laws (C4, lessons 68, 69 and 72). No lesson
 * uses the last five: the one-loop cascade (C6), the Smith predictor (C9), the gain schedule (C10), the ON/OFF controller (C3) and the PID
 * group with its decoupler (C7). Their loops are the fixtures of control-studio-structure-loop-fixtures.ts.
 */
const SERVED_LOOPS: ReadonlyArray<readonly [string, () => LoopFile]> = [
  ["state feedback (C4, lesson 68)", () => readLoop(course("11-state-space-estimation-and-predictive-control", "68-state-feedback"))],
  ["state feedback with a finite horizon (LQR, lesson 69)", () => readLoop(course("11-state-space-estimation-and-predictive-control", "69-lqr"))],
  ["state feedback with an estimator (LQG, lesson 72)", () => readLoop(course("11-state-space-estimation-and-predictive-control", "72-lqg"))],
  ["a schema-1 observation loop", () => readLoop(course("01-feedback-foundations-and-temperature-control", "01-cold-storage-loop-anatomy"))],
  ["a schema-1 ON/OFF lesson", () => readLoop(course("01-feedback-foundations-and-temperature-control", "02-cold-storage-on-off"))],
  ["a schema-1 hysteresis lesson", () => readLoop(course("01-feedback-foundations-and-temperature-control", "03-cold-storage-hysteresis-dwell"))],
  ["a schema-1 discrete-controller lesson", () => readLoop(course("14-integrated-capstones", "80-cold-storage"))],
  ["a declared analysis with a parameter tool", () => readLoop(course("07-classical-stability-and-frequency-design", "44-routh-hurwitz"))],
  ["a declared root locus with a parameter tool", () => readLoop(course("07-classical-stability-and-frequency-design", "45-root-locus"))],
  ["a declared analysis without a parameter tool", () => readLoop(course("07-classical-stability-and-frequency-design", "47-nyquist-stability"))],
  ["a declared sensitivity analysis without a parameter tool", () => readLoop(course("07-classical-stability-and-frequency-design", "48-sensitivity-functions"))],
  ["a measured-disturbance feedforward", () => readLoop(course("14-integrated-capstones", "84-independent"))],
  ["a trajectory feedforward", () => readLoop(course("09-industrial-control-structures", "90-trajectory-feedforward"))],
  ["the finite search of a program's own law", () => readLoop(course("07-classical-stability-and-frequency-design", "89-mechanical-resonance"))],
  ["channels for measured MIMO paths", () => readLoop(course("10-multivariable-control", "61-mimo-paths"))],
  ["channels in a relative-gain array", () => readLoop(course("10-multivariable-control", "62-relative-gain-array"))],
  ["a one-loop cascade (C6)", cascadeLoop],
  ["a Smith predictor (C9)", smithLoop],
  ["a Smith predictor with its library block (C9)", smithLibraryLoop],
  ["a gain schedule (C10)", gainScheduleLoop],
  ["an ON/OFF controller (C3)", onOffLoop],
  ["a PID group with its decoupler (C7)", pidGroupLoop],
];

suite("Control Studio engine parked loops", () => {
  let folder = "";
  let engine: EngineProcess | undefined;
  let route: EnginePanelRoute | undefined;
  let posted: Array<Record<string, unknown>> = [];

  setup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-parked-"));
    posted = [];
  });

  teardown(() => {
    route?.dispose();
    engine?.dispose();
    route = undefined;
    engine = undefined;
    fs.rmSync(folder, { recursive: true, force: true });
  });

  /** Writes the loop as the project's root loop, and returns its identity. */
  function projectLoop(loop: LoopFile): LoopIdentity {
    fs.writeFileSync(path.join(folder, "control-loop.json"), JSON.stringify(loop, null, 2));
    return {
      id: String(loop.id),
      runtimeId: String(loop.runtimeId),
      projectId: String(loop.projectId),
      revision: String(loop.revision),
    };
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

  /** A page message with the scope fields every page message carries. */
  function pageMessage(scope: LoopMessageScope, kind: string, payload: Record<string, unknown>): Record<string, unknown> {
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

  /** Every page message a stage route or the Tune and preview routes takes, each as the route method that takes it. */
  function pageMessages(scope: LoopMessageScope): PageMessage[] {
    const at = (method: Method, kind: string, payload: Record<string, unknown> = {}): PageMessage => ({
      method,
      message: pageMessage(scope, kind, payload),
    });
    return [
      at("navigation", "control-studio.navigation.select", { stage: "model", view: { acquisitionMethod: "automatic", experimentSignalIds: [] } }),
      at("navigation", "control-studio.navigation.select", { stage: "tune", view: { acquisitionMethod: "automatic", experimentSignalIds: [] } }),
      at("identify", "control-studio.identify.run", { model: "fopdt" }),
      at("identify", "control-studio.identify.fit"),
      at("identify", "control-studio.identify.accept"),
      at("identify", "control-studio.identify.cancel"),
      at("identify", "control-studio.identify.edit", { form: {} }),
      at("identify", "control-studio.identify.choose-structure", { choice: "first_order" }),
      at("identify", "control-studio.study.reset"),
      at("identify", "control-studio.recording.refit", { recordingSelection: { inputs: ["P1.MV"], outputs: ["P1.PV"] } }),
      at("importFile", "control-studio.identify.import", { json: "{}" }),
      at("importFile", "control-studio.recording.import", { json: "{}" }),
      at("importFile", "control-studio.general-model.import", { json: "{}" }),
      at("tune", "control-studio.flow.start", { from: "tune", family: "automatic", applicationMode: "auto", pinnedValues: null, tuningPreferences: { method: "automatic", values: {} }, task: {} }),
      at("tune", "control-studio.flow.cancel"),
      at("preview", "control-studio.engineer.manual-preview", { settings: { "P1.Parameters.ProportionalGain": 2 } }),
      at("handle", "control-studio.flow.start", { from: "find" }),
      at("handle", "control-studio.flow.start", { from: "recording" }),
      at("handle", "control-studio.flow.start", { from: "verify" }),
      at("handle", "control-studio.flow.cancel"),
      at("handle", "control-studio.engineer.invalidate", { acquisitionSettings: {} }),
      at("handle", "control-studio.application.apply"),
      at("handle", "control-studio.application.restore"),
      at("handle", "control-studio.application.keep"),
      at("handle", "control-studio.engineer.cancel"),
      at("handle", "control-studio.experiment.review", { kind: "observation", amplitude: 0, durationSeconds: 5, physicalTopologyReviewed: true }),
      at("handle", "control-studio.experiment.approve", { planId: "plan-test", physicalApproved: false }),
      at("handle", "control-studio.experiment.abort", { experimentId: "experiment-test" }),
      at("handle", "control-studio.parameters.inspect"),
      at("handle", "control-studio.parameters.review", { json: "{}" }),
      at("handle", "control-studio.parameters.apply", { reviewId: "review-test", physicalApproved: false }),
      at("handle", "control-studio.parameters.invalidate"),
    ];
  }

  /** The failures the page has been sent, which the engine's refusal of the open loop is one of. */
  function refusalsPosted(): Array<Record<string, unknown>> {
    return posted.filter((message) => message.kind === "control-studio.engine.failure");
  }

  /**
   * Every stage message of a refused loop is answered with the refusal the engine gave the open (`control-studio.engine.failure`, method
   * `open`), and nothing else is shown for it. The page's navigation is the page's own view state, so the route does not take it.
   */
  async function assertStagesAnswered(current: EnginePanelRoute, scope: LoopMessageScope, refusal: Record<string, unknown>): Promise<void> {
    for (const { method, message } of pageMessages(scope)) {
      const kind = String(message.kind);
      posted = [];
      const taken = await current[method](message, scope);
      if (method === "navigation") {
        assert.strictEqual(taken, false, `the route leaves the page's ${kind} to the page`);
        assert.deepStrictEqual(posted, [], `${kind} shows nothing`);
        continue;
      }
      assert.strictEqual(taken, true, `${method} answers ${kind} with the refusal`);
      assert.deepStrictEqual(posted.map((sent) => sent.kind), ["control-studio.engine.failure"], `${kind} shows the refusal and nothing else`);
      assert.strictEqual(posted[0]?.method, "open", `${kind} shows the refusal of the open`);
      assert.deepStrictEqual(posted[0]?.failure, refusal, `${kind} shows the engine's sentence and next action`);
    }
  }

  for (const [feature, load] of REFUSED_LOOPS) {
    test(`shows the engine's refusal of a loop with ${feature}, and answers each of its page's stage messages with it`, async function () {
      this.timeout(60_000);
      const loop = projectLoop(load());
      const scope = scopeOf(loop);
      const current = startRoute();
      assert.strictEqual(await current.open(loop, scope), false, "the engine refuses the loop, so the route does not serve it");
      const shown = refusalsPosted();
      assert.strictEqual(shown.length, 1, "the page is shown one refusal");
      assert.strictEqual(shown[0]?.method, "open", "the refusal answers the open");
      const refusal = shown[0]?.failure as Record<string, unknown>;
      assert.strictEqual(refusal.cause, "controller-capability", "the engine refuses the loop's controller, as the cause says");
      assert.ok(typeof refusal.message === "string" && refusal.message.length > 0, "the refusal has the engine's sentence");
      assert.ok(typeof refusal.nextAction === "string", "the refusal has the engine's next action");
      posted = [];
      assert.strictEqual(await current.open(loop, scope), false, "the refused loop is asked again on a second open");
      assert.strictEqual(refusalsPosted().length, 1, "the second open shows the same refusal once");
      await assertStagesAnswered(current, scope, refusal);
    });
  }

  for (const [feature, load] of SERVED_LOOPS) {
    test(`serves a loop with ${feature} on the engine, so its page's messages reach the engine route`, async function () {
      this.timeout(60_000);
      const loop = projectLoop(load());
      const scope = scopeOf(loop);
      const current = startRoute();
      assert.strictEqual(await current.open(loop, scope), true, "the engine serves the loop, so the route takes its page's messages");
    });
  }

  test("takes the page's navigation for a loop the engine serves, so the refusal answers above are not vacuous", async function () {
    this.timeout(60_000);
    const loop = projectLoop(coreLoop());
    const scope = scopeOf(loop);
    const current = startRoute();
    assert.strictEqual(await current.open(loop, scope), true, "the core loop is served");
    posted = [];
    assert.strictEqual(
      await current.navigation(pageMessage(scope, "control-studio.navigation.select", { stage: "tune", view: { acquisitionMethod: "automatic", experimentSignalIds: [] } }), scope),
      true,
      "a served loop takes its page's selection"
    );
  });
});
