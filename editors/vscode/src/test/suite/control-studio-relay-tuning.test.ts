import * as assert from "assert";

suite("Control Studio relay tuning", () => {
  test("excludes baseline and the incomplete initial relay cycle from repeatability", () => {
    const samples = [
      [0, 0, 0], [0.1, 0, 1], [0.3, 0.2, -1], [0.7, -0.2, 1],
      [1.1, 0.2, -1], [1.5, -0.2, 1], [1.9, 0.2, -1], [2.3, -0.2, 1],
      [2.7, 0.2, -1], [3.1, -0.2, 1], [3.2, 0, 0],
    ].map(([timeSeconds, processValue, output]) => ({ timeSeconds, processValue, output }));
    const result = relayApi().relayPiFromMeasuredRecord({ highOutput: 1, lowOutput: -1, samples });
    assert.strictEqual(result.status, "qualified", "recording startup and restoration are not complete relay periods");
    assert.strictEqual(result.cycles.length, 3);
    assert.ok(result.cycles.every((cycle: { periodSeconds: number }) => Math.abs(cycle.periodSeconds - 0.8) < 1e-12));
  });

  test("reports intermediate actuator values instead of missing relay transitions", () => {
    const samples = [-1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1].map((output, index) => ({
      timeSeconds: index * 0.1, output, processValue: Math.sin(index),
    }));
    assert.throws(() => relayApi().relayPiFromMeasuredRecord({ highOutput: 1, lowOutput: -1, samples }),
      /clipped or intermediate output values/, "Ramped relay cycles must not be treated as an ideal two-level relay");
  });

  test("calculates an unapplied PI candidate only from qualified feedback cycles", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.calculateRelayPiCandidate, "function",
      "relay tuning needs a qualified-cycle calculation rather than a time-driven output guess");
    const result = api.calculateRelayPiCandidate({
      highOutput: 80,
      lowOutput: 70,
      cycles: [
        { peak: 35.2, trough: 34.8, periodSeconds: 4 },
        { peak: 35.21, trough: 34.81, periodSeconds: 4.1 },
        { peak: 35.19, trough: 34.79, periodSeconds: 3.9 },
      ],
    });
    assert.strictEqual(result.status, "qualified");
    assert.ok(Math.abs(result.ultimateGain - 31.8309886) < 1e-5);
    assert.ok(Math.abs(result.periodSeconds - 4) < 1e-9);
    assert.ok(Math.abs(result.candidate.proportionalGain - 14.3239449) < 1e-5);
    assert.ok(Math.abs(result.candidate.integralTimeSeconds - 3.3333333) < 1e-5);
    assert.ok(Math.abs(result.candidate.integralGain - 4.2971835) < 1e-5);
    assert.strictEqual(result.candidate.applied, false);
    assert.deepStrictEqual(api.calculateRelayPiCandidate({ highOutput: 80, lowOutput: 70,
      cycles: [{ peak: 35.2, trough: 34.8, periodSeconds: 4 }] }), {
      status: "not_qualified",
      reason: "At least three complete feedback relay cycles are required.",
    });
  });

  test("rejects nonrepeatable relay amplitudes and periods before offering a candidate", () => {
    const api = relayApi();
    for (const variant of ["amplitude", "period"] as const) {
      const cycles = [1, 1, 2].map(factor => ({
        peak: variant === "amplitude" ? factor : 1,
        trough: 0,
        periodSeconds: variant === "period" ? factor : 1,
      }));
      const result = api.calculateRelayPiCandidate({ highOutput: 80, lowOutput: 70, cycles });
      assert.strictEqual(result.status, "not_qualified", `${variant} variation must block the candidate`);
      assert.match(result.reason, new RegExp(variant, "i"));
      assert.strictEqual("candidate" in result, false);
    }
    const session = new api.RelayTuningSession();
    const samples = relaySamples().map((sample, index) => ({
      ...sample, output: index === 7 ? 38 : sample.output,
    }));
    const terminal = session.augmentTerminalPayload(
      { kind: "relay", relayFeedback: { highOutput: 80, lowOutput: 70 } },
      { identification: { samples } },
    );
    assert.strictEqual(terminal.relayTuning.status, "not_qualified");
    assert.strictEqual(session.calculateCandidate().status, "not_qualified");
  });

  test("extracts relay cycles only from measured feedback-driven output transitions", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.extractRelayCycles, "function");
    const cycles = api.extractRelayCycles({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35.0, output: 80 },
      { timeSeconds: 1, processValue: 35.2, output: 70 },
      { timeSeconds: 2, processValue: 34.8, output: 80 },
      { timeSeconds: 3, processValue: 35.21, output: 70 },
      { timeSeconds: 4, processValue: 34.79, output: 80 },
      { timeSeconds: 5, processValue: 35.19, output: 70 },
      { timeSeconds: 6, processValue: 34.81, output: 80 },
      { timeSeconds: 7, processValue: 35.22, output: 70 },
      { timeSeconds: 8, processValue: 34.78, output: 80 },
    ] });
    assert.deepStrictEqual(cycles, [
      { peak: 35.21, trough: 34.79, periodSeconds: 2 },
      { peak: 35.19, trough: 34.79, periodSeconds: 2 },
      { peak: 35.22, trough: 34.78, periodSeconds: 2 },
    ]);
    assert.throws(() => api.extractRelayCycles({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35, output: 80 }, { timeSeconds: 1, processValue: 35.1, output: 80 },
      { timeSeconds: 2, processValue: 35.2, output: 80 },
    ] }), /transitions/i);
  });

  test("builds an unapplied relay PI candidate only after measured cycles qualify", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.relayPiFromMeasuredRecord, "function");
    const result = api.relayPiFromMeasuredRecord({ highOutput: 80, lowOutput: 70, samples: [
      { timeSeconds: 0, processValue: 35.0, output: 80 }, { timeSeconds: 1, processValue: 35.2, output: 70 },
      { timeSeconds: 2, processValue: 34.8, output: 80 }, { timeSeconds: 3, processValue: 35.21, output: 70 },
      { timeSeconds: 4, processValue: 34.79, output: 80 }, { timeSeconds: 5, processValue: 35.19, output: 70 },
      { timeSeconds: 6, processValue: 34.81, output: 80 }, { timeSeconds: 7, processValue: 35.22, output: 70 },
      { timeSeconds: 8, processValue: 34.78, output: 80 },
    ] });
    assert.strictEqual(result.status, "qualified");
    assert.strictEqual(result.candidate.applied, false);
    assert.strictEqual(result.cycles.length, 3);
  });

  test("accepts only a completed good-quality feedback-relay Test ledger record", () => {
    const api = relayApi();
    const plan = { kind: "relay", relayFeedback: { highOutput: 80, lowOutput: 70 } };
    const samples = [
      { timeSeconds: 0, input: 80, output: 35, quality: "good" }, { timeSeconds: 1, input: 70, output: 35.2, quality: "good" },
      { timeSeconds: 2, input: 80, output: 34.8, quality: "good" }, { timeSeconds: 3, input: 70, output: 35.21, quality: "good" },
      { timeSeconds: 4, input: 80, output: 34.79, quality: "good" }, { timeSeconds: 5, input: 70, output: 35.19, quality: "good" },
      { timeSeconds: 6, input: 80, output: 34.81, quality: "good" }, { timeSeconds: 7, input: 70, output: 35.22, quality: "good" },
      { timeSeconds: 8, input: 80, output: 34.78, quality: "good" },
    ];
    assert.strictEqual(api.relayPiFromExperimentRecord({ plan, samples }).status, "qualified");
    assert.throws(() => api.relayPiFromExperimentRecord({ plan: { kind: "mv_step" }, samples }), /feedback-relay/i);
    assert.throws(() => api.relayPiFromExperimentRecord({ plan, samples: [{ ...samples[0], quality: "bad" }] }), /good-quality/i);
  });

  test("retains qualified Test evidence until the engineer explicitly calculates an unapplied relay candidate", () => {
    const api = relayApi();
    assert.strictEqual(typeof api.RelayTuningSession, "function",
      "a relay Test needs a retained-evidence session before it can offer Calculate relay candidate");
    const session = new api.RelayTuningSession();
    const plan = { kind: "relay", relayFeedback: { highOutput: 80, lowOutput: 70 } };
    const samples = relaySamples();
    const terminal = session.augmentTerminalPayload(plan, { identification: { samples } });

    assert.deepStrictEqual(terminal.relayTuning, {
      status: "ready",
      cycles: 3,
      message: "Three measured feedback-relay cycles are ready. Calculate relay candidate to create an unapplied PI proposal.",
    });
    assert.strictEqual("candidate" in terminal.relayTuning, false,
      "qualifying a Test record must not silently create or apply controller settings");

    const calculation = session.calculateCandidate();
    assert.strictEqual(calculation.status, "qualified");
    assert.strictEqual(calculation.candidate.applied, false);
    assert.ok(Math.abs(calculation.candidate.proportionalGain - (9 / (Math.PI * 0.21))) < 1e-5,
      "the calculation must use the retained 0.2075-unit mean response amplitude, not the illustrative 0.20-unit example");
  });

  test("shows relay evidence before the explicit calculation and labels the result unapplied", () => {
    const presentation = relayPresentationApi();
    assert.strictEqual(typeof presentation.reduceRelayTuningPresentation, "function",
      "the Test workspace needs a dedicated visible-state owner for relay evidence and its explicit calculation");
    let state = presentation.initialRelayTuningPresentation();
    state = presentation.reduceRelayTuningPresentation(state, "terminal", { relayTuning: {
      status: "ready", cycles: 4,
      message: "Four measured feedback-relay cycles are ready. Calculate relay candidate to create an unapplied PI proposal.",
    } });
    assert.deepStrictEqual(state, {
      phase: "ready", visible: true, calculateVisible: true, calculateEnabled: true,
      candidateVisible: false, cycles: 4,
      status: "Four measured feedback-relay cycles are ready. Calculate relay candidate to create an unapplied PI proposal.",
      candidate: undefined,
    });
    state = presentation.reduceRelayTuningPresentation(state, "relay-candidate", { relayTuning: {
      status: "qualified", ultimateGain: 30.68047096, periodSeconds: 2,
      candidate: { proportionalGain: 13.80621193, integralTimeSeconds: 1.66666667, integralGain: 8.28372716, applied: false },
    } });
    assert.strictEqual(state.phase, "calculated");
    assert.strictEqual(state.calculateVisible, false);
    assert.strictEqual(state.candidateVisible, true);
    assert.match(state.status, /unapplied.*PI/i);
    assert.deepStrictEqual(state.candidate, { proportionalGain: 13.80621193, integralTimeSeconds: 1.66666667, integralGain: 8.28372716, applied: false });
  });
});

function relaySamples() {
  return [
    { timeSeconds: 0, input: 80, output: 35, quality: "good" }, { timeSeconds: 1, input: 70, output: 35.2, quality: "good" },
    { timeSeconds: 2, input: 80, output: 34.8, quality: "good" }, { timeSeconds: 3, input: 70, output: 35.21, quality: "good" },
    { timeSeconds: 4, input: 80, output: 34.79, quality: "good" }, { timeSeconds: 5, input: 70, output: 35.19, quality: "good" },
    { timeSeconds: 6, input: 80, output: 34.81, quality: "good" }, { timeSeconds: 7, input: 70, output: 35.22, quality: "good" },
    { timeSeconds: 8, input: 80, output: 34.78, quality: "good" },
  ];
}

function relayApi(): any {
  try { return require("../../controlStudio/relayTuning"); } catch { return {}; }
}

function relayPresentationApi(): any {
  try { return require("../../controlStudio/relayTuningPresentation"); } catch { return {}; }
}
