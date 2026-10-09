import * as assert from "assert";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import { steadyRecordPeriod } from "../../controlStudio/recordSequence";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";

function fixture(releaseFails: boolean, sensorQuality: number, change?: (result: any) => unknown, symbolicMode = false, unaddressedMode = false,
  measuredActuator = false, status?: (params: any) => unknown) {
  const signal = (symbol: string, address: string, iecType = "LREAL") => ({ symbol, address, iecType,
    direction: "read" as const, unit: "%", range: { minimum: 0, maximum: 100 } });
  const loop: ControlLoopDefinition = {
    schemaVersion: 2, id: "loop", name: "Custom loop", revision: "sha256:loop", projectId: "project", runtimeId: "simulator",
    controller: { instance: "P1", type: "Custom", design: "project", revision: "sha256:controller", action: "direct", taskPeriodMs: 10 },
    signals: { setpoint: signal("P1.SP", "%ML0"), processVariable: signal("P1.PV", "%IL0"),
      manipulatedVariable: signal("P1.MV", "%QL0"), additional: [
        { ...signal("P1.Valid", "%MX16.0", "BOOL"), id: "valid", channel: "primary", role: "quality" },
      ] },
    limits: { outputMinimum: 0, outputMaximum: 100 }, observedSampleTiming: { nominalPeriodMs: 10 },
  };
  if (measuredActuator) loop.signals.actuator = signal("P1.Stem", "%IL8");
  if (symbolicMode) { loop.controller.modeSignal = "P1.Valid"; loop.controller.modeWhenTrue = "Auto"; }
  if (unaddressedMode) { loop.controller.modeSignal = "P1.Manual"; loop.controller.modeWhenTrue = "Manual"; }
  const bindings: { id: string; address?: string; symbol?: string; iec_type: string }[] = [
    { id: "setpoint", address: "%ML0", iec_type: "LREAL" },
    { id: "processVariable", address: "%IL0", iec_type: "LREAL" },
    { id: "manipulatedVariable", address: "%QL0", iec_type: "LREAL" },
    ...(measuredActuator ? [{ id: "actuator", address: "%IL8", iec_type: "LREAL" }] : []),
    { id: "valid", address: "%MX16.0", iec_type: "BOOL" },
  ];
  if (unaddressedMode) bindings.push({ id: "controllerMode", symbol: "P1.Manual", iec_type: "BOOL" });
  let releaseCalls = 0;
  const send = async (operation: string, params?: unknown) => {
    if (operation === "control.recording.status" && status) return status(params);
    if (operation === "control.recording.release") {
      if (releaseFails && releaseCalls++ === 0) throw new Error("release reply lost");
      return { status: "released" };
    }
    const result = { recording_id: "record", loop_revision: loop.revision, program_revision: unaddressedMode ? "program" : undefined, bindings: bindings.map(binding => ({ ...binding })), stop_reason: "stopped",
      samples: [0, 1].map(index => ({ scan: index + 1, runtime_time_ns: index * 10_000_000,
        captured_at_ms: 1000, values: [1, 0.5, 2, ...(measuredActuator ? [1.5] : []), sensorQuality, ...(unaddressedMode ? [1] : [])],
        forced: bindings.map(() => false), quality: unaddressedMode ? bindings.map(() => "good") : undefined })),
      acquisition: { method: "runtime_scan_buffer", completeness: "complete", observedSampleCount: 2,
        scanRange: { first: 1, last: 2 }, missingScanRanges: [], droppedObservedSamples: 0,
        actualCycleTimingAvailable: true, freshnessAvailable: false, reason: "Captured every scan." },
    };
    return change ? change(result) : result;
  };
  const Constructor = RuntimeRecordingSession as unknown as new (...args: unknown[]) => RuntimeRecordingSession;
  return new Constructor(loop, "record", bindings, "Simulation", send, "", unaddressedMode ? "program" : undefined);
}

