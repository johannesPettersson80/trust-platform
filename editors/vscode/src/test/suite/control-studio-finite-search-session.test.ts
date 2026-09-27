import * as fs from "fs";
import * as path from "path";
import * as assert from "assert";

suite("Control Studio finite search session", () => {
  test("rejects malformed restore without discarding a reviewed search", async () => {
    const { FiniteSearchSession } = require("../../controlStudio/finiteSearchSession");
    const replies: any[] = [];
    const result = { job_id: "job", project_revision: "source", status: "completed", trial_count: 1,
      trials: [{ index: 0, parameters: { "Main.Kp": 2 }, status: "failed", reason: "fixture", score: null }],
      ranking: [], unattempted_trials: [], source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
    const connection = { source: "simulation", request: async (operation: string, params: any) => {
      if (operation === "control.search.start") return { job_id: "job", project_revision: "source" };
      if (params.job_id) return result;
      return { ready: true, project_revision: "source" };
    } };
    const session = new FiniteSearchSession(async () => connection);
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
    const message = (action: string, payload: object) => ({ schemaVersion: 1, kind: `control-studio.search.${action}`, messageId: "message",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload });
    const post = async (value: unknown) => { replies.push(value); };
    await session.handle(message("review", { json: '{"trial_count":1}' }), scope, post);
    const reviewId = replies.find(reply => reply.status === "reviewed")?.reviewId;
    assert.ok(reviewId);
    assert.throws(() => session.restore({ malformed: true } as any), /invalid/i);
    await session.handle(message("start", { reviewId }), scope, post);
    assert.strictEqual(replies[replies.length - 1]?.status, "complete", "invalid restore must preserve the reviewed search");
    session.dispose();
  });
  test("exposes selected values only for complete compatible bindings and the same source", async () => {
    const { FiniteSearchSession } = require("../../controlStudio/finiteSearchSession");
    let revision = "source";
    const session = new FiniteSearchSession(async () => ({ source: "simulation", request: async () => ({ ready: true, project_revision: revision }) }));
    const run = { id: "run", evidence: "ST-executed", parameters: { "Main.Kp": 2 },
      conditions: { initialState: {}, events: [], durationSeconds: 1, sampleTimesSeconds: [0,1], seed: 1,
        modelRevision: "model", programRevision: "program", parameterSchemaRevision: "parameters" }, channels: [{ id: "y", unit: "1" }],
      samples: [0,1].map(sequence => ({ sequence, timeSeconds: sequence, values: { y: { value: 1, quality: "good" } } })) };
    session.restore({ origin: "runtime-isolated-simulation", selectedIndex: 0,
      request: { execution: "isolated-simulation", expected_project_revision: "source", trial_count: 1,
        grid: [{ symbol: "Main.Kp", unit: "1", minimum: 0, maximum: 10, values: [2] }], initial_values: {} },
      result: { job_id: "job", project_revision: "source", status: "completed", trial_count: 1,
        trials: [{ index: 0, parameters: { "Main.Kp": 2 }, status: "qualified", score: 1, run }], ranking: [0], unattempted_trials: [],
        source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" } });
    const binding = { loop_id: "loop", task_name: "Task", mode: { symbol: "Main.Auto", manual_value: false },
      parameters: [{ symbol: "Main.Kp", iec_type: "LREAL", unit: "1", convention: "gain", minimum: 0, maximum: 10 }] };
    assert.deepStrictEqual(await session.selectedParameterValues?.(binding), { "Main.Kp": 2 });
    await assert.rejects(() => session.selectedParameterValues({ ...binding, parameters: [...binding.parameters, { ...binding.parameters[0], symbol: "Main.Ki" }] }), /complete|missing/i);
    revision = "changed";
    await assert.rejects(() => session.selectedParameterValues(binding), /source|changed/i);
  });

  test("prepares a general declared grid with fixed values and explicit isolated initial mode", () => {
    let api: any;
    try { api = require("../../controlStudio/finiteSearchDraft"); } catch { api = undefined; }
    const root = process.env.TRUST_REPOSITORY_ROOT ?? path.resolve(__dirname, "../../../../..");
    const loop = JSON.parse(fs.readFileSync(path.join(root, "examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"), "utf8"));
    const draft = api?.buildFiniteSearchDraft(loop, { "P1.Kp": 16, "P1.Kd": 7 });
    assert.deepStrictEqual(draft?.grid, [
      { symbol: "P1.Kp", unit: "N·m/rad", minimum: 0, maximum: 20, values: [12, 16, 20] },
      { symbol: "P1.Kd", unit: "N·m·s/rad", minimum: 7, maximum: 7, values: [7] },
    ], "only the declared selected parameter varies; fixed values remain explicit");
    assert.strictEqual(draft.trial_count, 3);
    assert.deepStrictEqual(draft.initial_values, { "P1.ExperimentEnabled": 1 });
    assert.deepStrictEqual(draft.objective, { reference_symbol: "P1.Setpoint", feedback_symbol: "P1.Angle", error_unit: "rad" });
    assert.strictEqual(draft.step_ms, 20);
    assert.strictEqual(draft.duration_ms, 8100);
  });

  test("an edit invalidates a review still waiting for source inspection", async () => {
    const { FiniteSearchSession } = require("../../controlStudio/finiteSearchSession");
    let resolve: ((value: unknown) => void) | undefined;
    const session = new FiniteSearchSession(async () => ({ source: "simulation", request: async () => new Promise(done => { resolve = done; }) }));
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
    const message = (action: string, payload: object) => ({ schemaVersion: 1, kind: `control-studio.search.${action}`, messageId: "message",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload });
    const replies: any[] = [];
    const post = async (value: unknown) => { replies.push(value); };
    const pending = session.handle(message("review", { json: '{"trial_count":1}' }), scope, post);
    while (!resolve) await new Promise(done => setTimeout(done, 1));
    await session.handle(message("invalidate", {}), scope, post);
    resolve({ ready: true, project_revision: "source" });
    await pending;
    assert.ok(!replies.some(reply => reply.status === "reviewed"), "stale source inspection cannot recreate an invalidated review");
  });

  test("closing a search cannot resurrect its result from a late completion", async () => {
    const { FiniteSearchSession } = require("../../controlStudio/finiteSearchSession");
    let resolve: ((value: unknown) => void) | undefined;
    let detailed = false;
    const result = { job_id: "job", project_revision: "source", status: "completed", trial_count: 1,
      trials: [{ index: 0, parameters: { "Main.Kp": 2 }, status: "failed", reason: "Explicit fixture failure", score: null }],
      ranking: [], unattempted_trials: [], source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
    const session = new FiniteSearchSession(async () => ({ source: "simulation", request: async (operation: string, params: any) => {
      if (operation === "control.search.cancel") return {};
      if (operation === "control.search.start") return { job_id: "job", project_revision: "source" };
      if (!params.job_id) return { ready: true, project_revision: "source" };
      if (params.include_results) { detailed = true; return result; }
      return new Promise(done => { resolve = done; });
    } }));
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
    const message = (action: string, payload: object) => ({ schemaVersion: 1, kind: `control-studio.search.${action}`, messageId: "message",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload });
    const replies: any[] = [];
    const post = async (value: unknown) => { replies.push(value); };
    await session.handle(message("review", { json: '{"trial_count":1}' }), scope, post);
    const pending = session.handle(message("start", { reviewId: replies[0].reviewId }), scope, post);
    while (!resolve) await new Promise(done => setTimeout(done, 1));
    session.dispose(); resolve(result); await pending;
    assert.strictEqual(session.snapshot(), undefined, "closed session must not retain stale results");
    assert.strictEqual(detailed, false, "closed session need not transfer full trial evidence");
  });

  test("reviews immutable inputs and fetches detailed trial evidence only after completion", async () => {
    let api: any;
    try { api = require("../../controlStudio/finiteSearchSession"); } catch { api = undefined; }
    const calls: { operation: string; params: any }[] = [];
    const request = async (operation: string, params: any) => {
      calls.push({ operation, params });
      if (operation === "control.search.start") return { job_id: "job", status: "queued", project_revision: "source", trial_count: 1 };
      if (!params.job_id) return { ready: true, project_revision: "source", source_basis: "loaded-runtime-sources" };
      return { job_id: "job", project_revision: "source", status: "completed", trial_count: 1,
        trials: [{ index: 0, parameters: { "Main.Kp": 2 }, status: "failed", reason: "Explicit fixture failure", score: null }],
        ranking: [], unattempted_trials: [], source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
    };
    const session = api ? new api.FiniteSearchSession(async () => ({ source: "simulation", request })) : undefined;
    const scope = { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "revision" };
    const message = (action: string, payload: object) => ({ schemaVersion: 1, kind: `control-studio.search.${action}`, messageId: "message",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload });
    const config = { grid: [{ symbol: "Main.Kp", unit: "1", minimum: 0, maximum: 10, values: [2] }], trial_count: 1,
      step_ms: 20, duration_ms: 200, seed: 7, max_total_cycles: 1000, wall_budget_ms: 5000, per_trial_wall_ms: 1000,
      initial_values: {}, events: [], observations: [{ symbol: "Main.SP", unit: "C" }, { symbol: "Main.PV", unit: "C" }],
      requirements: [{ id: "bounds", symbol: "Main.PV", minimum: -10, maximum: 10, start_ms: 0, end_ms: 200 }],
      objective: { reference_symbol: "Main.SP", feedback_symbol: "Main.PV", error_unit: "C" } };
    const replies: any[] = [];
    const post = async (value: unknown) => { replies.push(value); };
    await session?.handle(message("review", { json: JSON.stringify(config) }), scope, post);
    assert.strictEqual(replies[0]?.status, "reviewed", "prepare a retained isolated search before execution");
    assert.ok(!calls.some(call => call.operation === "control.search.start"));
    config.grid[0].values[0] = 9;
    await session.handle(message("start", { reviewId: replies[0].reviewId }), scope, post);
    const started = calls.find(call => call.operation === "control.search.start")!.params;
    assert.deepStrictEqual(started.grid[0].values, [2]);
    assert.strictEqual(started.execution, "isolated-simulation");
    assert.strictEqual(started.expected_project_revision, "source");
    assert.deepStrictEqual(calls.filter(call => call.params.job_id).map(call => call.params.include_results), [false, true]);
    assert.strictEqual(replies[replies.length - 1].status, "complete");
    assert.deepStrictEqual(session.snapshot().result.ranking, []);
    await session.handle(message("start", { reviewId: replies[0].reviewId }), scope, post);
    assert.strictEqual(calls.filter(call => call.operation === "control.search.start").length, 1);
  });
});
