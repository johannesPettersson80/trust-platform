import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";

/**
 * Specification 35 §0.1a and checklist V6 decision D9: a loop the engine parks (a feature the Rust engine does not run yet, until
 * its V7 port) stays on the TypeScript route. `open` answers false, and no page message of that loop reaches an engine stage.
 */

type LoopFile = Record<string, unknown>;

interface LoopIdentity {
  readonly id: string;
  readonly runtimeId: string;
  readonly projectId: string;
  readonly revision: string;
}

type Method = "navigation" | "identify" | "tune" | "preview" | "handle";

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
 * A loop whose controller is the program's own law of `family` (`controller.project`), as the generators of the feedforward, Smith
 * predictor, gain-schedule, cascade and PID-group loops write it. The PID law is replaced: a loop has one law.
 */
function ownLawLoop(family: string, change?: (loop: LoopFile, controller: LoopFile) => void): LoopFile {
  const loop = coreLoop();
  const controller = loop.controller as LoopFile;
  delete controller.law;
  controller.project = {
    family,
    convention: "project-owned-native-controller",
    initialization: { kind: "declared-state-v1", states: [] },
    axes: [],
  };
  change?.(loop, controller);
  return loop;
}

/** The loops the engine parks, with the feature each one uses: course lessons where one carries the feature, else the generated shape. */
const PARKED_LOOPS: ReadonlyArray<readonly [string, () => LoopFile]> = [
  ["a declared analysis", () => readLoop(course("07-classical-stability-and-frequency-design", "44-routh-hurwitz"))],
  ["state feedback", () => readLoop(course("11-state-space-estimation-and-predictive-control", "68-state-feedback"))],
  ["channels", () => readLoop(course("10-multivariable-control", "61-mimo-paths"))],
  ["the program's own law", () => readLoop(course("07-classical-stability-and-frequency-design", "89-mechanical-resonance"))],
  ["a feedforward law", () => readLoop(course("14-integrated-capstones", "84-independent"))],
  ["a Smith predictor", () => ownLawLoop("Smith")],
  ["a gain schedule", () => ownLawLoop("gain-scheduled")],
  ["a cascade", () => ownLawLoop("cascade", loop => { loop.channels = { inputs: [], outputs: [] }; })],
  ["PID groups", () => ownLawLoop("PID", (_loop, controller) => { controller.group = { members: [] }; })],
  ["an ON/OFF controller", () => {
    const loop = coreLoop();
    const controller = loop.controller as LoopFile;
    delete controller.law;
    controller.onOff = { mode: "on-off" };
    return loop;
  }],
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
      at("handle", "control-studio.engineer.confirm"),
      at("handle", "control-studio.engineer.rollback"),
      at("handle", "control-studio.engineer.review-runtime"),
      at("handle", "control-studio.engineer.retry-recovery"),
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

  /** Asks the route method of each message; every one must be left to the TypeScript route. */
  async function assertNoStageTakes(current: EnginePanelRoute, scope: LoopMessageScope): Promise<void> {
    for (const { method, message } of pageMessages(scope)) {
      const kind = String(message.kind);
      const taken = await current[method](message, scope);
      assert.strictEqual(taken, false, `${method} must leave ${kind} to the TypeScript route`);
    }
    assert.deepStrictEqual(posted, [], "no engine answer or failure reaches the page for a parked loop");
  }

  for (const [feature, load] of PARKED_LOOPS) {
    test(`keeps a loop with ${feature} on the TypeScript route, and no page message reaches an engine stage`, async function () {
      this.timeout(60_000);
      const loop = projectLoop(load());
      const scope = scopeOf(loop);
      const current = startRoute();
      assert.strictEqual(await current.open(loop, scope), false, "the engine refuses the loop as parked, so the TypeScript route keeps it");
      assert.strictEqual(await current.open(loop, scope), false, "the parked loop stays out of the way on a second open");
      assert.deepStrictEqual(posted, [], "a parked refusal is not a failure the page shows");
      await assertNoStageTakes(current, scope);
    });
  }

  test("takes the page's navigation for a loop the engine serves, so the parked answers above are not vacuous", async function () {
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
