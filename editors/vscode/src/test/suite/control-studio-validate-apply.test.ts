import * as assert from "assert";
import { ControllerApplicationClient } from "../../controlStudio/controllerApplication";
import { createWorkflowResult, decodeWorkflowResult, exportWorkflowJson, exportSamplesCsv } from "../../controlStudio/workflowResult";
import { candidateFromControlPid, type ControlPidParameters } from "../../controlStudio/controllerCandidate";
import * as workflowResultModule from "../../controlStudio/workflowResult";

suite("Control Studio validation, apply, rollback, and export", () => {
  test("exports the current completed recording instead of an older working draft", async () => {
    const exportResult = require("../../controlStudio/workflowResult").workflowResultForExport;
    const previous = { recording: { rawSamples: [] } };
    const current = { recording: { rawSamples: [{ elapsedSeconds: 0, appliedMv: 85, pv: 35 }] } };
    const result = await exportResult(previous, () => current, false);
    assert.deepStrictEqual(result, current, "A working view must snapshot its current recording at export time");
    assert.deepStrictEqual(await exportResult(previous, () => current, true), previous,
      "Explicitly opened historical results remain immutable");
  });

  test("retains project-owned Boolean activation and terminal restoration in saved results", () => {
    for (const status of ["completed", "aborted", "failed"] as const) {
      const context = workflowResultModule.workflowExperimentContext({
        source: "simulation", kind: "project_sequence", affectedSignal: "P1.ExperimentEnabled", address: "%MX112.0",
        from: false, to: true, minimum: 0, maximum: 1, restorationValue: false,
        durationMs: 60100, trajectory: [], stopConditions: ["Abort", "Signal force"],
      }, { status, stop_reason: status === "completed" ? "duration_complete" : "signal_forced", restoration: "restored", applied_value: false });
      assert.ok(context, "a project-owned experiment must retain its terminal evidence");
      assert.strictEqual(context.kind, "observation");
      assert.strictEqual(context.affectedSignal, "P1.ExperimentEnabled");
      assert.strictEqual(context.requestedValue, 1);
      assert.strictEqual(context.appliedValue, 0, "retain the actual Boolean applied value, including false");
      assert.strictEqual(context.durationMs, 60100);
      assert.strictEqual(context.status, status);
      assert.strictEqual(context.aborted, status === "aborted");
      assert.deepStrictEqual(context.restoration, { value: 0, status: "confirmed" });
      const saved = { ...completeSavedRun(`project-${status}`, 1), experiment: context };
      assert.deepStrictEqual(decodeWorkflowResult(JSON.parse(exportWorkflowJson(saved)))?.experiment, context);
    }
  });
  test("v4 exports auxiliary channels and rejects changing channel metadata", () => {
    const base = completeSavedRun("channels", 1);
    const additional = [{ id: "speed", channel: "shaft", role: "state", unit: "rad/s", value: 2, quality: "good" }] as const;
    const samples = [{ ...base.recording.rawSamples[0], additional },
      { ...base.recording.rawSamples[0], scan: 2, elapsedSeconds: 0.1, observedAtMs: 1100, additional }];
    assert.match(exportSamplesCsv(samples), /additional\.speed\.value/, "CSV must retain the measured auxiliary signal");
    const result = { ...base, recording: { rawSamples: samples, selectedIntervals: [] } };
    assert.ok(decodeWorkflowResult(result));
    assert.strictEqual(decodeWorkflowResult({ ...result, recording: { ...result.recording,
      rawSamples: [samples[0], { ...samples[1], additional: [{ ...additional[0], unit: "rpm" }] }],
    } }), undefined, "a loop revision cannot silently change a recorded channel's unit");
  });

  test("v4 retains open-loop observations without inventing a setpoint or its unit", () => {
    const base = completeSavedRun("open-recording", 1);
    const units = { sp: "", pv: "%", mv: "%" };
    const value = { ...base, loop: { ...base.loop, topology: "open-loop" }, units,
      recording: { rawSamples: base.recording.rawSamples.map(sample => ({ ...sample, sp: null, units })), selectedIntervals: [] } };
    assert.ok(decodeWorkflowResult(value), "an open-loop record has no SP unit");
    assert.strictEqual(decodeWorkflowResult({ ...value, recording: { ...value.recording,
      rawSamples: value.recording.rawSamples.map(sample => ({ ...sample, sp: 1 })),
    } }), undefined, "open-loop records cannot claim a setpoint");
  });

  test("runtime recordings use scan time when wall-clock milliseconds repeat", () => {
    const base = completeSavedRun("fast-recording", 1);
    const sample = base.recording.rawSamples[0];
    const value = { ...base, recording: {
      rawSamples: [{ ...sample, actualCycleSeconds: null },
        { ...sample, scan: 2, elapsedSeconds: 0.01, actualCycleSeconds: 0.01 }],
      selectedIntervals: [],
      acquisition: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 2,
        scanRange: { first: 1, last: 2 }, missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true, freshnessAvailable: false, reason: "Every runtime scan was captured." },
    } };
    assert.ok(decodeWorkflowResult(value), "runtime sequence and elapsed time establish ordering independently of coarse wall time");
    assert.strictEqual(decodeWorkflowResult({ ...value, schemaVersion: 3 }), undefined, "legacy timestamp validation is preserved");
  });

  test("writes result v4 while preserving readable v3 records without migration on read", () => {
    const created = completeSavedRun("schema-current", 1);
    assert.strictEqual(created.schemaVersion, 4, "new saves use the current result schema");
    const legacy = { ...created, schemaVersion: 3 };
    const original = JSON.stringify(legacy);
    assert.strictEqual(decodeWorkflowResult(legacy)?.schemaVersion, 3, "reading legacy data does not silently rewrite it");
    assert.strictEqual(JSON.stringify(legacy), original);
  });

  test("uses revision-guarded existing runtime requests and separate physical approval", async () => {
    const calls: Array<{ operation: string; params: unknown }> = [];
    let statusCount = 0;
    const client = new ControllerApplicationClient(async (operation, params) => {
      calls.push({ operation, params });
      if (operation.endsWith("apply")) return { previous_revision: "controller-1", pending_revision: "controller-2" };
      if (operation.endsWith("status")) {
        statusCount += 1;
        if (statusCount === 1) return { active_revision: "controller-1", pending_revision: "controller-2", confirmed: false, parameters: candidate };
        if (statusCount === 2) return { active_revision: "controller-2", confirmed: true, parameters: candidate };
        return { active_revision: "controller-1", confirmed: true, parameters: candidate };
      }
      return { pending_revision: "controller-1" };
    });
    const candidate = { proportionalGain: 1, integralGain: 0.25, derivativeGain: 0.1,
      proportionalSetpointWeight: 0.8, derivativeSetpointWeight: 0, bias: 2,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -20, rateMaximum: 20,
      trackingTime: 4, antiWindup: "Combined" };
    await assert.rejects(() => client.apply({ instancePath: "Main.Controller", source: "Physical", expectedRevision: "controller-1", candidate: candidate as never, physicalApproved: false }), /approval/i);
    const applied = await client.apply({ instancePath: "Main.Controller", source: "Simulation", expectedRevision: "controller-1", candidate: candidate as never, physicalApproved: false });
    await client.rollback({ instancePath: "Main.Controller", expectedActiveRevision: applied.activeRevision, previousRevision: applied.previousRevision });
    assert.deepStrictEqual(calls.map((call) => call.operation), [
      "control.pid.apply", "control.pid.status", "control.pid.status", "control.pid.rollback", "control.pid.status",
    ]);
    assert.deepStrictEqual(calls[0].params, {
      instance_path: "Main.Controller", source: "simulation", expected_revision: "controller-1",
      parameters: { ProportionalGain: 1, IntegralGain: 0.25, DerivativeGain: 0.1,
        ProportionalSetpointWeight: 0.8, DerivativeSetpointWeight: 0, Bias: 2,
        OutputMinimum: 0, OutputMaximum: 100, RateMinimum: -20, RateMaximum: 20,
        TrackingTime: 4, AntiWindup: "Combined" }, physical_approved: false,
    });
  });

  test("exports reproducible results without secrets and preserves raw samples", () => {
    const raw = completeSavedRun("run-export", 1).recording.rawSamples;
    assert.match(exportSamplesCsv(raw),
      /^elapsed_seconds,observed_at_ms,actual_cycle_seconds,sp,pv,requested_mv,applied_mv/);
    const exported = exportWorkflowJson({ source: "Simulation", rawSamples: raw, candidate: { kp: 1 }, authToken: "secret" });
    assert.match(exported, /"source": "Simulation"/);
    assert.doesNotMatch(exported, /secret|authToken/);
    assert.strictEqual(raw.length, 1);
  });

  test("retains requested and applied output separately without inventing a request", () => {
    const result = completeSavedRun("run-output-path", 1) as unknown as Record<string, unknown>;
    const recording = result.recording as { rawSamples: Array<Record<string, unknown>> };
    const sample = recording.rawSamples[0];
    delete sample.mv;
    sample.requestedMv = null;
    sample.appliedMv = 12.5;
    result.schemaVersion = 3;

    assert.deepStrictEqual(decodeWorkflowResult(result), result,
      "the saved result must preserve a missing request separately from observed applied output");
    assert.match(exportSamplesCsv(recording.rawSamples as never),
      /^elapsed_seconds,observed_at_ms,actual_cycle_seconds,sp,pv,requested_mv,applied_mv/);
  });

  test("round-trips one reproducible workflow result while retaining raw samples", () => {
    const parameters: ControlPidParameters = { proportionalGain: 0.8, integralGain: 0.2, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -20, rateMaximum: 20,
      trackingTime: 4, antiWindup: "BackCalculation" };
    const rawSamples = [{ elapsedSeconds: 0, observedAtMs: 1000, scan: 1, source: "Simulation" as const,
      loopRevision: "sha256:loop-1", sp: 1, pv: 0, requestedMv: null, appliedMv: 0,
      units: { sp: "%", pv: "%", mv: "%" }, quality: "good" as const,
      actualCycleSeconds: 0.1, freshnessMs: 0, controllerMode: "Auto" as const,
      outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, safetyState: "Unavailable",
      events: ["recording_started"] }];
    const result = createWorkflowResult({
      resultId: "run-1", resultRevision: 1, projectId: "project:tank",
      source: "Simulation", loop: { id: "loop:tank", revision: "sha256:loop-1" },
      controller: { instance: "Main.Controller", revision: "sha256:controller-1" },
      software: { extensionVersion: "0.24.64", numericalAdapters: { tuning: "trust-control-tuning-v1" } },
      units: { sp: "%", pv: "%", mv: "%" },
      recording: { rawSamples, selectedIntervals: [{ startSeconds: 0, endSeconds: 0 }] },
      model: { kind: "first_order", gain: 1, timeConstantSeconds: 2, stable: true, normalizedError: 0 },
      candidate: candidateFromControlPid(parameters, "SIMC"), method: "SIMC",
      evidence: { observation: "Observed", model: "Identified from simulation data" },
      conclusion: "Result retained for later comparison.",
    }, "2026-08-25T12:00:00.000Z");
    rawSamples[0].pv = 99;
    const decoded = decodeWorkflowResult(JSON.parse(exportWorkflowJson({ ...result, authToken: "secret" })));
    assert.deepStrictEqual(decoded, result);
    assert.strictEqual(decoded?.recording.rawSamples[0].pv, 0, "saving must not alias or rewrite the raw recording");
    assert.doesNotMatch(exportWorkflowJson(result), /authToken|secret/);
    assert.strictEqual(decodeWorkflowResult({ ...result, schemaVersion: 5 }), undefined);
  });

  test("round-trips a retained constrained proportional recommendation", () => {
    const base = completeSavedRun("run-constrained-p", 1);
    const candidate = candidateFromControlPid({
      proportionalGain: 0.8, integralGain: 0, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: -10, outputMaximum: 10, rateMinimum: -100, rateMaximum: 100,
      trackingTime: 1, antiWindup: "BackCalculation",
    }, "Constrained comparison");
    const result = { ...base, candidate, method: "Constrained comparison" };
    assert.deepStrictEqual(decodeWorkflowResult(JSON.parse(exportWorkflowJson(result))), result);
  });

  test("round-trips and validates the complete automatic tuning comparison", () => {
    const base = completeSavedRun("run-tuning-comparison", 1);
    const tuningComparison = {
      objective: "balanced",
      winnerMethod: "SIMC",
      closedLoopTimeSeconds: 1.5,
      attempts: [
        { id: "simc", method: "SIMC", structure: "PI", status: "Feasible", score: 4.25 },
        { id: "amigo", method: "AMIGO", structure: "PI", status: "Rejected", reason: "Failed: Noise response." },
      ],
    };
    const result = { ...base, tuningComparison };
    assert.deepStrictEqual(decodeWorkflowResult(JSON.parse(exportWorkflowJson(result))), result);
    assert.strictEqual(decodeWorkflowResult({ ...result,
      tuningComparison: { ...tuningComparison, attempts: [{ ...tuningComparison.attempts[0], status: "Winner" }] } }),
    undefined, "unknown comparison status must not reopen as qualified evidence");
    assert.strictEqual(decodeWorkflowResult({ ...result,
      tuningComparison: { ...tuningComparison, closedLoopTimeSeconds: -1 } }), undefined,
    "an invalid saved SIMC lambda must not reopen as qualified evidence");
  });

  test("appends immutable bounded saved-result revisions", () => {
    const append = (workflowResultModule as unknown as {
      appendWorkflowResultRevision?: (
        history: readonly unknown[],
        result: unknown,
        capacity: number
      ) => readonly unknown[];
    }).appendWorkflowResultRevision;
    assert.strictEqual(typeof append, "function", "saved-result revision history is not implemented");

    const first = { schemaVersion: 1, resultId: "run-1", resultRevision: 1,
      recording: { rawSamples: [{ elapsedSeconds: 0, pv: 0 }] } };
    const second = { schemaVersion: 1, resultId: "run-2", resultRevision: 2,
      recording: { rawSamples: [{ elapsedSeconds: 0, pv: 1 }] } };
    const third = { schemaVersion: 1, resultId: "run-3", resultRevision: 3,
      recording: { rawSamples: [{ elapsedSeconds: 0, pv: 2 }] } };
    const firstHistory = append?.([], first, 2) ?? [];
    const secondHistory = append?.(firstHistory, second, 2) ?? [];
    const bounded = append?.(secondHistory, third, 2) ?? [];

    (third.recording.rawSamples[0] as { pv: number }).pv = 99;
    assert.deepStrictEqual(bounded, [second, { ...third,
      recording: { rawSamples: [{ elapsedSeconds: 0, pv: 2 }] } }]);
    assert.deepStrictEqual(firstHistory, [first], "later saves must not rewrite earlier history");
  });

  test("rejects mixed evidence sources and non-monotonic saved samples", () => {
    const parameters: ControlPidParameters = { proportionalGain: 0.8, integralGain: 0.2, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0,
      outputMinimum: 0, outputMaximum: 100, rateMinimum: -20, rateMaximum: 20,
      trackingTime: 4, antiWindup: "BackCalculation" };
    const result = createWorkflowResult({
      resultId: "run-2", resultRevision: 1, projectId: "project:tank",
      source: "Simulation", loop: { id: "loop:tank", revision: "sha256:loop-1" },
      controller: { instance: "Main.Controller", revision: "sha256:controller-1" },
      software: { extensionVersion: "0.24.64", numericalAdapters: { tuning: "trust-control-tuning-v1" } },
      units: { sp: "%", pv: "%", mv: "%" },
      recording: { rawSamples: [
        { elapsedSeconds: 0, observedAtMs: 1000, actualCycleSeconds: 0.1, scan: 1, source: "Simulation",
          loopRevision: "sha256:loop-1", sp: 1, pv: 0, requestedMv: null, appliedMv: 0,
          units: { sp: "%", pv: "%", mv: "%" }, quality: "good", freshnessMs: 0,
          controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {},
          safetyState: "Unavailable", events: ["recording_started"] },
        { elapsedSeconds: 0.1, observedAtMs: 1100, actualCycleSeconds: 0.1, scan: 2, source: "Simulation",
          loopRevision: "sha256:loop-1", sp: 1, pv: 0.1, requestedMv: 0.25, appliedMv: 0.2,
          units: { sp: "%", pv: "%", mv: "%" }, quality: "good", freshnessMs: 0,
          controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {},
          safetyState: "Unavailable", events: ["recording_stopped"] },
      ], selectedIntervals: [{ startSeconds: 0, endSeconds: 0.1 }] },
      candidate: candidateFromControlPid(parameters, "SIMC"), method: "SIMC",
      evidence: { observation: "Observed", model: "Not evaluated" },
      conclusion: "Result retained for later comparison.",
    }, "2026-08-25T12:00:00.000Z");

    const mixed = structuredClone(result);
    mixed.recording.rawSamples[1].source = "Physical";
    assert.strictEqual(decodeWorkflowResult(mixed), undefined,
      "simulation evidence must not be accepted with a physical sample");

    const reversed = structuredClone(result);
    reversed.recording.rawSamples[1].elapsedSeconds = -0.1;
    assert.strictEqual(decodeWorkflowResult(reversed), undefined,
      "saved samples must remain in monotonic elapsed-time order");
  });

  test("creates a complete reproducible saved-run record", () => {
    const create = createWorkflowResult as unknown as (
      input: Record<string, unknown>, createdAt: string
    ) => Record<string, unknown>;
    const result = create({
      resultId: "run-20260826-1",
      resultRevision: 3,
      projectId: "project:tank",
      source: "Simulation",
      loop: { id: "loop:tank", revision: "sha256:loop-1" },
      controller: { instance: "Main.Controller", revision: "sha256:controller-1" },
      software: { extensionVersion: "0.24.64", runtimeVersion: "0.24.64",
        numericalAdapters: { tuning: "trust-control-tuning-v1" } },
      units: { sp: "%", pv: "%", mv: "%" },
      recording: { rawSamples: [{
        elapsedSeconds: 0, observedAtMs: 1000, actualCycleSeconds: 0.1, scan: 1,
        source: "Simulation", loopRevision: "sha256:loop-1", sp: 1, pv: 0,
        requestedMv: null, appliedMv: 0,
        units: { sp: "%", pv: "%", mv: "%" }, quality: "good", freshnessMs: 0,
        controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 },
        rateLimits: { downPerSecond: 20, upPerSecond: 20 }, safetyState: "Unavailable",
        events: ["recording_started"],
      }], selectedIntervals: [] },
      evidence: { observation: "Observed", model: "Not evaluated" },
      conclusion: "Recording retained for later identification.",
    }, "2026-08-26T12:00:00.000Z");

    assert.strictEqual(result.schemaVersion, 4);
    assert.strictEqual(result.resultId, "run-20260826-1");
    assert.strictEqual(result.resultRevision, 3);
    assert.deepStrictEqual((result.recording as { rawSamples: unknown[] }).rawSamples, [{
      elapsedSeconds: 0, observedAtMs: 1000, actualCycleSeconds: 0.1, scan: 1,
      source: "Simulation", loopRevision: "sha256:loop-1", sp: 1, pv: 0,
      requestedMv: null, appliedMv: 0,
      units: { sp: "%", pv: "%", mv: "%" }, quality: "good", freshnessMs: 0,
      controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 },
      rateLimits: { downPerSecond: 20, upPerSecond: 20 }, safetyState: "Unavailable",
      events: ["recording_started"],
    }]);
    assert.deepStrictEqual(decodeWorkflowResult(JSON.parse(exportWorkflowJson(result))), result);
  });

  test("retains validated experiment and plant context without changing evidence class", () => {
    const base = completeSavedRun("run-context", 1);
    const contextual = { ...base,
      experiment: { kind: "setpoint_step", requestedValue: 1, appliedValue: 1,
        minimum: 0, maximum: 100, approval: "Simulation confirmation", status: "completed",
        stopReason: "duration_complete", aborted: false,
        restoration: { value: 0, status: "confirmed" } },
      plant: { source: "Simulation", id: "tank", configurationRevision: "sha256:revision-1",
        configuration: { parameters: { gain: 1.5, time_constant_s: 4 }, initial_state: [0] },
        model: "first_order_dead_time", effectiveSeed: "42", uncertainties: ["gain-mismatch"],
        scheduledEvents: ["load-step"], actuator: "bounded valve", validityRange: "0 to 100 percent" },
    };
    assert.deepStrictEqual(decodeWorkflowResult(contextual), contextual);
    assert.strictEqual(decodeWorkflowResult({ ...contextual,
      plant: { ...contextual.plant, source: "Physical" } }), undefined,
    "simulation plant context must not be accepted as physical evidence");
    const { configuration: _configuration, ...incompletePlant } = contextual.plant;
    assert.strictEqual(decodeWorkflowResult({ ...contextual, plant: incompletePlant }), undefined,
      "simulation evidence must retain the configuration that produced it");
    assert.strictEqual(decodeWorkflowResult({ ...contextual,
      experiment: { ...contextual.experiment, restoration: { value: 0, status: "unknown" } } }), undefined);
  });

  test("decodes only one ordered saved-result history for the same loop", () => {
    const decodeHistory = (workflowResultModule as unknown as {
      decodeWorkflowResultHistory?: (value: unknown) => readonly unknown[] | undefined;
    }).decodeWorkflowResultHistory;
    assert.strictEqual(typeof decodeHistory, "function", "saved-result history decoding is not implemented");

    const first = completeSavedRun("run-1", 1);
    const second = { ...completeSavedRun("run-1", 2), createdAt: "2026-08-26T12:01:00.000Z" };
    assert.deepStrictEqual(decodeHistory?.([first, second]), [first, second]);
    assert.strictEqual(decodeHistory?.([second, first]), undefined, "revisions must be strictly ordered");
    assert.strictEqual(decodeHistory?.([first, { ...second, projectId: "project:other" }]), undefined,
      "one history must not mix project or loop identity");
  });

  test("exports the complete reproducible signal table as CSV", () => {
    const sample = completeSavedRun("run-csv", 1).recording.rawSamples[0];
    const csv = exportSamplesCsv([sample]);
    assert.strictEqual(csv.split("\n")[0], [
      "elapsed_seconds", "observed_at_ms", "actual_cycle_seconds", "sp", "pv",
      "requested_mv", "applied_mv",
      "sp_unit", "pv_unit", "mv_unit", "quality", "freshness_ms", "editor_observation_interval_seconds", "controller_mode",
      "safety_state", "output_minimum", "output_maximum", "rate_down_per_second",
      "rate_up_per_second", "source", "events",
    ].join(","));
    const cells = csv.split("\n")[1].split(",");
    assert.deepStrictEqual(cells.slice(2, 8), ["0.1", "1", "0", "", "0", "%"],
      "CSV must retain the actual interval and full-precision signal values");
    assert.deepStrictEqual(cells.slice(-2), ["Simulation", "recording_started"]);
  });

  test("rejects malformed saved runs without partially accepting history", () => {
    const decodeHistory = (workflowResultModule as unknown as {
      decodeWorkflowResultHistory: (value: unknown) => readonly unknown[] | undefined;
    }).decodeWorkflowResultHistory;
    const valid = completeSavedRun("run-invalid", 1);

    const invalidUnit = structuredClone(valid);
    invalidUnit.units.pv = "";
    invalidUnit.recording.rawSamples[0].units.pv = "";
    assert.strictEqual(decodeWorkflowResult(invalidUnit), undefined);
    assert.strictEqual(decodeWorkflowResult({ ...valid, schemaVersion: 99 }), undefined);
    assert.strictEqual(decodeWorkflowResult({ ...valid, projectId: "" }), undefined);
    assert.strictEqual(decodeWorkflowResult({ ...valid, recording: { ...valid.recording,
      rawSamples: [{ ...valid.recording.rawSamples[0], pv: Number.NaN }] } }), undefined);
    assert.strictEqual(decodeWorkflowResult({ ...valid, recording: { ...valid.recording,
      selectedIntervals: [{ startSeconds: 0, endSeconds: 1 }] } }), undefined);
    assert.strictEqual(decodeHistory([valid, invalidUnit]), undefined,
      "one invalid revision must reject the complete persisted history");
  });

  test("reports why stored result history cannot be restored", () => {
    const decodeState = (workflowResultModule as unknown as {
      decodeWorkflowResultHistoryState?: (value: unknown) =>
        { history: readonly unknown[]; unavailableReason?: string };
    }).decodeWorkflowResultHistoryState;
    assert.strictEqual(typeof decodeState, "function", "explicit stored-history recovery is not implemented");
    assert.deepStrictEqual(decodeState?.(undefined), { history: [] });
    assert.deepStrictEqual(decodeState?.([{ schemaVersion: 99 }]), {
      history: [],
      unavailableReason: "Saved Control Studio results use an unsupported or invalid format.",
    });
  });

  test("projects the executed experiment and saved simulation configuration into result evidence", () => {
    const module = workflowResultModule as unknown as {
      workflowExperimentContext?: (plan: unknown, terminal: unknown) => unknown;
      workflowSimulationPlantContext?: (saved: unknown, plantId: string, validityRange: string) => unknown;
    };
    assert.strictEqual(typeof module.workflowExperimentContext, "function");
    assert.strictEqual(typeof module.workflowSimulationPlantContext, "function");
    assert.deepStrictEqual(module.workflowExperimentContext?.({
      source: "simulation", kind: "prbs", affectedSignal: "ValveMV", address: "%QL0",
      from: 20, to: 25, minimum: 0, maximum: 100, restorationValue: 20,
      durationMs: 10_000, intervalMs: 500, seed: 7,
      trajectory: [{ atMs: 0, value: 25 }, { atMs: 500, value: 15 }],
      stopConditions: ["Abort", "10 s timeout", "Stale runtime data"],
    }, { status: "failed_to_restore", stop_reason: "signal_forced", restoration: "failed",
      restoration_reason: "confirmation_timeout", applied_value: 25 }), {
      kind: "prbs", affectedSignal: "ValveMV", address: "%QL0",
      requestedValue: 25, appliedValue: 25, durationMs: 10_000, intervalMs: 500, seed: 7,
      trajectory: [{ atMs: 0, value: 25 }, { atMs: 500, value: 15 }],
      stopConditions: ["Abort", "10 s timeout", "Stale runtime data"],
      minimum: 0, maximum: 100, approval: "Simulation confirmation",
      status: "failed_to_restore", stopReason: "signal_forced", aborted: false,
      restoration: { value: 20, status: "failed", reason: "confirmation_timeout" },
    });
    assert.deepStrictEqual(module.workflowSimulationPlantContext?.({
      configuration_revision: "sha256:plant-1",
      saved_configuration: {
        simulation: { seed: 42 },
        plants: [{ id: "tank", model: "first_order_dead_time",
          parameters: { gain: 1.5 }, initial_state: [20], actuator: { minimum: 0, maximum: 100 },
          uncertainties: [{ target: "gain", mode: "fixed", value: 0.1 }] }],
        plant_events: [{ plant: "tank", at_ms: 5000, kind: "set_parameter", parameter: "gain", value: 1.2 }],
      },
    }, "tank", "PV 0…100 °C; MV 0…100 %"), {
      source: "Simulation", id: "tank", configurationRevision: "sha256:plant-1",
      configuration: { id: "tank", model: "first_order_dead_time", parameters: { gain: 1.5 },
        initial_state: [20], actuator: { minimum: 0, maximum: 100 },
        uncertainties: [{ target: "gain", mode: "fixed", value: 0.1 }] },
      model: "first_order_dead_time", effectiveSeed: "42",
      uncertainties: ['{"target":"gain","mode":"fixed","value":0.1}'],
      scheduledEvents: ['{"plant":"tank","at_ms":5000,"kind":"set_parameter","parameter":"gain","value":1.2}'],
      actuator: '{"minimum":0,"maximum":100}', validityRange: "PV 0…100 °C; MV 0…100 %",
    });
  });

  test("keeps the last valid history when saving is interrupted or existing data is invalid", async () => {
    const save = (workflowResultModule as unknown as {
      saveWorkflowResultRevision?: (
        storage: { get(key: string): unknown; update(key: string, value: unknown): PromiseLike<void> },
        key: string, result: unknown, capacity: number
      ) => Promise<readonly unknown[]>;
    }).saveWorkflowResultRevision;
    assert.strictEqual(typeof save, "function", "atomic saved-result storage is not implemented");

    const first = completeSavedRun("run-atomic", 1);
    const second = { ...completeSavedRun("run-atomic", 2), createdAt: "2026-08-26T12:01:00.000Z" };
    let stored: unknown = [first];
    const interrupted = {
      get: () => stored,
      update: async () => { throw new Error("simulated interrupted workspace update"); },
    };
    await assert.rejects(() => save!(interrupted, "result", second, 20), /interrupted workspace update/);
    assert.deepStrictEqual(stored, [first]);

    let updates = 0;
    const invalid = { get: () => ({ malformed: true }), update: async () => { updates += 1; } };
    await assert.rejects(() => save!(invalid, "result", second, 20), /invalid/i);
    assert.strictEqual(updates, 0, "invalid existing storage must not be partially replaced");
  });

  test("projects compact saved-result history and comparison evidence", () => {
    const module = workflowResultModule as unknown as {
      workflowResultHistoryItems?: (history: readonly unknown[]) => readonly unknown[];
      compareWorkflowResultRevisions?: (left: unknown, right: unknown) => unknown;
    };
    assert.strictEqual(typeof module.workflowResultHistoryItems, "function",
      "saved-result history projection is not implemented");
    assert.strictEqual(typeof module.compareWorkflowResultRevisions, "function",
      "saved-result comparison projection is not implemented");
    const first = completeSavedRun("run-compare", 1);
    const second = { ...completeSavedRun("run-compare", 2), createdAt: "2026-08-26T12:01:00.000Z",
      method: "SIMC", conclusion: "Candidate passed validation." };
    assert.deepStrictEqual(module.workflowResultHistoryItems?.([first, second]), [
      { resultRevision: 1, createdAt: first.createdAt, source: "Simulation",
        controllerRevision: "sha256:controller-1", method: "Not selected", sampleCount: 1 },
      { resultRevision: 2, createdAt: second.createdAt, source: "Simulation",
        controllerRevision: "sha256:controller-1", method: "SIMC", sampleCount: 1 },
    ]);
    assert.deepStrictEqual(module.compareWorkflowResultRevisions?.(first, second), {
      left: { resultRevision: 1, source: "Simulation", controllerRevision: "sha256:controller-1",
        method: "Not selected", createdAt: first.createdAt, conclusion: first.conclusion },
      right: { resultRevision: 2, source: "Simulation", controllerRevision: "sha256:controller-1",
        method: "SIMC", createdAt: second.createdAt, conclusion: second.conclusion },
      sameEvidenceSource: true,
    });
  });

  test("exports the exact reopened revision instead of rebuilding its evidence", async () => {
    const select = (workflowResultModule as unknown as {
      workflowResultForExport?: <T>(opened: T | undefined, draft: () => T | Promise<T>) => Promise<T>;
    }).workflowResultForExport;
    assert.strictEqual(typeof select, "function", "reopened-result export selection is not implemented");
    const selectResult = select as NonNullable<typeof select>;
    const opened = completeSavedRun("run-export", 1);
    assert.strictEqual(await selectResult(opened, () => ({ ...opened, resultRevision: 2 })), opened);
    assert.strictEqual((await selectResult(undefined, () => ({ ...opened, resultRevision: 2 }))).resultRevision, 2);
  });

  test("reconstructs excluded intervals from a restored result without changing raw samples", () => {
    const restoreExclusions = (workflowResultModule as unknown as {
      excludedIntervalsFromSelection?: (
        endSeconds: number,
        selected: readonly { startSeconds: number; endSeconds: number }[]
      ) => readonly { startSeconds: number; endSeconds: number }[];
    }).excludedIntervalsFromSelection;
    assert.strictEqual(typeof restoreExclusions, "function", "restored interval conversion is not implemented");
    const selected = [{ startSeconds: 0, endSeconds: 2 }, { startSeconds: 3, endSeconds: 5 }];
    assert.deepStrictEqual(restoreExclusions?.(5, selected), [{ startSeconds: 2, endSeconds: 3 }]);
    assert.deepStrictEqual(selected, [{ startSeconds: 0, endSeconds: 2 }, { startSeconds: 3, endSeconds: 5 }]);
  });

  test("saves and restores the complete evidence-bound chart session", () => {
    const result = completeSavedRun("run-session", 1);
    const chartSession = {
      schemaVersion: 1 as const,
      boundary: "loop:tank:sha256:loop-1",
      activeStage: "Tune" as const,
      selectedPlots: { Monitor: "response", Tune: "bode", Validate: "requirements" },
      selectedSignals: ["sp", "pv", "mv"],
      window: [10, 60] as [number, number],
      expanded: true,
      candidateRevision: 2,
      pinnedCandidateRevisions: [1],
    };
    const restored = decodeWorkflowResult({ ...result, chartSession }) as unknown as {
      chartSession?: unknown;
    } | undefined;
    assert.deepStrictEqual(restored?.chartSession, chartSession);
    assert.strictEqual(decodeWorkflowResult({ ...result, chartSession: {
      ...chartSession, boundary: "loop:other:sha256:loop-1",
    } }), undefined);
  });
});

function completeSavedRun(resultId: string, resultRevision: number) {
  return createWorkflowResult({
    resultId, resultRevision, projectId: "project:tank", source: "Simulation",
    loop: { id: "loop:tank", revision: "sha256:loop-1" },
    controller: { instance: "Main.Controller", revision: "sha256:controller-1" },
    software: { extensionVersion: "0.24.64", numericalAdapters: { tuning: "trust-control-tuning-v1" } },
    units: { sp: "%", pv: "%", mv: "%" },
    recording: { rawSamples: [{ elapsedSeconds: 0, observedAtMs: 1000,
      actualCycleSeconds: 0.1, scan: 1, source: "Simulation", loopRevision: "sha256:loop-1",
      sp: 1, pv: 0, requestedMv: null, appliedMv: 0,
      units: { sp: "%", pv: "%", mv: "%" }, quality: "good",
      freshnessMs: 0, controllerMode: "Auto", outputLimits: { minimum: 0, maximum: 100 },
      rateLimits: {}, safetyState: "Unavailable", events: ["recording_started"] }],
      selectedIntervals: [] },
    evidence: { observation: "Observed", model: "Not evaluated" },
    conclusion: "Result retained for later comparison.",
  }, "2026-08-26T12:00:00.000Z");
}
