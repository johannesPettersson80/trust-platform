import * as assert from "assert";

type RuntimeRequest = (operation: string, params: Record<string, unknown>) => Promise<unknown>;
type VerificationClient = {
  verify: (request: Record<string, unknown>, requirements: readonly Record<string, unknown>[]) => Promise<{
    projectRevision: string; run: { evidence: string }; evaluation: { status: string };
  }>;
  cancel: () => Promise<void>;
  dispose: () => void;
};

function clientModule(): { RuntimeVerificationClient?: new (connection: { source: "simulation"; request: RuntimeRequest }) => VerificationClient } {
  try { return require("../../controlStudio/verificationClient") as ReturnType<typeof clientModule>; } catch { return {}; }
}

const request = {
  step_ms: 1000, duration_ms: 2000, seed: 7, max_total_cycles: 8, wall_budget_ms: 500,
  initial_values: { PV: 0 }, events: [{ at_ms: 1000, symbol: "PV", value: 1 }],
  observations: [{ symbol: "PV", unit: "degC" }],
  parameter_overrides: [{ symbol: "Gain", iec_type: "REAL", unit: "1", convention: "parallel",
    minimum: 0, maximum: 2, value: 1 }],
};
const requirements = [{ id: "pv-bounds", kind: "bounds", channel: "PV", window: { start: 0, end: 2 }, minimum: -1, maximum: 2 }];
const run = {
  id: "run-1", evidence: "ST-executed", parameters: { Gain: 1 },
  conditions: { initialState: { PV: 0 }, events: [{ atSeconds: 1, channel: "PV", value: 1 }], durationSeconds: 2,
    sampleTimesSeconds: [0, 1, 2], seed: 7, modelRevision: "model-1", programRevision: "source-digest-1", parameterSchemaRevision: "schema-1" },
  channels: [{ id: "PV", unit: "degC" }],
  samples: [0, 1, 2].map((timeSeconds, sequence) => ({ timeSeconds, sequence, values: { PV: { value: sequence, quality: "good" } } })),
};

