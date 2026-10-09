import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { libraryPidInitialization } from "../../controlStudio/libraryPidInitialization";
import { declaredCommissioningProfile, declaredTransferBinding } from "../../controlStudio/controllerInitialization";
import { engineerVerificationPlan } from "../../controlStudio/engineerVerificationPlan";
import { buildControlExperimentPlan, ControlExperimentClient } from "../../controlStudio/experiment";
import { ConnectedIdentificationMode } from "../../controlStudio/connectedIdentificationMode";
import { ConnectedIdentification } from "../../controlStudio/connectedIdentification";
import { evenlySampled } from "../../controlStudio/evenSampling";
import { observedTimeScale } from "../../controlStudio/settleDetection";
import { RuntimeControlError } from "../../runtimeControlClient";
import type { TransferBinding } from "../../controlStudio/proportionalTransfer";
import { materializeSelectedModelSimulation } from "../../controlStudio/modelSimulation";
import { decodeCompiledCatalog, pidCandidates, pidParameterRecords, pidLoopDefinition, setupFormModel } from "../../controlStudio/loopSetup/pidLoopSetup";
import type { CompiledCatalog } from "../../controlStudio/loopSetup/compiledCatalog";
import { PARAMETER_FIELDS, pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";
import { open } from "./control-studio-loop-setup-form-harness";

/**
 * Specification 35 §17.1 and §17.3: the loop Create PLC code writes (specification 39 §10.2) as Control Studio's compiled
 * evidence shows it. The temperature reaches the PID through a 4–20 mA input and a sensor block, the manual output and setpoint
 * have no address, the Auto/Manual signal is TRUE in Manual and the manual output follows the output in Auto.
 */
const generated: CompiledCatalog = {
  symbols: [
    { symbol: "Main.TT101_Signal", type: "LREAL", address: "%IL8" },
    { symbol: "Main.TT101.Value", type: "LREAL" },
    { symbol: "Main.EH101_Power", type: "LREAL", address: "%QL8" },
    { symbol: "Main.EH101.AutoPower", type: "LREAL" },
    { symbol: "Main.TIC101_Setpoint", type: "LREAL" },
    { symbol: "Main.TIC101_ManualOutput", type: "LREAL" },
    { symbol: "Main.TIC101_Output", type: "LREAL" },
    { symbol: "Main.TIC101_Manual", type: "BOOL" },
    { symbol: "Main.TIC101_Initialized", type: "BOOL" },
    ...["Setpoint", "ProcessValue", "ManualOutput", "CycleTime"].map(field => ({ symbol: `Main.TIC101_Input.${field}`, type: "LREAL" })),
    ...pidSymbols("Main.TIC101", "Main.TIC101_Parameters"),
    // Control.PID's own copy of the record its Configure receives.
    ...PARAMETER_FIELDS.map(field => ({ symbol: `Main.TIC101.ParametersValue.${field}`, type: "LREAL" })),
  ],
  controllers: [{ instance: "Main", type: "Main", tasks: TASK }, { instance: "Main.EH101", type: "Plant.FB_Heater", tasks: TASK },
    { instance: "Main.TIC101", type: "Control.PID", tasks: TASK }, { instance: "Main.TT101", type: "Plant.FB_TemperatureInput", tasks: TASK }],
  links: [
    { instance: "Main.TIC101", method: "Configure", argument: "Parameters", record: "Main.TIC101_Parameters", output: null, fields: {} },
    { instance: "Main.TIC101", method: "Initialize", argument: "State", record: "Main.TIC101_Start", output: null,
      fields: { Setpoint: ["Main.TIC101_Setpoint"] }, guardedBy: "Main.TIC101_Initialized" },
    { instance: "Main.TIC101", method: "Update", argument: "Input", record: "Main.TIC101_Input", output: "Main.TIC101_Output",
      fields: { ManualOutput: ["Main.TIC101_ManualOutput"], ProcessValue: ["Main.TT101"], Setpoint: ["Main.TIC101_Setpoint"] },
      mode: { symbol: "Main.TIC101_Manual", whenTrue: "Manual" },
      copiesOfOutput: ["Main.TIC101_Start.NominalOutput", "Main.TIC101_ManualOutput", "Main.EH101.AutoPower"] },
  ],
  writes: ["Main.EH101.AutoPower", "Main.TIC101_Initialized", "Main.TIC101_Input.CycleTime", "Main.TIC101_Input.ManualOutput",
    "Main.TIC101_Input.ProcessValue", "Main.TIC101_Input.Setpoint", "Main.TIC101_ManualOutput", "Main.TIC101_Output"],
};

const OWN = "Main.TIC101.StateValue.ProcessValue";

function draft(overrides: Record<string, unknown> = {}) {
  return { ...scope, name: "Tank temperature", instance: "Main.TIC101", action: "direct", parameterRecord: "Main.TIC101_Parameters",
    processVariable: { symbol: OWN, unit: "degC", minimum: 0, maximum: 100 },
    manipulatedVariable: { symbol: "Main.TIC101_Output", unit: "%", minimum: 0, maximum: 100 },
    setpoint: { symbol: "Main.TIC101_Setpoint" }, mode: { symbol: "Main.TIC101_Manual", whenTrue: "Manual" },
    manualOutput: "Main.TIC101_ManualOutput", startupFlag: "Main.TIC101_Initialized", ...overrides } as any;
}

const model = { kind: "transfer-function", numerator: [0.2], denominator: [300, 1], delaySeconds: 0, revision: "tank", source: "provided",
  validation: "validated", evidence: ["fixture"], inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: 0, maximum: 100 } }], operatingPoint: { inputs: [30], outputs: [45] } } as any;

