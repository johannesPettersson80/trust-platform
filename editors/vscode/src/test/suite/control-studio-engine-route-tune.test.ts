import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { LoopMessageScope } from "../../controlStudio/messages";
import { createEnginePanelRoute, type EnginePanelRoute } from "../../controlStudio/engine/enginePanelRoute";
import { tuneCandidatePayload } from "../../controlStudio/webview/engineTune";
import { commandMessage, fakeEngine, openedAnswer, tuneMessage } from "./control-studio-engine-route-support";

/**
 * The Tune of a served loop over a fake engine (specification 35, B4 and C2): what the route sends the engine for the page's Tune, and how it
 * shows the engine's answer. The other cases of the route are in `control-studio-engine-route.test.ts`.
 */
suite("Control Studio engine route: Tune", () => {
  let folder = "";
  let route: EnginePanelRoute | undefined;
  let posted: Array<Record<string, unknown>> = [];

  setup(() => {
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-control-studio-route-"));
    posted = [];
  });

  teardown(() => {
    route?.dispose();
    route = undefined;
    fs.rmSync(folder, { recursive: true, force: true });
  });

  /** A program's own law's candidate as the engine answers it (C2): its structure, its gains and its values by variable. */
  function projectCandidateOf(structure: string): Record<string, unknown> {
    return { applied: false, source: "SIMC", structure, gains: { kp: 2, ki: 0.5, kd: 0 }, values: { "P1.Kp": 2, "P1.Ki": 0.5 } };
  }

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
      applicationMode: "auto",
      pinnedValues: null,
      tuningPreferences: { method: "automatic" },
      task: { tuningObjective: "fast", automaticCriteria: { fields: [] } },
    });
    assert.strictEqual(await current.tune(tune, scope), true);
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.ok(request, "the Tune request reaches the engine");
    const params = request.params as Record<string, unknown>;
    assert.ok(!("choice" in params), "the page sends no response of its own");
    for (const absent of ["method", "structure", "settings", "pins"]) {
      assert.ok(!(absent in params), `Automatic, with no family chosen, names no ${absent}`);
    }
  });

  test("a manual Tune sends the page's settings and the family the engineer chose, as words the engine reads (B4.2, B4.4)", async () => {
    const loop = { id: "tank", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "tank", loopRevision: "revision-test" };
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": { result: { operationId: "operation-10" } },
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
    const settings = { "P1.Parameters.ProportionalGain": "0.5", "P1.Parameters.IntegralGain": "x" };
    assert.strictEqual(await current.tune(tuneMessage(scope, { family: "PI", tuningPreferences: { method: "manual", values: settings } }), scope), true);
    const params = fake.requests.find((sent) => sent.method === "controlStudio/tune")?.params as Record<string, unknown>;
    assert.strictEqual(params.method, "manual");
    assert.strictEqual(params.structure, "PI", "the family goes as the word the page sent");
    assert.deepStrictEqual(params.settings, settings, "the entries go as typed: the engine reads them and refuses what it cannot read");
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
        return { result: { stages: [], invalidated: { validation: false }, inputs: {}, changed: true } };
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

  test("a program's own law answers Tune with projectCandidate, and the pins the page keeps go to the engine without a family (C2, B4.2)", async function () {
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
    assert.deepStrictEqual((tunes[1].params as { pins?: unknown }).pins, { loopRevision: "revision-test", values: { "P1.Kp": 2 } },
      "the pins carry the page's values and no family: the engine gives them the family of the program's candidate (B4.2)");
  });

  test("a reopened program's law sends the pins the page keeps without a family, which the engine names from the stored candidate (C2, A8.6)", async function () {
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
    assert.deepStrictEqual((request?.params as { pins?: unknown } | undefined)?.pins, { loopRevision: "revision-test", values: { "P1.Kp": 2 } },
      "the pins carry the page's values; the engine keys them by the stored candidate's family");
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

  test("a served loop claims a Tune whose family is not a PID structure: the engine receives the word and fixes none (C2, B4.4)", async function () {
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
    assert.strictEqual((request.params as Record<string, unknown>).structure, "PQ",
      "the family goes as the word the page shows; the engine decides that it names no PID structure");
  });

  test("a served loop sends pins with no family to the engine, and a method the engine does not run is the engine's refusal (C2, B4.2)", async function () {
    this.timeout(30_000);
    const loop = { id: "temperature", runtimeId: "simulator" };
    const scope: LoopMessageScope = { projectId: "project-test", editorSessionId: "editor-session-test", loopId: "temperature", loopRevision: "revision-test" };
    let refuse = false;
    const fake = fakeEngine({
      "controlStudio/open": openedAnswer([]),
      "controlStudio/tune": () => refuse
        ? { failure: { message: "The engine does not run the tuning method the page chose.", cause: "settings", nextAction: "none" } }
        : { result: { operationId: "operation-25" } },
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
      "the engine names the pins' family, so the route sends the pins as the page holds them");
    refuse = true;
    assert.strictEqual(await current.tune(tuneMessage(scope, { tuningPreferences: { method: "state-design" } }), scope), true,
      "a method the engine does not run is the engine's to refuse, and the Tune is still claimed");
    const sent = fake.requests.filter((request) => request.method === "controlStudio/tune");
    assert.strictEqual(sent.length, 2, "the route sends both Tunes: it names no method as unknown");
    assert.deepStrictEqual((sent[0].params as { pins?: unknown }).pins, { loopRevision: "revision-test", values: { "P1.Kp": 2 } });
    assert.strictEqual((sent[1].params as { method?: unknown }).method, "state-design", "the method goes as the page's word");
    assert.deepStrictEqual(posted.map((message) => [message.kind, message.method]),
      [["control-studio.engine.failure", "tune"]], "the engine's refusal is posted to the page as a Tune failure");
    assert.strictEqual((posted[0].failure as { message: string }).message, "The engine does not run the tuning method the page chose.");
  });

  test("a served state law's Tune is the engine's state-space method, and the engine, not the route, decides what its pins mean (C4, D-B9)", async function () {
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
      tuningPreferences: { method: "state-space", stateDesignDraft: { stateWeights: "[[400, 0], [0, 400]]" } },
      pinnedValues: { "P1.N": 0.1 },
    }), scope), true, "the engine serves the state law, so its Tune is the engine's");
    const request = fake.requests.find((sent) => sent.method === "controlStudio/tune");
    assert.ok(request, "the Tune reaches the engine");
    const params = request.params as Record<string, unknown>;
    assert.strictEqual(params.method, "state-space", "the method is the engine's name for the state-space design");
    // The engine ignores the pins of a state law (C4), so the route sends the pins the page holds, as it does for any method.
    assert.deepStrictEqual(params.pins, { loopRevision: "revision-test", values: { "P1.N": 0.1 } }, "the page's pins go to the engine");
    assert.strictEqual(params.structure, "LQR", "the state family goes as the word the page shows; the engine fixes no PID structure by it");
    assert.ok(!("settings" in params), "the state-space method takes no settings");
    assert.deepStrictEqual(posted.filter((message) => message.kind === "control-studio.engine.failure"), [],
      "the route refuses none of the state law's Tune, with the pins the page keeps");
  });
});