suite("Control Studio runtime verification client", () => {
  test("explains a failed target using the actual observed error and its limit", () => {
    const failure = require("../../controlStudio/mainWorkflowFailure");
    assert.strictEqual(failure.verificationFailureReason?.([{ id: "target-error", kind: "error", status: "Fail",
      value: 0.8935125977748761, limit: 0.1, unit: "°C", reason: "Observed measurement exceeds the declared limit." }]),
    "Recorded target error 0.8935 °C exceeds 0.1 °C.");
  });
  test("retains a failed terminal runtime record without accepting it as verified", async () => {
    const module = clientModule();
    let detailed = false;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation === "control.simulation.run.status" && Object.keys(params).length === 0) return {
        ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
      if (operation === "control.simulation.run.start") return job(params);
      if (params.include_results === true) detailed = true;
      return { ...job(), status: "failed", reason: "execution budget exhausted",
        source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers",
        ...(params.include_results === true ? { run: { ...run, samples: run.samples.slice(0, 2) } } : {}) };
    };
    const client = new module.RuntimeVerificationClient!({ source: "simulation", request: runtime });
    await assert.rejects(client.verify(request, requirements), /budget exhausted/);
    assert.strictEqual(detailed, true, "Failed terminal jobs must request their retained evidence");
    const snapshot = (client as unknown as { snapshot?: () => { status: string; jobId?: string; run?: { id: string }; reason?: string } }).snapshot?.();
    assert.strictEqual(snapshot?.status, "failed");
    assert.strictEqual(snapshot?.run?.id, `${snapshot?.jobId}:run-1`);
    assert.match(snapshot?.reason ?? "", /budget exhausted/);
  });

  test("runs a frozen actual-ST simulation and evaluates its matching recorded run", async () => {
    const module = clientModule();
    assert.strictEqual(typeof module.RuntimeVerificationClient, "function");
    const calls: { operation: string; params: Record<string, unknown> }[] = [];
    const runtime: RuntimeRequest = async (operation, params) => {
      calls.push({ operation, params });
      if (operation === "control.simulation.run.status" && Object.keys(params).length === 0) {
        return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
      }
      if (operation === "control.simulation.run.start") return job(params);
      if (operation === "control.simulation.run.status" && params.include_results === false) return { ...job(), status: "completed" };
      return { ...job(), status: "completed", source_basis: "loaded-runtime-sources",
        io_provenance: "isolated-simulation-no-physical-drivers", run };
    };
    const result = await new module.RuntimeVerificationClient!({ source: "simulation", request: runtime }).verify(request, requirements);
    assert.strictEqual(result.projectRevision, "rev-1");
    assert.strictEqual(result.run.evidence, "ST-executed");
    assert.strictEqual(result.evaluation.status, "Pass");
    const start = calls.find(call => call.operation === "control.simulation.run.start")!.params;
    assert.strictEqual(start.expected_project_revision, "rev-1");
    assert.strictEqual(start.execution, "isolated-simulation");
    assert.ok(Object.isFrozen(start));
  });

  test("rejects an unmatched runtime identity before returning an evaluation", async () => {
    const module = clientModule();
    assert.strictEqual(typeof module.RuntimeVerificationClient, "function");
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation === "control.simulation.run.status" && Object.keys(params).length === 0) return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
      if (operation === "control.simulation.run.start") return { job_id: "job-1", project_revision: "wrong-revision" };
      return {};
    };
    await assert.rejects(() => new module.RuntimeVerificationClient!({ source: "simulation", request: runtime }).verify(request, requirements), /identity|revision/i);
  });

  test("returns Inconclusive for missing required samples and cancels stale work", async () => {
    const module = clientModule();
    assert.strictEqual(typeof module.RuntimeVerificationClient, "function");
    let release: (() => void) | undefined; let cancelled = false; let polls = 0;
    const incomplete = { ...run, samples: run.samples.slice(0, 2) };
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation === "control.simulation.run.status" && Object.keys(params).length === 0) return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
      if (operation === "control.simulation.run.start") return job(params);
      if (operation === "control.simulation.run.cancel") { cancelled = true; return { ...job(), status: "running", cancel_requested: true }; }
      if (params.include_results === false) {
        polls += 1;
        if (polls === 1) return new Promise(resolve => { release = () => resolve({ ...job(), status: "running" }); });
        return terminal("cancelled", "verification cancelled");
      }
      return terminal("cancelled", "verification cancelled");
    };
    const client = new module.RuntimeVerificationClient!({ source: "simulation", request: runtime });
    const pending = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    assert.strictEqual(cancelled, true);
    release?.();
    await assert.rejects(() => pending, /cancelled|stale/i);
  });

  test("reports incomplete recorded evidence as Inconclusive", async () => {
    const module = clientModule();
    const incomplete = { ...run, samples: run.samples.slice(0, 2) };
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation === "control.simulation.run.status" && Object.keys(params).length === 0) return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
      if (operation === "control.simulation.run.start") return job(params);
      if (params.include_results === false) return { ...job(), status: "completed" };
      return { ...job(), status: "completed", source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers", run: incomplete };
    };
    const result = await new module.RuntimeVerificationClient!({ source: "simulation", request: runtime }).verify(request, requirements);
    assert.strictEqual(result.evaluation.status, "Inconclusive");
  });
  test("rejects a selected model or configuration mismatch in actual runtime evidence", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    const selected = { model_revision: "selected-model", model_provenance: "declared-provided-validated", simulation_toml: "[simulation]\nenabled = true\n" };
    const digest = `sha256:${require("crypto").createHash("sha256").update(selected.simulation_toml).digest("hex")}`;
    const exact = { ...run, conditions: { ...run.conditions, modelRevision: selected.model_revision,
      modelProvenance: selected.model_provenance, simulationConfigRevision: digest } };
    await new Client({ source: "simulation", request: completeProtocol(exact) }).verify({ ...request, simulation_override: selected }, requirements);
    for (const mismatch of [{ modelRevision: "wrong" }, { modelProvenance: "wrong" }, { simulationConfigRevision: "sha256:wrong" }]) {
      const changed = { ...exact, conditions: { ...exact.conditions, ...mismatch } };
      await assert.rejects(new Client({ source: "simulation", request: completeProtocol(changed) })
        .verify({ ...request, simulation_override: selected }, requirements), /model|configuration/i);
    }
  });

  test("cancels the returned job when cancellation arrives during start", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let entered!: () => void; const starting = new Promise<void>(resolve => { entered = resolve; });
    let startedId = "";
    let finish!: (value: unknown) => void;
    const reply = new Promise(resolve => { finish = resolve; });
    const cancelled: unknown[] = [];
    const complete = completeProtocol(run);
    const client = new Client({ source: "simulation", request: async (operation, params) => {
      if (operation.endsWith(".start")) { startedId = String(params.request_id); activeJobId = startedId; entered(); return reply; }
      if (operation.endsWith(".cancel")) { cancelled.push(params); return {}; }
      return complete(operation, params);
    } });
    const pending = client.verify(request, requirements);
    await starting;
    await client.cancel();
    finish({ job_id: startedId, project_revision: "rev-1" });
    await assert.rejects(pending, /cancelled|stale/i);
    assert.strictEqual(cancelled.length, 2, "start cancellation may be reconciled by the completion finally block");
    assert.ok(cancelled.every(value => JSON.stringify(value) === JSON.stringify({ job_id: startedId, expected_project_revision: "rev-1", include_results: false })));
  });

  test("rejects concurrent readiness and keeps a caller-owned snapshot without freezing it", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let finish!: () => void; const hold = new Promise<void>(resolve => { finish = resolve; });
    let readyCalls = 0;
    const input = structuredClone(request); const limits = structuredClone(requirements);
    const complete = completeProtocol(run);
    const client = new Client({ source: "simulation", request: async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) { readyCalls++; await hold; }
      return complete(operation, params);
    } });
    const pending = client.verify(input, limits).catch(error => error);
    const concurrent = client.verify(input, limits).catch(error => error);
    await new Promise(resolve => setImmediate(resolve));
    const count = readyCalls;
    assert.strictEqual(Object.isFrozen(input.initial_values), false);
    input.initial_values.PV = 99; limits[0].maximum = 0;
    finish();
    const results = await Promise.all([pending, concurrent]);
    assert.strictEqual(count, 1, "only one readiness/start sequence may be in progress");
    assert.strictEqual(results[0].evaluation.status, "Pass", "capture inputs and requirements before the first await");
    assert.match(String(results[1]), /already running/);
    client.dispose();
    await assert.rejects(client.verify(request, requirements), /disposed/);
  });

  test("compares map values independent of runtime key order and retains valid Fail reports", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    const input = { ...request, initial_values: { Z: 2, A: 1 } };
    const recorded = { ...run, conditions: { ...run.conditions, initialState: { A: 1, Z: 2 } } };
    const result = await new Client({ source: "simulation", request: completeProtocol(recorded) })
      .verify(input, [{ ...requirements[0], maximum: 1 }]);
    assert.strictEqual(result.evaluation.status, "Fail");
  });

  test("waits past a nonterminal cancellation acknowledgement and retains the cancelled terminal outcome", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let releasePoll!: () => void;
    let firstPoll = true;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) return readiness();
      if (operation.endsWith(".start")) return job(params);
      if (operation.endsWith(".cancel")) return { ...job(), status: "running", cancel_requested: true };
      if (params.include_results === true) return terminal("cancelled", "operator stopped the isolated run");
      if (firstPoll) {
        firstPoll = false;
        return new Promise(resolve => { releasePoll = () => resolve({ ...job(), status: "running" }); });
      }
      return terminal("cancelled", "operator stopped the isolated run");
    };
    const client = new Client({ source: "simulation", request: runtime });
    const verification = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    const snapshot = (client as any).snapshot();
    assert.strictEqual(snapshot?.status, "cancelled");
    assert.match(snapshot?.reason ?? "", /operator stopped/);
    releasePoll();
    await assert.rejects(verification, /cancelled|stale/i);
  });

  test("retains a late completed terminal run after cancellation without approving verification", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let releasePoll!: () => void;
    let firstPoll = true;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) return readiness();
      if (operation.endsWith(".start")) return job(params);
      if (operation.endsWith(".cancel")) return { ...job(), status: "running", cancel_requested: true };
      if (params.include_results === true) return { ...terminal("completed"), run };
      if (firstPoll) {
        firstPoll = false;
        return new Promise(resolve => { releasePoll = () => resolve({ ...job(), status: "running" }); });
      }
      return terminal("completed");
    };
    const client = new Client({ source: "simulation", request: runtime });
    const verification = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    const snapshot = (client as any).snapshot();
    assert.strictEqual(snapshot?.status, "completed");
    assert.strictEqual(snapshot?.run?.id, `${snapshot?.jobId}:run-1`);
    assert.match(snapshot?.reason ?? "", /completed.*cancellation|cancellation.*completed/i);
    releasePoll();
    await assert.rejects(verification, /cancelled|stale/i);
  });

  test("retains an unconfirmed cancellation and retries confirmation for the owned job", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let releasePoll!: () => void;
    let firstPoll = true;
    let confirmationAttempts = 0;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) return readiness();
      if (operation.endsWith(".start")) return job(params);
      if (operation.endsWith(".cancel")) return { ...job(), status: "running", cancel_requested: true };
      if (params.include_results === true) return terminal("cancelled", "operator stopped the isolated run");
      if (firstPoll) {
        firstPoll = false;
        return new Promise(resolve => { releasePoll = () => resolve({ ...job(), status: "running" }); });
      }
      confirmationAttempts += 1;
      if (confirmationAttempts === 1) throw new Error("control transport interrupted");
      return terminal("cancelled", "operator stopped the isolated run");
    };
    const client = new Client({ source: "simulation", request: runtime });
    const verification = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    assert.strictEqual((client as any).snapshot()?.status, "cancellation-unconfirmed");
    await client.cancel();
    assert.strictEqual((client as any).snapshot()?.status, "cancelled");
    releasePoll();
    await assert.rejects(verification, /cancelled|stale/i);
  });

  test("reissues cancellation when the original acknowledgement transport failed", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let releasePoll!: () => void;
    let firstPoll = true;
    let cancelCalls = 0;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) return readiness();
      if (operation.endsWith(".start")) return job(params);
      if (operation.endsWith(".cancel")) {
        cancelCalls += 1;
        if (cancelCalls === 1) throw new Error("cancel acknowledgement transport failed");
        return { ...job(), status: "running", cancel_requested: true };
      }
      if (params.include_results === true) return terminal("cancelled", "operator stopped the isolated run");
      if (firstPoll) {
        firstPoll = false;
        return new Promise(resolve => { releasePoll = () => resolve({ ...job(), status: "running" }); });
      }
      return terminal("cancelled", "operator stopped the isolated run");
    };
    const client = new Client({ source: "simulation", request: runtime });
    const verification = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    assert.strictEqual((client as any).snapshot()?.status, "cancellation-unconfirmed");
    await client.cancel();
    assert.strictEqual(cancelCalls, 2, "retry must resend the cancellation request when its acknowledgement was not delivered");
    assert.strictEqual((client as any).snapshot()?.status, "cancelled");
    releasePoll();
    await assert.rejects(verification, /cancelled|stale/i);
  });

  test("marks cancellation unconfirmed when its optional retained run is malformed", async () => {
    const Client = clientModule().RuntimeVerificationClient!;
    let releasePoll!: () => void;
    let firstPoll = true;
    const runtime: RuntimeRequest = async (operation, params) => {
      if (operation.endsWith(".status") && Object.keys(params).length === 0) return readiness();
      if (operation.endsWith(".start")) return job(params);
      if (operation.endsWith(".cancel")) return { ...job(), status: "running", cancel_requested: true };
      if (params.include_results === true) return { ...terminal("cancelled", "operator stopped the isolated run"), run: { evidence: "invented" } };
      if (firstPoll) {
        firstPoll = false;
        return new Promise(resolve => { releasePoll = () => resolve({ ...job(), status: "running" }); });
      }
      return terminal("cancelled", "operator stopped the isolated run");
    };
    const client = new Client({ source: "simulation", request: runtime });
    const verification = client.verify(request, requirements);
    await new Promise(resolve => setImmediate(resolve));
    await client.cancel();
    assert.strictEqual((client as any).snapshot()?.status, "cancellation-unconfirmed");
    releasePoll();
    await assert.rejects(verification, /cancelled|stale/i);
  });

});

function readiness() { return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" }; }
let activeJobId = "job-1";
function job(params?: Record<string, unknown>) {
  if (typeof params?.request_id === "string") activeJobId = params.request_id;
  return { job_id: activeJobId, project_revision: "rev-1" };
}
function terminal(status: "completed" | "cancelled", reason?: string) {
  return { ...job(), status, ...(reason === undefined ? {} : { reason }),
    source_basis: "loaded-runtime-sources", io_provenance: "isolated-simulation-no-physical-drivers" };
}

function completeProtocol(recorded: unknown): RuntimeRequest {
  return async (operation, params) => {
    if (operation.endsWith(".status") && Object.keys(params).length === 0)
      return { ready: true, project_revision: "rev-1", execution: "isolated-simulation", supports_request_id: true, source_basis: "loaded-runtime-sources" };
    if (operation.endsWith(".start")) return job(params);
    return { ...job(), status: "completed", source_basis: "loaded-runtime-sources",
      io_provenance: "isolated-simulation-no-physical-drivers", ...(params.include_results === true ? { run: recorded } : {}) };
  };
}