const values = { "Main.TIC101_Parameters.ProportionalGain": 33.3, "Main.TIC101_Parameters.IntegralGain": 0.0667,
  "Main.TIC101_Parameters.DerivativeGain": 0, "Main.TIC101_Parameters.ProportionalSetpointWeight": 1,
  "Main.TIC101_Parameters.DerivativeSetpointWeight": 0, "Main.TIC101_Parameters.Bias": 0, "Main.TIC101_Parameters.OutputMinimum": 0,
  "Main.TIC101_Parameters.OutputMaximum": 100, "Main.TIC101_Parameters.RateMinimum": -1000, "Main.TIC101_Parameters.RateMaximum": 1000,
  "Main.TIC101_Parameters.TrackingTime": 1 };

suite("Control Studio on a loop Create PLC code wrote", () => {
  test("reads the startup flag, the Auto/Manual signal and the output copies from the compiled links", () => {
    const decoded = decodeCompiledCatalog(JSON.parse(JSON.stringify(generated)))!;
    assert.deepStrictEqual(decoded.links!.map(link => [link.method, link.guardedBy, link.mode, link.copiesOfOutput]), [
      ["Configure", undefined, undefined, undefined],
      ["Initialize", "Main.TIC101_Initialized", undefined, undefined],
      ["Update", undefined, { symbol: "Main.TIC101_Manual", whenTrue: "Manual" },
        ["Main.TIC101_Start.NominalOutput", "Main.TIC101_ManualOutput", "Main.EH101.AutoPower"]]]);
    assert.deepStrictEqual(pidCandidates(decoded), [{
      instance: "Main.TIC101", task: { name: "MainTask", intervalMs: 100 }, parameterRecord: "Main.TIC101_Parameters",
      output: "Main.TIC101_Output", setpoint: "Main.TIC101_Setpoint", processValue: "Main.TT101", manualOutput: "Main.TIC101_ManualOutput",
      ownMeasurement: OWN, mode: { symbol: "Main.TIC101_Manual", whenTrue: "Manual" }, startupFlag: "Main.TIC101_Initialized",
      manualOutputFollowsOutput: true,
    }]);
    assert.deepStrictEqual(pidParameterRecords(decoded), ["Main.TIC101_Parameters"],
      "the PID's own copy is not offered: Configure overwrites it from the program's record every scan");
  });

  test("prefills the PID's own measurement and an unaddressed manual output, mode and startup flag", () => {
    const model = setupFormModel(generated);
    assert.deepStrictEqual(model.prefill, { instance: "Main.TIC101", parameterRecord: "Main.TIC101_Parameters",
      manipulatedVariable: "Main.TIC101_Output", setpoint: "Main.TIC101_Setpoint", processValue: OWN, manualOutput: "Main.TIC101_ManualOutput",
      mode: "Main.TIC101_Manual", modeWhenTrue: "Manual", startupFlag: "Main.TIC101_Initialized" });
    assert.ok(model.measurements!.includes(OWN), "the PID's own measurement is never refused as computed");
    const view = open(model);
    try {
      const field = (name: string) => view.root.querySelector<HTMLSelectElement>(`[data-setup-field="${name}"]`)!.value;
      assert.deepStrictEqual(["processVariable", "manualOutput", "mode", "modeWhenTrue", "startupFlag", "parameterRecord", "setpoint"].map(field),
        [OWN, "Main.TIC101_ManualOutput", "Main.TIC101_Manual", "Manual", "Main.TIC101_Initialized", "Main.TIC101_Parameters", "Main.TIC101_Setpoint"]);
      for (const [name, value] of [["name", "Tank temperature"], ["processVariable.unit", "degC"], ["processVariable.minimum", "0"],
        ["processVariable.maximum", "100"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"]]) view.set(name, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      const saved = view.posts[view.posts.length - 1]?.draft;
      assert.deepStrictEqual([saved?.mode, saved?.manualOutput, saved?.startupFlag], [{ symbol: "Main.TIC101_Manual", whenTrue: "Manual" },
        "Main.TIC101_ManualOutput", "Main.TIC101_Initialized"], "the draft carries the prefilled choices, with no %M mapping asked for");
    } finally { view.close(); }
  });

  test("generates a loop on the PID's own measurement whose manual output follows the output", () => {
    const loop = pidLoopDefinition(generated, draft());
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.doesNotThrow(() => libraryPidInitialization(loop));
    assert.deepStrictEqual([loop.signals.processVariable.symbol, loop.signals.processVariable.address], [OWN, undefined]);
    assert.deepStrictEqual([loop.controller.manualOutputSignal, loop.controller.manualOutputAddress, loop.controller.manualOutputFollowsOutput],
      ["Main.TIC101_ManualOutput", undefined, true]);
    assert.strictEqual(loop.parameterBinding!.mode.manual_value, true, "TIC101_Manual is TRUE in Manual");
    const elsewhere = pidLoopDefinition(generated, draft({ name: "Other manual output", manualOutput: "Main.TIC101_Setpoint" }));
    assert.strictEqual(elsewhere.controller.manualOutputFollowsOutput, undefined, "only the variable the code copies the output into follows it");
  });

  test("gives the model's measurement to the PID in a model run and leaves that state out of the transfer", () => {
    const loop = pidLoopDefinition(generated, draft());
    const { request } = engineerVerificationPlan(loop, model,
      { referenceChange: 2, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 200, durationSeconds: 400 } as any, values);
    const toml = request.simulation_override!.simulation_toml;
    assert.match(toml, /^feedback_targets = \["pid:Main\.TIC101"\]$/m, toml);
    assert.match(toml, /^command_sources = \["Main\.TIC101_Output"\]$/m);
    const definition = libraryPidInitialization(loop);
    const binding = declaredTransferBinding(loop, definition);
    assert.ok(!binding.states.some(state => state.symbol === OWN), "the measurement is read at the boundary, never written");
    assert.ok(binding.guards.some(guard => guard.symbol === OWN), "it is the transfer's measurement");
    const tracking = declaredCommissioningProfile(loop, model, definition, "Auto entry").tracking(loop, binding, true) as any;
    assert.ok(!(OWN in tracking.initialization_values));
    assert.ok(tracking.bounded_state_symbols.includes("Main.TIC101_ManualOutput"),
      "the program copies the output into the manual output in Auto, so after the scan it is bounded, not fixed");
    assert.ok(!tracking.bounded_state_symbols.includes(OWN));
  });

  test("switches to Manual for identification with the manual output the code sets as a bounded state", () => {
    const loop = pidLoopDefinition(generated, draft());
    const mode = new ConnectedIdentificationMode(loop, async () => ({}), { read: () => undefined, write: async () => undefined });
    const tracking = (mode as unknown as { tracking(status: { values: Record<string, number> }): { bounded_state_symbols: string[] } })
      .tracking({ values: {} });
    assert.ok(tracking.bounded_state_symbols.includes("Main.TIC101_ManualOutput"),
      "in Auto the code copies the output into the manual output every scan, also just before a transfer to Manual");
  });

  test("returns to Auto after identification allowing the PID's own first step, not a fixed 1e-6", () => {
    const loop = pidLoopDefinition(generated, draft());
    const mode = new ConnectedIdentificationMode(loop, async () => ({}), { read: () => undefined, write: async () => undefined });
    mode.processNoise = 0.05;
    const allowance = (mode as unknown as { autoEntryResponse(status: { values: Record<string, number> }): number }).autoEntryResponse({ values: {
      "Main.TIC101_Parameters.ProportionalGain": 33.3, "Main.TIC101_Parameters.IntegralGain": 0.0667, "Main.TIC101_Parameters.DerivativeGain": 0,
      "Main.TIC101_Setpoint": 45, [OWN]: 43 } });
    // Kp·2n from the measurement's noise, and Ki·Δt·(|e| + n): the integral step on the 2 °C the identification moved it by.
    const expected = 2 * 33.3 * 0.05 + 0.0667 * 0.1 * 0.05 + 0.0667 * 0.1 * 2;
    assert.ok(Math.abs(allowance - expected) < 1e-9, `${allowance} versus ${expected}`);
  });

  test("returns a loop to Auto after identification without writing back the manual output the code sets", async () => {
    const loop = pidLoopDefinition(generated, draft());
    // The runtime's answer for any binding: Manual now (identification's mode), every member a number.
    const status = (binding: TransferBinding, manual: boolean) => ({ program_revision: "program", binding_revision: "binding", status: "idle",
      reason: null, transaction: null, values: Object.fromEntries([
        ...[binding.mode, ...(binding.additional_modes ?? [])].map(mode => [mode.symbol, manual]),
        ...[...binding.states, ...binding.guards, binding.output, ...(binding.additional_outputs ?? [])].map(member => [member.symbol, 1])]) });
    let written: TransferBinding | undefined;
    const request = async (operation: string, params: Record<string, unknown>) => {
      const binding = params.binding as TransferBinding;
      if (operation === "control.transfer.status") return status(binding, true);
      if (operation === "control.transfer.apply") { written = binding; throw new RuntimeControlError("stop here", "invalid_transfer_request"); }
      throw new Error(`unexpected ${operation}`);
    };
    const probe = new ConnectedIdentificationMode(loop, request, { read: () => undefined, write: async () => undefined });
    const binding = (probe as unknown as { binding: TransferBinding }).binding;
    // Before identification the loop was in Auto.
    const checkpoint = { loopRevision: loop.revision, original: status(binding, false) as never };
    const mode = new ConnectedIdentificationMode(loop, request, { read: () => checkpoint, write: async () => undefined });
    await assert.rejects(mode.restore(), /stop here/);
    assert.deepStrictEqual(written?.states.map(state => state.symbol), ["Main.TIC101_Setpoint"],
      "the setpoint returns; the manual output is the output's copy of then, which Auto overwrites");
  });

  test("reports the observed time scale of a campaign from its process and wall times", () => {
    assert.strictEqual(observedTimeScale(60, 6_000), 10, "60 s of process time in 6 s of wall time is 10x");
    assert.strictEqual(observedTimeScale(60, 500), undefined, "less than a second of wall time is not yet an observation");
  });

  test("refuses to reattach when the journal holds no running experiment", async () => {
    const connected = new ConnectedIdentification(async () => ({}), {
      sample: async () => ({}) as never, recordingOwner: () => undefined, persist: async () => undefined, clear: async () => undefined,
      retain: async () => undefined, mode: { read: () => undefined, write: async () => undefined },
      campaign: { read: () => undefined, write: async () => undefined },
    });
    await assert.rejects(connected.resume(pidLoopDefinition(generated, draft()), {} as never, async () => undefined),
      /no running experiment to resume/);
  });

  test("compares a live plant's recording on an even grid although its task runs every 100 or 110 ms", () => {
    // A 100 ms task in a 10 ms resource: it runs at the first cycle after it is due, every 100 or 110 ms.
    const intervals = Array.from({ length: 39 }, (_, i) => (i % 3 === 2 ? 0.11 : 0.1));
    const times = intervals.reduce((all, interval) => [...all, all[all.length - 1] + interval], [0]);
    const sample = (time: number, i: number) => ({ timeSeconds: time, sequence: 7 + i, values: { PV: { value: i, quality: "good" as const } } });
    const { samples, step } = evenlySampled(times.map(sample));
    assert.ok(Math.abs(step - 0.1) < 1e-9, `the median spacing: ${step}`);
    assert.ok(samples.every((entry, k) => Math.abs(entry.timeSeconds - k * 0.1) < 1e-9 && entry.sequence === 7 + k), "an even grid");
    assert.deepStrictEqual(samples.slice(0, 5).map(entry => entry.values.PV.value), [0, 1, 2, 2, 3],
      "each grid time holds the latest sample at or before it, as a PLC holds a value");
    assert.throws(() => evenlySampled(times.map((time, i) => sample(i === 20 ? time + 0.2 : time, i))), /missing or irregular/,
      "a gap of more than one and a half spacings is refused");
    assert.throws(() => evenlySampled(times.map((time, i) => ({ ...sample(time, i), sequence: i === 5 ? 99 : 7 + i }))), /missing or irregular/,
      "a missing scan is refused");
  });

  test("accepts a PID's measurement only as a plant feedback", () => {
    const base = { plantId: "selected", stepMs: 100, initialState: [0], initialInput: [0], initialDelayedInput: 0 };
    assert.doesNotThrow(() => materializeSelectedModelSimulation(model, { ...base, commandSources: ["Main.TIC101_Output"], feedbackTargets: ["pid:Main.TIC101"] }));
    assert.throws(() => materializeSelectedModelSimulation(model, { ...base, commandSources: ["pid:Main.TIC101"], feedbackTargets: ["%IL8"] }),
      /command bindings/);
  });

  test("steps an unaddressed manual output or setpoint by name", async () => {
    const loop = pidLoopDefinition(generated, draft());
    const sample = (mode: "Auto" | "Manual") => ({ availability: "available", source: "Simulation", loopId: loop.id, loopRevision: loop.revision,
      controller: { instance: "Main.TIC101", revision: "sha256:unverified", mode }, scan: 7, observedAtMs: 700,
      setpoint: { symbol: "Main.TIC101_Setpoint", value: 45, unit: "degC", quality: "good" },
      processVariable: { symbol: OWN, value: 44.6, unit: "degC", quality: "good" },
      manipulatedVariable: { symbol: "Main.TIC101_Output", value: 30, unit: "%", quality: "good" },
      outputLimits: { minimum: 0, maximum: 100 }, rateLimits: {}, saturation: "none", recording: false }) as any;
    const output = buildControlExperimentPlan({ loop, sample: sample("Manual"), kind: "mv_step", amplitude: 10, durationSeconds: 120 });
    assert.strictEqual(output.kind, "accepted", output.kind === "rejected" ? output.reason : "");
    const plan = output.kind === "accepted" ? output.plan : undefined!;
    assert.deepStrictEqual([plan.affectedSignal, plan.address, plan.to], ["Main.TIC101_ManualOutput", undefined, 40]);
    const sent: Record<string, unknown>[] = [];
    await new ControlExperimentClient(async (_operation, params) => { sent.push(params as Record<string, unknown>); return {}; }).start(plan, false);
    assert.strictEqual(sent[0].symbol, "Main.TIC101_ManualOutput", "no address: the runtime writes the variable by name");
    assert.ok(!("address" in sent[0]));
    const setpoint = buildControlExperimentPlan({ loop, sample: sample("Auto"), kind: "setpoint_step", amplitude: 2, durationSeconds: 120 });
    assert.deepStrictEqual(setpoint.kind === "accepted" ? [setpoint.plan.affectedSignal, setpoint.plan.address] : setpoint,
      ["Main.TIC101_Setpoint", undefined]);
  });
});
