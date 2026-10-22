import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { ControlStudioEngine, EngineAnswer } from "../../controlStudio/engine/engineClient";
import type { EngineLoop, EngineMethod, EngineRouteContext, EngineStageRoute } from "../../controlStudio/engine/engineRouteContext";
import { createValidateRoute } from "../../controlStudio/engine/routes/validateRoute";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { startEngine, type EngineProcess } from "../../controlStudio/engine/engineProcess";
import type { Failure } from "../../controlStudio/generated/Failure";
import type { OperationUpdate } from "../../controlStudio/generated/OperationUpdate";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { measuredPlot, validateReceiver } from "../../controlStudio/webview/engineValidate";

/**
 * The engine binary the test runner names (`ST_CONTROL_STUDIO_TEST_BIN`), else the one the build left in the cargo target folder
 * (CARGO_TARGET_DIR, else `target`).
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

/** The control course's PID tuning example, a runnable project the engine serves (specification 35, B5). */
function exampleFolder(): string {
  const repoRoot = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
  return path.join(repoRoot, "examples/control/06-pid-tuning-methods/30-model-based-pi-tuning");
}

/** A loop with a declared analysis: the engine refuses it (`controller-capability`), so the page shows the refusal. */
const PARKED_LOOP_FILE = path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json");

const SCOPE: LoopMessageScope = {
  projectId: "project-test",
  editorSessionId: "editor-session-test",
  loopId: "loop-test",
  loopRevision: "revision-test",
};

const LOOP: EngineLoop = { id: "loop-test", runtimeId: "runtime-test" };