suite("Control Studio runtime recording client", () => {
  test("records about 200 samples per expected settling time, never more than one per execution (A4.2)", () => {
    const { recordPeriodExecutions } = require("../../controlStudio/controlStudioLimits");
    // A 45 min validation of a process that settles in 25 min, at a 100 ms task: one sample per 7.5 s, 75 executions each.
    assert.strictEqual(recordPeriodExecutions(2_700_000, 100, 100_000, 1_500), 75);
    // A fast process keeps every execution.
    assert.strictEqual(recordPeriodExecutions(30_000, 100, 100_000, 10), 1);
    // A recording keeps at least 200 samples, even when it is shorter than the settling time.
    assert.strictEqual(recordPeriodExecutions(600_000, 100, 100_000, 7_200), 30);
    // Without an expected settling time only the sample limit sets the period.
    assert.strictEqual(recordPeriodExecutions(28_800_000, 100, 100_000), 3);
  });
  test("retains the original native recording identity after Stop and release", async () => {
    const session = fixture(false, 1);
    const result = await session.stop();
    assert.strictEqual((result.evidence as any).recordingId, "record",
      "independent identification needs the acquisition identity of the selected native record");
    assert.strictEqual(((await session.stop()).evidence as any).recordingId, "record");
  });

  test("resolves a symbolic controller mode through its declared BOOL channel", async () => {
    const result = await fixture(false, 1, undefined, true).stop();
    assert.strictEqual(result.rawSamples?.[0].controllerMode, "Auto");
  });

  test("decodes an unaddressed symbolic BOOL mode and rejects bad-quality mode as unavailable", async () => {
    const result = await fixture(false, 1, raw => {
      raw.samples[1].quality[4] = "bad"; return raw;
    }, false, true).stop();
    assert.strictEqual(result.evidence?.programRevision, "program");
    assert.strictEqual(result.rawSamples?.[0].controllerMode, "Manual");
    assert.strictEqual(result.rawSamples?.[1].controllerMode, "Unavailable",
      "A numeric mode value with bad quality cannot establish current controller mode");
    assert.strictEqual(result.rawSamples?.[1].quality, "bad");
  });

  test("rejects malformed symbolic recording identities, quality vectors and BOOL values", async () => {
    for (const mutate of [
      (raw: any) => { raw.program_revision = "other"; },
      (raw: any) => { raw.bindings[4] = { ...raw.bindings[4], symbol: "Other.Manual" }; },
      (raw: any) => { raw.samples[0].quality = ["good"]; },
      (raw: any) => { raw.samples[0].values[4] = 2; },
    ]) {
      await assert.rejects(fixture(false, 1, raw => { mutate(raw); return raw; }, false, true).stop(),
        /recording|samples|BOOL/i);
    }
  });

  test("reports a declared actuator's measured position as the applied input and the command as the request", async () => {
    const sample = (await fixture(false, 1, undefined, false, false, true).stop()).rawSamples?.[0];
    assert.strictEqual(sample?.appliedMv, 1.5, "the applied input is what the actuator reached");
    assert.strictEqual(sample?.requestedMv, 2, "the manipulated variable is the requested output");
    const plain = (await fixture(false, 1).stop()).rawSamples?.[0];
    assert.strictEqual(plain?.appliedMv, 2, "without actuator feedback the command is the only applied input");
    assert.strictEqual(plain?.requestedMv, null);
  });

  test("retains auxiliary channel values and their declared meaning", async () => {
    const result = await fixture(false, 0).stop();
    assert.deepStrictEqual((result.rawSamples?.[0] as any).additional, [
      { id: "valid", channel: "primary", role: "quality", unit: "%", value: 0, quality: "bad" },
    ]);
  });

  test("rejects contradictory complete-acquisition claims", async () => {
    await assert.rejects(() => fixture(false, 1, result => ({ ...result,
      acquisition: { ...result.acquisition, missingScanRanges: [{ first: 1, last: 1 }] },
    })).stop(), /acquisition/i);
    await assert.rejects(() => fixture(false, 1, result => ({ ...result,
      acquisition: { ...result.acquisition, actualCycleTimingAvailable: false },
    })).stop(), /acquisition/i);
  });

  test("a lost release reply cannot discard an already received recording", async () => {
    const session = fixture(true, 1);
    const result = await session.stop();
    assert.strictEqual(result.rawSamples?.length, 2);
    await session.release(); // the first release reply is lost; the stopped buffer stays pending
    assert.strictEqual((await session.stop()).rawSamples?.length, 2);
    await session.release(); // the retry is acknowledged
  });

  test("exposes an immutable credential-free owner for extension-lifetime recovery", () => {
    const session = fixture(false, 1, undefined, false, true);
    assert.strictEqual(typeof (session as any).recoveryOwner, "function",
      "A recording must expose a bounded owner before Control Studio can persist and recover its original Stop path.");
    const owner = (session as any).recoveryOwner();
    assert.deepStrictEqual(owner, {
      schemaVersion: 1, recordingId: "record", projectId: "project", runtimeId: "simulator", loopId: "loop", loopRevision: "sha256:loop",
      source: "Simulation", programRevision: "program", bindings: [
        { id: "setpoint", address: "%ML0", iec_type: "LREAL" },
        { id: "processVariable", address: "%IL0", iec_type: "LREAL" },
        { id: "manipulatedVariable", address: "%QL0", iec_type: "LREAL" },
        { id: "valid", address: "%MX16.0", iec_type: "BOOL" },
        { id: "controllerMode", symbol: "P1.Manual", iec_type: "BOOL" },
      ],
    });
    owner.bindings[0].id = "mutated";
    assert.strictEqual((session as any).recoveryOwner().bindings[0].id, "setpoint",
      "Persisted recovery metadata must not mutate the live recording session.");
    assert.ok(!JSON.stringify(owner).match(/endpoint|token|authorization/i));
  });

  test("reconstructs Stop only for the persisted recording owner", async () => {
    const session = fixture(false, 1, undefined, false, true);
    const owner = (session as any).recoveryOwner();
    const calls: any[] = [];
    const api: any = RuntimeRecordingSession;
    assert.strictEqual(typeof api.fromRecoveryOwner, "function",
      "A host-recovery path must rebuild only the persisted recording owner before it can submit Stop.");
    const recovered = api.fromRecoveryOwner(session.loop, owner, async (operation: string, params: unknown) => {
      calls.push({ operation, params });
      return (session as any).send(operation, params);
    }, "");
    const result = await recovered.stop();
    assert.strictEqual(result.rawSamples.length, 2);
    assert.deepStrictEqual(calls[0], { operation: "control.recording.stop", params: { recording_id: "record", loop_revision: "sha256:loop" } });
    assert.throws(() => api.fromRecoveryOwner({ ...session.loop, revision: "sha256:changed" }, owner, async () => ({}), ""), /scope|loop/i);
  });

  test("keeps a lost Start acknowledgement incomplete after host recovery stops the same owner", async () => {
    const session = fixture(false, 1, undefined, false, true);
    const owner = { ...(session as any).recoveryOwner(), startAcknowledgementUnconfirmed: true };
    const calls: any[] = [];
    const api: any = RuntimeRecordingSession;
    const recovered = api.fromRecoveryOwner(session.loop, owner, async (operation: string, params: unknown) => {
      calls.push({ operation, params });
      return (session as any).send(operation, params);
    }, "");

    const result = await recovered.stop();

    assert.deepStrictEqual(calls[0], { operation: "control.recording.stop", params: { recording_id: "record", loop_revision: "sha256:loop" } });
    assert.strictEqual(result.evidence.completeness, "incomplete",
      "A recovered owner cannot upgrade an unacknowledged Start to complete evidence.");
    assert.match(result.evidence.reason, /Start acknowledgement unconfirmed/i);
    assert.match(result.evidence.reason, /Extension Host reload/i);
    assert.strictEqual(result.rawSamples.length, 2, "The same owner must still retain the runtime's actual samples.");
  });

  test("keeps an acknowledged recovered owner complete", async () => {
    const session = fixture(false, 1, undefined, false, true);
    const api: any = RuntimeRecordingSession;
    const recovered = api.fromRecoveryOwner(session.loop, (session as any).recoveryOwner(),
      (operation: string, params: unknown) => (session as any).send(operation, params), "");
    const result = await recovered.stop();
    assert.strictEqual(result.evidence.completeness, "complete");
    assert.doesNotMatch(result.evidence.reason, /Start acknowledgement unconfirmed/i);
  });

  test("preserves a false sensor-quality signal as bad identification data", async () => {
    const result = await fixture(false, 0).stop();
    assert.strictEqual(result.rawSamples?.[0].quality, "bad", "a complete acquisition can still contain bad-quality values");
    assert.strictEqual(result.evidence.completeness, "complete");
  });

  test("accepts samples at one steady record period and refuses a missed execution (A4.2, E4.5)", () => {
    assert.strictEqual(steadyRecordPeriod([10, 11, 12], false), true);
    assert.strictEqual(steadyRecordPeriod([10, 14, 18, 20], true), true, "a stopped recording's final sample may cover fewer executions");
    assert.strictEqual(steadyRecordPeriod([10, 14, 18, 20], false), false, "only a stopped recording ends with a partial period");
    assert.strictEqual(steadyRecordPeriod([10, 14, 19], true), false, "a final step longer than the period is a missed execution");
    assert.strictEqual(steadyRecordPeriod([10, 14, 17, 21], false), false, "a period that changes is a missed execution");
    assert.strictEqual(steadyRecordPeriod([10, 14], true), false, "two samples cannot establish a period, so they must be consecutive");
    assert.strictEqual(steadyRecordPeriod([10, 11], true), true);
  });

  test("follows a running recording through the runtime's status pages, which carry no loop revision (protocol annex E4.6)", async () => {
    // The exact fields of a status reply: identity, state, process time, counts, completeness, reason, one page and the cursor.
    const sample = (index: number) => ({ scan: index + 1, runtime_time_ns: index * 10_000_000, captured_at_ms: 1000,
      values: [1, 0.5, 2, 1], forced: [false, false, false, false], quality: ["good", "good", "good", "good"] });
    const requests: any[] = [];
    const session = fixture(false, 1, undefined, false, false, false, (params: any) => {
      requests.push(params);
      const from = params.from_sample;
      return { recording_id: "record", state: "recording", elapsed_ms: 10, sample_count: 2, complete: false, reason: null,
        samples: [0, 1].slice(from).map(sample), next_sample: 2 };
    });
    const first = await session.follow();
    assert.deepStrictEqual([first.state, first.sampleCount, first.rawSamples.length, first.nextSample], ["recording", 2, 2, 2]);
    assert.deepStrictEqual(first.rawSamples.map(value => value.scan), [1, 2]);
    const next = await session.follow();
    assert.strictEqual(next.rawSamples.length, 0, "the next page continues after the samples already read");
    assert.deepStrictEqual(requests.map(request => [request.loop_revision, request.from_sample]), [["sha256:loop", 0], ["sha256:loop", 2]],
      "each request names the loop revision, which the runtime checks");
  });
});