/** A page message of the loop's scope, with the fields every page message carries. */
function pageMessage(kind: string, payload: Record<string, unknown>, scope: LoopMessageScope = SCOPE): Record<string, unknown> {
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

/** The page's flow start from a stage's button: `from` names the stage (Identify, Tune or Validate). */
function flowStart(from: string, scope: LoopMessageScope = SCOPE): Record<string, unknown> {
  return pageMessage("control-studio.flow.start", {
    from,
    family: "automatic",
    applicationMode: "auto",
    pinnedValues: null,
    tuningPreferences: { method: "automatic", values: {} },
    task: {},
  }, scope);
}

function refusal(message: string, nextAction: string, nextActionLabel: string): Failure {
  return { cause: "model-or-data", message, nextAction, nextActionLabel } as Failure;
}

interface FakeRoute {
  readonly route: EngineStageRoute;
  readonly requests: Array<{ method: string; request: unknown }>;
  readonly listeners: Map<string, (update: OperationUpdate) => void>;
  readonly posted: Array<Record<string, unknown>>;
  readonly followed: string[];
}

/**
 * The Validate route over a fake engine and a fake poster. The engine's answers are the ones the options give; every request and
 * every message to the page is recorded, so that the route's functions can be tested without a process.
 */
function fakeRoute(options: {
  served?: EngineLoop;
  connection?: { endpoint: string; token?: string };
  validate?: EngineAnswer<{ operationId: string }>;
  cancel?: EngineAnswer<OperationUpdate>;
}): FakeRoute {
  const requests: Array<{ method: string; request: unknown }> = [];
  const listeners = new Map<string, (update: OperationUpdate) => void>();
  const posted: Array<Record<string, unknown>> = [];
  const followed: string[] = [];
  const client = {
    validate: async (request: unknown) => {
      requests.push({ method: "validate", request });
      return options.validate ?? { result: { operationId: "op-1" } };
    },
    cancel: async (request: unknown) => {
      requests.push({ method: "cancel", request });
      return options.cancel ?? { result: { operationId: "op-1", state: "cancelled", command: "validate" } as OperationUpdate };
    },
    onUpdate: (operationId: string, listener: (update: OperationUpdate) => void) => {
      listeners.set(operationId, listener);
      return { dispose: () => void listeners.delete(operationId) };
    },
  } as unknown as ControlStudioEngine;
  const context: EngineRouteContext = {
    client,
    projectRoot: "/project",
    storageFolder: "",
    served: () => options.served,
    post: async (_scope, message) => {
      posted.push({ ...message });
    },
    postFailure: async (_scope, method: EngineMethod, failure) => {
      posted.push({ kind: "control-studio.engine.failure", method, failure });
    },
    ask: async <T>(_scope: LoopMessageScope, _method: EngineMethod, run: () => Promise<EngineAnswer<T>>) => run(),
    connection: async () => options.connection,
    follow: (operationId) => {
      followed.push(operationId);
    },
  };
  return { route: createValidateRoute(context), requests, listeners, posted, followed };
}

suite("Control Studio engine Validate route", () => {
  suite("the route's functions over a fake engine and a fake poster", () => {
    test("sends the page's Validate to controlStudio/validate with the runtime connection, and follows its operation", async () => {
      const fake = fakeRoute({ served: LOOP, connection: { endpoint: "tcp://127.0.0.1:9000", token: "control-token" } });
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), true);
      assert.deepStrictEqual(fake.requests, [{
        method: "validate",
        request: {
          connection: { endpoint: "tcp://127.0.0.1:9000", token: "control-token" },
          loopId: LOOP.id,
          projectRoot: "/project",
          runtimeId: LOOP.runtimeId,
          storageFolder: "",
        },
      }]);
      assert.deepStrictEqual(fake.followed, ["op-1"], "the operation's updates and its result follow to the page");
      assert.deepStrictEqual(fake.posted, [], "the page hears nothing until the engine answers its operation");
    });

    test("sends a Validate without a runtime with no connection, so that the engine refuses it", async () => {
      const refused = refusal("Connect a runtime, so the isolated simulations can run.", "connect-runtime", "Connect runtime…");
      const fake = fakeRoute({ served: LOOP, validate: { failure: refused } });
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), true);
      const request = fake.requests[0]?.request as Record<string, unknown>;
      assert.ok(!("connection" in request), "no connection is sent without a runtime");
      assert.deepStrictEqual(fake.posted, [{ kind: "control-studio.engine.failure", method: "validate", failure: refused }]);
    });

    test("posts the engine's refusal to the page with its next action and label", async () => {
      const refused = refusal("The study has no candidate for the current model.", "tune", "Tune controller");
      const fake = fakeRoute({ served: LOOP, validate: { failure: refused } });
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), true);
      assert.deepStrictEqual(fake.posted, [{ kind: "control-studio.engine.failure", method: "validate", failure: refused }]);
      assert.deepStrictEqual(fake.followed, [], "a refused Validate starts no operation to follow");
    });

    test("leaves the flows of the other stages to their own routes", async () => {
      const fake = fakeRoute({ served: LOOP });
      for (const from of ["find", "recording", "tune"]) {
        assert.strictEqual(await fake.route.handle(flowStart(from), SCOPE), false, `a flow from ${from} is not Validate`);
      }
      assert.strictEqual(await fake.route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), false, "no Validate runs to cancel");
      assert.deepStrictEqual(fake.requests, []);
      assert.deepStrictEqual(fake.posted, []);
    });

    test("keeps the Validate of a loop the engine does not serve out of the engine", async () => {
      const fake = fakeRoute({});
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), false);
      assert.deepStrictEqual(fake.requests, []);
      assert.deepStrictEqual(fake.posted, []);
    });

    test("cancels the Validate it runs, and no longer once the operation has finished", async () => {
      const fake = fakeRoute({ served: LOOP });
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), true);
      assert.strictEqual(await fake.route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), true);
      assert.deepStrictEqual(fake.requests[1], { method: "cancel", request: { operationId: "op-1" } });
      assert.deepStrictEqual(fake.posted, [], "a cancelled operation answers through its update");
      fake.listeners.get("op-1")?.({ operationId: "op-1", command: "validate", state: "finished" } as unknown as OperationUpdate);
      assert.strictEqual(await fake.route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), false, "the finished Validate is not cancelled");
      assert.strictEqual(fake.requests.length, 2);
    });

    test("posts the engine's refusal of a Cancel to the page", async () => {
      const refused = refusal("The operation is not running.", "none", "");
      const fake = fakeRoute({ served: LOOP, cancel: { failure: refused } });
      assert.strictEqual(await fake.route.handle(flowStart("verify"), SCOPE), true);
      assert.strictEqual(await fake.route.handle(pageMessage("control-studio.flow.cancel", {}), SCOPE), true);
      assert.deepStrictEqual(fake.posted, [{ kind: "control-studio.engine.failure", method: "validate", failure: refused }]);
    });
  });

  suite("the real engine", () => {
    let folder = "";
    let engine: EngineProcess | undefined;
    let route: EnginePanelRoute | undefined;
    let posted: Array<Record<string, unknown>> = [];

    setup(() => {
      folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-validate-"));
      posted = [];
    });

    teardown(() => {
      route?.dispose();
      engine?.dispose();
      route = undefined;
      engine = undefined;
      fs.rmSync(folder, { recursive: true, force: true });
    });

    /** Copies a project folder into the test's folder, and returns the loop's identity from its loop file. */
    function projectLoop(loopFile: string, projectFolder: string): { loop: EngineLoop; scope: LoopMessageScope } {
      fs.cpSync(projectFolder, folder, { recursive: true });
      const identity = JSON.parse(fs.readFileSync(loopFile, "utf8")) as { id: string; runtimeId: string; projectId: string; revision: string };
      const stored = JSON.parse(fs.readFileSync(path.join(folder, path.basename(loopFile)), "utf8")) as { id: string };
      assert.strictEqual(stored.id, identity.id, "the copied loop is the one the test names");
      return {
        loop: { id: identity.id, runtimeId: identity.runtimeId },
        scope: { projectId: identity.projectId, editorSessionId: SCOPE.editorSessionId, loopId: identity.id, loopRevision: identity.revision },
      };
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

    test("answers a Validate before the model is identified with the engine's failure, its next action and label", async function () {
      this.timeout(30_000);
      const loopFile = path.join(exampleFolder(), "control-loop.json");
      const { loop, scope } = projectLoop(loopFile, exampleFolder());
      const current = startRoute();
      assert.strictEqual(await current.open(loop, scope), true);
      posted = [];
      assert.strictEqual(await current.handle(flowStart("verify", scope), scope), true);
      assert.strictEqual(posted.length, 1, "the refusal is shown once");
      assert.strictEqual(posted[0].kind, "control-studio.engine.failure");
      assert.strictEqual(posted[0].method, "validate");
      const failure = posted[0].failure as { cause: string; nextAction: string; nextActionLabel?: string };
      assert.strictEqual(failure.cause, "model-or-data");
      assert.strictEqual(failure.nextAction, "identify");
      assert.strictEqual(failure.nextActionLabel, "Identify plant");
      assert.doesNotMatch(JSON.stringify(posted), /"(?:token|authToken|controlAuthToken)":/, "no control token reaches the page");
    });

    test("answers the Validate of a refused loop with the engine's refusal, out of the engine", async function () {
      this.timeout(30_000);
      const { loop, scope } = projectLoop(PARKED_LOOP_FILE, path.dirname(PARKED_LOOP_FILE));
      const current = startRoute();
      assert.strictEqual(await current.open(loop, scope), false);
      const refusal = posted.find((message) => message.kind === "control-studio.engine.failure");
      assert.ok(refusal, "the page is shown the engine's refusal of the open");
      posted = [];
      assert.strictEqual(await current.handle(flowStart("verify", scope), scope), true, "the refused loop's Validate is answered, not started");
      assert.deepStrictEqual(posted.map((message) => message.kind), ["control-studio.engine.failure"], "the page shows the refusal and nothing else");
      assert.deepStrictEqual(posted[0].failure, refusal?.failure, "the refusal is the engine's sentence and next action");
    });
  });

  suite("the page's Validate receiver on a DOM fixture", () => {
    // The rows the engine writes for the requirements of `engineAnswer` (ValidateRow, validate_answer.rs). The page writes these words
    // as they come; the engine's Rust tests (table_rows_tests.rs) pin how they are made.
    const MODEL_CHECK_ROW = { title: "Model agreement", symbol: "", source: "", measured: "4 %", limit: "≤ 15 %", verdict: "Pass", verdictText: "Pass", verdictClass: "pass", requirement: "model-check" };
    const CANDIDATE_SETTLING_ROW = { title: "Candidate · Maximum settling time · Measurement (m)", symbol: "Main.PV", source: "model model-4", measured: "20 s", limit: "≤ 15 s", verdict: "Fail", verdictText: "Fail · The response settles at 20 s, after the 15 s limit.", verdictClass: "fail", requirement: "response-deadline" };
    const CURRENT_SETTLING_ROW = { title: "Current settings · Maximum settling time · Measurement (m)", symbol: "Main.PV", source: "model model-4", measured: "4 s", limit: "≤ 15 s", verdict: "Pass", verdictText: "Pass", verdictClass: "pass", requirement: "response-deadline" };
    const OVERALL_FAIL_ROW = { title: "Candidate overall", symbol: "", source: "", measured: "—", limit: "—", verdict: "Fail", verdictText: "Fail", verdictClass: "fail", requirement: "overall" };
    const OVERALL_PASS_ROW = { ...OVERALL_FAIL_ROW, verdict: "Pass", verdictText: "Pass", verdictClass: "pass" };
    /** The page's requirement table, as the Validate page's template holds it. */
    function validatePage(): { root: HTMLElement; table: HTMLTableElement; view: Window } {
      const { JSDOM } = require("jsdom");
      const dom = new JSDOM(
        '<main><section><div class="studio-validation-scroll"><table class="studio-requirements" data-studio-validation-result hidden>'
        + "<thead><tr><th>Requirement</th><th>Measured</th><th>Limit</th><th>Result</th></tr></thead><tbody></tbody></table></div></section></main>"
      );
      const root = dom.window.document.querySelector("main") as unknown as HTMLElement;
      const table = root.querySelector("table") as unknown as HTMLTableElement;
      return { root, table, view: dom.window as unknown as Window };
    }

    /** One scenario whose candidate fails the settling requirement, as the engine answers it. */
    function engineAnswer(overrides: Record<string, unknown> = {}): Record<string, unknown> {
      const settling = (status: string, value: number, reason: string) => ({
        id: "response-deadline",
        label: "Maximum settling time",
        channel: { role: "Measurement", unit: "m", symbol: "Main.PV" },
        limitSide: "atMost",
        status,
        value,
        limit: 15,
        unit: "s",
        source: "model model-4",
        reason,
      });
      const answer: Record<string, unknown> = {
        labels: { currentSettings: "Current settings", candidate: "Candidate", candidateOverall: "Candidate overall", automatic: "Automatic" },
        modelAgreement: {
          id: "model-check",
          label: "Model agreement",
          channel: { role: "", unit: "", symbol: "" },
          limitSide: "atMost",
          status: "Pass",
          value: 4,
          limit: 15,
          unit: "%",
          source: "",
          reason: "The check measured its error against the limit.",
          automatic: false,
        },
        scenarios: [{
          id: "target-response",
          label: "Target response",
          plant: "model model-4",
          current: {
            runId: "run-current",
            projectRevision: "rev-1",
            evidence: "ST-executed",
            status: "Pass",
            requirements: [settling("Pass", 4, "The response settles in time.")],
          },
          candidate: {
            runId: "run-candidate",
            projectRevision: "rev-1",
            evidence: "ST-executed",
            status: "Fail",
            requirements: [settling("Fail", 20, "The response settles at 20 s, after the 15 s limit.")],
          },
        }],
        status: "Fail",
        applyAvailable: false,
        failure: { cause: "settings", message: "The response settles at 20 s, after the 15 s limit.", nextAction: "tune", nextActionLabel: "Tune controller" },
        stages: [],
        // The engine's table: the model check, the failing candidate run, the passing current run, then the overall row.
        table: [MODEL_CHECK_ROW, CANDIDATE_SETTLING_ROW, CURRENT_SETTLING_ROW, OVERALL_FAIL_ROW],
        ...overrides,
      };
      // The engine's evidence for the answer: the model and the candidate it validated, passed exactly when Apply is available (B9.4).
      return { ...answer, stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: answer.applyAvailable === true } };
    }

    function receiveOf(): NonNullable<typeof validateReceiver.result> {
      const receive = validateReceiver.result;
      assert.ok(receive, "the Validate receiver shows results");
      return receive;
    }

    test("shows the model check and each run's requirements by label, role, value, limit and source, then the overall result", () => {
      const { root, table } = validatePage();
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports = { select: () => undefined, receive: (stage: string, payload: Record<string, unknown>) => void received.push({ stage, payload }) };
      receiveOf()(root, engineAnswer(), ports);
      assert.strictEqual(table.hidden, false);
      const rows = Array.from(table.tBodies[0].rows);
      assert.deepStrictEqual(rows.map((row) => row.dataset.studioRequirement), ["model-check", "response-deadline", "response-deadline", "overall"]);
      assert.match(rows[0].textContent ?? "", /Model agreement4 %≤ 15 %/, "the model check's error is against its limit, in percent");
      // Failures come first (B9.9): the candidate's failed row, then the current settings' passing row.
      const candidate = rows[1].cells;
      assert.match(candidate[0].textContent ?? "", /^Candidate · Maximum settling time · Measurement \(m\)model model-4$/, "the source is shown under the title");
      assert.strictEqual(candidate[1].textContent, "20 s");
      assert.strictEqual(candidate[3].textContent, "Fail · The response settles at 20 s, after the 15 s limit.");
      assert.strictEqual(candidate[3].dataset.verdict, "fail");
      assert.strictEqual(rows[1].title, "Main.PV", "the symbol is the hover text, not the row's name");
      const current = rows[2].cells;
      assert.match(current[0].textContent ?? "", /^Current settings · Maximum settling time · Measurement \(m\)model model-4$/);
      assert.strictEqual(current[1].textContent, "4 s");
      assert.strictEqual(current[2].textContent, "≤ 15 s");
      assert.doesNotMatch(table.textContent ?? "", /Main\.PV/, "no measurement is named by its symbol");
      assert.strictEqual(rows[3].cells[3].textContent, "Fail");
      assert.deepStrictEqual(received, [{
        stage: "engineer",
        payload: {
          status: "verification-failed",
          stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: false },
          failure: {
            message: "The response settles at 20 s, after the 15 s limit.",
            nextAction: { kind: "tune", label: "Tune controller" },
          },
        },
      }], "the page's stage state holds the refusal with its next action, and Apply stays unavailable");
    });

    test("shows each scenario's rows under the engine's names, in the engine's order, when the campaign has several", () => {
      const { root, table } = validatePage();
      const scenarioRows = [
        MODEL_CHECK_ROW,
        { ...CANDIDATE_SETTLING_ROW, title: "Target response · Candidate · Maximum settling time · Measurement (m)" },
        { ...CANDIDATE_SETTLING_ROW, title: "Cold start · Candidate · Maximum settling time · Measurement (m)" },
        { ...CURRENT_SETTLING_ROW, title: "Target response · Current settings · Maximum settling time · Measurement (m)" },
        { ...CURRENT_SETTLING_ROW, title: "Cold start · Current settings · Maximum settling time · Measurement (m)" },
        OVERALL_FAIL_ROW,
      ];
      receiveOf()(root, engineAnswer({ table: scenarioRows }), { select: () => undefined, receive: () => undefined });
      const rows = Array.from(table.tBodies[0].rows);
      assert.strictEqual(rows.length, 6, "the model check, two runs in each of two scenarios, and the overall result");
      // Failures come first (B9.9), each group in the engine's order: both candidates fail, both current settings pass.
      assert.match(rows[1].cells[0].textContent ?? "", /^Target response · Candidate · /);
      assert.match(rows[2].cells[0].textContent ?? "", /^Cold start · Candidate · /);
      assert.match(rows[3].cells[0].textContent ?? "", /^Target response · Current settings · /);
      assert.match(rows[4].cells[0].textContent ?? "", /^Cold start · Current settings · /);
    });

    test("passes a validated candidate to the page's stage state, which makes Apply available", () => {
      const { root, table } = validatePage();
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports = { select: () => undefined, receive: (stage: string, payload: Record<string, unknown>) => void received.push({ stage, payload }) };
      const passed = engineAnswer({ status: "Pass", applyAvailable: true, failure: undefined, table: [
        MODEL_CHECK_ROW,
        CURRENT_SETTLING_ROW,
        { ...CANDIDATE_SETTLING_ROW, measured: "4 s", verdict: "Pass", verdictText: "Pass", verdictClass: "pass" },
        OVERALL_PASS_ROW,
      ] });
      receiveOf()(root, passed, ports);
      assert.strictEqual(table.tBodies[0].rows[2].cells[3].textContent, "Pass");
      assert.deepStrictEqual(received, [{
        stage: "engineer",
        payload: { status: "verified", stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: true } },
      }]);
    });

    test("shows the engine's reason for the plant that ran the cases above the table", () => {
      const { root, table } = validatePage();
      const ports = { select: () => undefined, receive: () => undefined };
      const reason = "The target-response run started outside the settling band, so every case ran on the model.";
      receiveOf()(root, engineAnswer({ plantReason: reason }), ports);
      const note = root.querySelector("[data-studio-plant-reason]");
      assert.strictEqual(note?.textContent, reason);
      assert.strictEqual(note?.nextElementSibling, table.parentElement, "the reason sits above the table's scroll region");
    });

    test("marks a limit that Automatic generated, and shows an operating range by its measured range and its declared bounds", () => {
      const { root, table } = validatePage();
      // The engine's rows for a generated limit (Automatic), an operating-range check by its measured range and declared bounds, and a
      // single declared value (ValidateRow, validate_answer.rs).
      const answer = engineAnswer({ table: [
        MODEL_CHECK_ROW,
        { ...CANDIDATE_SETTLING_ROW, title: "Candidate · Maximum settling time · Automatic · Measurement (m)" },
        CURRENT_SETTLING_ROW,
        { title: "Candidate · Operating range · Measurement (m)", symbol: "Main.PV", source: "model model-4", measured: "20–30 m", limit: "0–100 m", verdict: "Pass", verdictText: "Pass", verdictClass: "pass", requirement: "process-bounds" },
        { title: "Candidate · Parameter unchanged · Kp (V/m)", symbol: "Main.Kp", source: "model model-4", measured: "2.5 V/m", limit: "= 2.5 V/m", verdict: "Pass", verdictText: "Pass", verdictClass: "pass", requirement: "fixed-parameter:Main.Kp" },
        OVERALL_FAIL_ROW,
      ] });
      receiveOf()(root, answer, { select: () => undefined, receive: () => undefined });
      const rows = Array.from(table.tBodies[0].rows);
      // The candidate's failed row comes first (B9.9), then the current settings' passing row and the candidate's other rows.
      assert.match(rows[1].cells[0].textContent ?? "", /^Candidate · Maximum settling time · Automatic · Measurement \(m\)/, "the Automatic mark follows the name");
      assert.doesNotMatch(rows[2].cells[0].textContent ?? "", /Automatic/, "a declared limit carries no mark");
      assert.match(rows[3].cells[0].textContent ?? "", /^Candidate · Operating range · Measurement \(m\)/);
      assert.strictEqual(rows[3].cells[1].textContent, "20–30 m", "the measured range is the lowest and the highest good value");
      assert.strictEqual(rows[3].cells[2].textContent, "0–100 m", "the limit is the declared bounds");
      assert.strictEqual(rows[4].cells[1].textContent, "2.5 V/m");
      assert.strictEqual(rows[4].cells[2].textContent, "= 2.5 V/m", "bounds of one value show that value");
    });

    test("hands the engine's plot to the chart by its names, and withdraws it when the engine gives none", () => {
      const { root, view } = validatePage();
      const events: unknown[] = [];
      view.addEventListener("trust-control-studio-engineer", (event) => events.push((event as CustomEvent).detail));
      const plot = {
        title: "Measured response",
        role: "Measurement",
        unit: "m",
        tuned: { name: "Tuned", timeSeconds: [0, 1, 2], values: [0, 1.5, null] },
        current: { name: "Current", timeSeconds: [0, 1], values: [0, 0.5] },
      };
      receiveOf()(root, engineAnswer({ plot }), { select: () => undefined, receive: () => undefined });
      receiveOf()(root, engineAnswer(), { select: () => undefined, receive: () => undefined });
      assert.deepStrictEqual(events, [
        {
          validationPlot: {
            title: "Measured response",
            groups: [{
              name: "Measurement",
              unit: "m",
              timeStart: 0,
              series: [
                { name: "Tuned", points: [[0, 0], [1, 1.5], [2, null]] },
                { name: "Current", points: [[0, 0], [1, 0.5]], dashed: true },
              ],
            }],
          },
        },
        { validationPlot: null },
      ], "the candidate is solid, the current settings dashed, and a result without a plot withdraws the last one");
    });

    test("a reopened loop shows the last validation the study holds: its table and the failure Apply names, with no plot", () => {
      const { root, table, view } = validatePage();
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const events: unknown[] = [];
      view.addEventListener("trust-control-studio-engineer", (event) => events.push((event as CustomEvent).detail));
      const ports = { select: () => undefined, receive: (stage: string, payload: Record<string, unknown>) => void received.push({ stage, payload }) };
      const opened = validateReceiver.opened;
      assert.ok(opened, "the Validate receiver shows an opened loop's validation");
      opened(root, { kind: "opened", validation: engineAnswer() }, ports);
      assert.strictEqual(table.hidden, false, "the table of the last result is shown");
      assert.deepStrictEqual(Array.from(table.tBodies[0].rows).map((row) => row.dataset.studioRequirement), ["model-check", "response-deadline", "response-deadline", "overall"]);
      assert.deepStrictEqual(received.map((item) => item.payload.status), ["verification-failed"], "Apply is not offered, and the engine's reason is shown");
      assert.deepStrictEqual(events, [{ validationPlot: null }], "the study keeps no plot, so none is shown");
    });

    test("a reopened loop whose validation passed offers Apply as the engine answers it", () => {
      const { root, table } = validatePage();
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      const ports = { select: () => undefined, receive: (stage: string, payload: Record<string, unknown>) => void received.push({ stage, payload }) };
      validateReceiver.opened?.(root, { kind: "opened", validation: engineAnswer({ status: "Pass", applyAvailable: true, failure: undefined, scenarios: [], table: [MODEL_CHECK_ROW] }) }, ports);
      assert.deepStrictEqual(received, [{
        stage: "engineer",
        payload: { status: "verified", stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: true } },
      }]);
      assert.strictEqual(table.tBodies[0].rows.length, 1, "the model check is the only row of this result");
    });

    test("a loop that has no current validation shows no table and offers nothing", () => {
      const { root, table } = validatePage();
      const received: Array<{ stage: string; payload: Record<string, unknown> }> = [];
      validateReceiver.opened?.(root, { kind: "opened" }, { select: () => undefined, receive: (stage, payload) => void received.push({ stage, payload }) });
      assert.strictEqual(table.hidden, true);
      assert.deepStrictEqual(received, []);
    });

    test("a stage change that leaves the validation no longer current hides its rows, and a reset drops the plant reason too", () => {
      const { root, table, view } = validatePage();
      const ports = { select: () => undefined, receive: () => undefined };
      const reason = "The target-response run started outside the settling band, so every case ran on the model.";
      const CustomEventOf = (view as unknown as { CustomEvent: typeof CustomEvent }).CustomEvent;
      const stageChange = (detail: Record<string, unknown>) => view.dispatchEvent(new CustomEventOf("trust-control-studio-engineer", { detail }));
      receiveOf()(root, engineAnswer({ plantReason: reason }), ports);
      assert.strictEqual(table.hidden, false);
      // The engine's stage evidence says whether the validation is still current; the status alone decides nothing.
      stageChange({ status: "edited", reason: "Inputs changed. Tune and verify before applying.", stageEvidence: { validationHasResult: true } });
      assert.strictEqual(table.hidden, false, "a change that leaves the validation current keeps the table, whatever its status");
      stageChange({ status: "edited", reason: "Inputs changed. Tune and verify before applying.", stageEvidence: { validationHasResult: false } });
      assert.strictEqual(table.hidden, true, "a changed input hides the table");
      assert.strictEqual(table.tBodies[0].rows.length, 0, "and drops its rows");
      receiveOf()(root, engineAnswer({ plantReason: reason }), ports);
      stageChange({ status: "candidate", family: "PI", values: {}, stageEvidence: { validationHasResult: false } });
      assert.strictEqual(table.hidden, true, "a new candidate hides the table");
      assert.strictEqual(table.tBodies[0].rows.length, 0);
      receiveOf()(root, engineAnswer({ plantReason: reason }), ports);
      stageChange({ status: "reset", liveIdentification: true });
      assert.strictEqual(table.hidden, true, "a reset hides the table");
      assert.strictEqual(table.tBodies[0].rows.length, 0);
      assert.strictEqual(root.querySelector("[data-studio-plant-reason]"), null, "and drops the plant reason");
      receiveOf()(root, engineAnswer(), ports);
      assert.strictEqual(table.hidden, false, "a later result shows the table again");
      stageChange({ status: "running", reason: "Validating." });
      stageChange({ liveIdentification: true });
      assert.strictEqual(table.hidden, false, "a stage change that leaves the validation current keeps the table");
    });

    test("a plot with no current run has the candidate's series alone", () => {
      const measured = measuredPlot({
        title: "Measured response",
        role: "Measurement",
        unit: "m",
        tuned: { name: "Tuned", timeSeconds: [0], values: [null] },
      });
      assert.deepStrictEqual(measured.groups[0].series, [{ name: "Tuned", points: [[0, null]] }]);
    });
  });
});
