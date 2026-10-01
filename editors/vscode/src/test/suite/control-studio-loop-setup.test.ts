import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { libraryPidInitialization } from "../../controlStudio/libraryPidInitialization";
import { cascade, cascadeTogetherDraft, pidSymbols, scope, TASK } from "./control-studio-cascade-fixtures";

/** Loaded dynamically so that a missing setup owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

/** The compiled catalog of a single-loop program shaped like the model-based tuning lesson. */
const singleLoop = {
  symbols: [
    { symbol: "P1.TemperatureC", type: "LREAL", address: "%IL8" },
    { symbol: "P1.AppliedPowerPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" },
    { symbol: "P1.AutoMode", type: "BOOL", address: "%MX96.0" },
    { symbol: "P1.ManualOutput", type: "LREAL", address: "%ML72" },
    { symbol: "P1.SafeProcessValueC", type: "LREAL" },
    ...pidSymbols("P1.Controller", "P1.Parameters"),
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Controller", type: "Control.PID", tasks: TASK }],
  links: [
    { instance: "P1.Controller", method: "Configure", argument: "Parameters", record: "P1.Parameters", output: null, fields: {} },
    { instance: "P1.Controller", method: "Update", argument: "Input", record: "P1.Input", output: "P1.AppliedPowerPct",
      fields: { ManualOutput: ["P1.ManualOutput"], ProcessValue: ["P1.SafeProcessValueC"], Setpoint: ["P1.SetpointC"], TrackingOutput: ["P1.ManualOutput"] } },
  ],
  // The program computes the checked measurement from the temperature and writes the PID output.
  writes: ["P1.SafeProcessValueC", "P1.AppliedPowerPct"],
};

suite("Control Studio loop setup from compiled facts", () => {
  test("prefills each library PID from its compiled calls without guessing from names", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidCandidates, "function", "setup must read the compiled catalog");
    assert.deepStrictEqual(api.pidCandidates(singleLoop), [{
      instance: "P1.Controller", task: { name: "MainTask", intervalMs: 100 }, parameterRecord: "P1.Parameters",
      output: "P1.AppliedPowerPct", setpoint: "P1.SetpointC", processValue: "P1.SafeProcessValueC", manualOutput: "P1.ManualOutput",
    }]);
  });

  test("generates a Control.PID loop that the existing tuning and application contracts accept", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidLoopDefinition, "function", "setup must write the complete loop definition");
    const loop = api.pidLoopDefinition(singleLoop, {
      ...scope, name: "Heater temperature", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 },
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput", experimentStep: 20,
      experimentDurationSeconds: 8,
    });
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.strictEqual(loop.experiment.amplitude, 20, "the chosen identification step is the experiment's step");
    assert.strictEqual(loop.experiment.durationSeconds, 8, "the chosen identification duration is the experiment's duration");
    assert.doesNotThrow(() => libraryPidInitialization(loop), "all seven retained states are bound with their units and coordinates");
    assert.strictEqual(loop.controller.type, "Control.PID");
    assert.strictEqual(loop.controller.taskPeriodMs, 100);
    assert.deepStrictEqual([loop.controller.modeSignal, loop.controller.modeWhenTrue, loop.controller.manualOutputSignal, loop.controller.manualOutputAddress],
      ["P1.AutoMode", "Auto", "P1.ManualOutput", "%ML72"]);
    assert.strictEqual(loop.controller.law.parameterRecordSymbol, "P1.Parameters");
    const units = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol.split(".").pop(), parameter.unit]));
    assert.deepStrictEqual([units.ProportionalGain, units.IntegralGain, units.DerivativeGain, units.RateMaximum, units.TrackingTime],
      ["%/degC", "%/(degC*s)", "%*s/degC", "%/s", "s"]);
    assert.deepStrictEqual([loop.signals.processVariable.address, loop.signals.manipulatedVariable.address, loop.signals.setpoint.address],
      ["%IL8", "%QL0", "%ML0"]);
    assert.strictEqual(loop.experiment.execution, "control-studio", "Studio's own identification methods drive the plant");
  });

  test("bounds the output bias as an offset, so a zero bias stays valid when the output range excludes zero", () => {
    const api = setup();
    // A cascade's outer output is a temperature setpoint, 20–80 °C; the program's zero bias adds nothing to it.
    const loop = api.pidLoopDefinition(singleLoop, {
      ...scope, name: "Outer", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 },
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "degC", minimum: 20, maximum: 80 },
      setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput", experimentDurationSeconds: 30,
    });
    const bias = loop.parameterBinding.parameters.find((parameter: any) => parameter.symbol === "P1.Parameters.Bias");
    assert.deepStrictEqual([bias.minimum, bias.maximum], [-60, 60], "the bias may shift the output by at most its span either way");
  });

  test("offers only complete Control.PID parameter records", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidParameterRecords, "function");
    const partial = { ...singleLoop, symbols: [...singleLoop.symbols, { symbol: "P1.Other.ProportionalGain", type: "LREAL" }] };
    assert.deepStrictEqual(api.pidParameterRecords(partial), ["P1.Parameters"], "a record missing any of the eleven numeric fields is not offered");
    assert.deepStrictEqual(api.pidParameterRecords(cascade), ["P1.InnerParameters", "P1.OuterParameters"]);
  });

  test("opens the setup form on the structure the compiled program shows, with only usable choices", () => {
    const api = setup();
    assert.strictEqual(typeof api.setupFormModel, "function");
    const single = api.setupFormModel(singleLoop);
    assert.strictEqual(single.structure, "pid");
    assert.deepStrictEqual(single.pids.map((pid: any) => pid.instance), ["P1.Controller"]);
    assert.deepStrictEqual(single.addressedReal, ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SetpointC", "P1.TemperatureC"],
      "identification writes the manual output through the process image");
    assert.deepStrictEqual(single.signals, ["P1.AppliedPowerPct", "P1.ManualOutput", "P1.SafeProcessValueC", "P1.SetpointC", "P1.TemperatureC"],
      "outputs and setpoints may be program variables without an address; the PID's own records and states are not signals");
    assert.deepStrictEqual(single.measurements, ["P1.TemperatureC"],
      "a simulated plant writes inputs and program variables the program only reads; it computes P1.SafeProcessValueC itself");
    assert.deepStrictEqual(single.booleans, ["P1.AutoMode"]);
    assert.deepStrictEqual(single.records, ["P1.Parameters"]);
    assert.deepStrictEqual(single.prefill, { instance: "P1.Controller", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.AppliedPowerPct",
      setpoint: "P1.SetpointC", manualOutput: "P1.ManualOutput", computed: { processValue: "P1.SafeProcessValueC" } },
      "a computed measurement link is not prefilled, but named so that the form can say why");
    const pair = api.setupFormModel(cascade);
    assert.strictEqual(pair.structure, "cascade");
    assert.deepStrictEqual(pair.cascade, { outer: "P1.Outer", inner: "P1.Inner" });
    assert.strictEqual(api.setupFormModel({ symbols: singleLoop.symbols.slice(0, 5), controllers: [], links: [] }).structure, "identify",
      "a program without a library PID is set up for identification");
  });

  test("offers as Auto/Manual signal only BOOLs without an I/O address, which Apply can switch", () => {
    const api = setup();
    const model = api.setupFormModel({ ...singleLoop, symbols: [...singleLoop.symbols, { symbol: "P1.Enabled", type: "BOOL" }] });
    assert.deepStrictEqual(model.modes, ["P1.Enabled"], "the controller transfer switches only an unaddressed mode (controller-transfer contract)");
    assert.deepStrictEqual(model.booleans, ["P1.AutoMode", "P1.Enabled"], "a startup flag or a switching state may be addressed");
    const { renderLoopSetupBody } = require("../../controlStudio/loopSetup/loopSetupTemplate");
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main>${renderLoopSetupBody(model, (text: string) => text)}</main>`);
    const offered = Array.from(dom.window.document.querySelectorAll('[data-setup-field="mode"] option') as ArrayLike<{ value: string }>)
      .map(option => option.value).filter(Boolean);
    assert.deepStrictEqual(offered, ["P1.Enabled"]);
  });

  test("asks for the startup flag when the program writes its own PID parameters, and declares it", () => {
    const api = setup();
    const writes = { ...singleLoop, symbols: [...singleLoop.symbols, { symbol: "P1.Initialized", type: "BOOL" }],
      writes: ["P1.AppliedPowerPct", "P1.Initialized", "P1.Parameters.IntegralGain", "P1.Parameters.ProportionalGain"] };
    assert.deepStrictEqual(api.setupFormModel(writes).writes, ["P1.AppliedPowerPct", "P1.Initialized", "P1.Parameters.IntegralGain", "P1.Parameters.ProportionalGain"],
      "a program that writes its own parameter record would overwrite a validated candidate");
    assert.deepStrictEqual(api.setupFormModel(singleLoop).writes, ["P1.SafeProcessValueC", "P1.AppliedPowerPct"],
      "neither the PID output nor the measurement the program computes is a value validation seeds");
    const loop = api.pidLoopDefinition(writes, {
      ...scope, name: "Heater temperature", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 },
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput", startupFlag: "P1.Initialized",
      experimentDurationSeconds: 60,
    });
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.doesNotThrow(() => libraryPidInitialization(loop));
    assert.deepStrictEqual(loop.controller.pidInitialization.applicationStates, [
      { symbol: "P1.Initialized", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1, initial: 1, postScan: "unchanged" }],
      "validation starts with the program's startup already done, so its startup code keeps the candidate");
  });

  test("declares the startup flag on a cascade whose program initializes its controllers", () => {
    const api = setup();
    const initializing = { ...cascade, symbols: [...cascade.symbols, { symbol: "P1.Initialized", type: "BOOL" }],
      writes: ["P1.HeaterPct", "P1.Initialized", "P1.JacketSetpointC", "P1.OuterManual"],
      links: [...cascade.links, { instance: "P1.Outer", method: "Initialize", argument: "State", record: "P1.State", output: null, fields: {} }] };
    const model = api.setupFormModel(initializing);
    assert.deepStrictEqual([model.writes, model.initializes], [["P1.HeaterPct", "P1.Initialized", "P1.JacketSetpointC", "P1.OuterManual"], ["P1.Outer"]],
      "startup code that sets a manual output or initializes a controller replaces the state validation starts from");
    const loop = api.cascadeLoopDefinition(initializing, { ...cascadeTogetherDraft(), startupFlag: "P1.Initialized" });
    assert.deepStrictEqual(loop.controller.project.initialization.states.filter((state: any) => state.symbol === "P1.Initialized"),
      [{ symbol: "P1.Initialized", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1, initial: 1, postScan: "unchanged" }]);
  });

  test("validates the inner loop of a cascade declared as two loop files with the outer PID held in Manual at the target", () => {
    const api = setup();
    // A hand-written two-file cascade: the inner loop's setpoint is the outer controller's manual output.
    const inner = api.pidLoopDefinition(cascade, { ...scope, name: "Vessel temperature · inner loop", instance: "P1.Inner", action: "direct",
      parameterRecord: "P1.InnerParameters", processVariable: { symbol: "P1.JacketC", unit: "degC", minimum: 0, maximum: 90 },
      manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 }, setpoint: { symbol: "P1.OuterManual" },
      mode: { symbol: "P1.InnerAuto", whenTrue: "Auto" }, manualOutput: "P1.InnerManual", experimentDurationSeconds: 3 });
    inner.cascade = { role: "inner", partnerLoopId: "loop:vessel-temperature-outer",
      referenceHold: { modeSymbol: "P1.OuterAuto", manualValue: false, manualOutputSymbol: "P1.OuterManual" } };
    assert.deepStrictEqual(inner.cascade.referenceHold, { modeSymbol: "P1.OuterAuto", manualValue: false, manualOutputSymbol: "P1.OuterManual" },
      "the inner setpoint is the outer output, which only holds still while the outer PID is in Manual");
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const model = { kind: "transfer-function", numerator: [0.2], denominator: [5, 1], delaySeconds: 0, revision: "jacket", source: "provided", validation: "validated",
      evidence: ["fixture"], inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
      outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: 0, maximum: 90 } }], operatingPoint: { inputs: [50], outputs: [40] } };
    const values = { "P1.InnerParameters.ProportionalGain": 1, "P1.InnerParameters.IntegralGain": 0.1, "P1.InnerParameters.DerivativeGain": 0,
      "P1.InnerParameters.ProportionalSetpointWeight": 1, "P1.InnerParameters.DerivativeSetpointWeight": 0, "P1.InnerParameters.Bias": 0,
      "P1.InnerParameters.OutputMinimum": 0, "P1.InnerParameters.OutputMaximum": 100, "P1.InnerParameters.RateMinimum": -1000,
      "P1.InnerParameters.RateMaximum": 1000, "P1.InnerParameters.TrackingTime": 1 };
    const { request } = engineerVerificationPlan(inner, model, { referenceChange: 2, maximumError: 0.5, settlingBand: 0.2, settlingTimeSeconds: 30, durationSeconds: 60 }, values);
    assert.deepStrictEqual([request.initial_values["P1.OuterAuto"], request.initial_values["P1.OuterManual"]], [0, 42]);
  });

  test("prefills every library PID from its own calls, so any of several controllers can be chosen", () => {
    const api = setup();
    const addressedCascade = { ...cascade, links: cascade.links.map(link => link.instance === "P1.Outer" && link.method === "Update"
      ? { ...link, fields: { ...link.fields, ManualOutput: ["P1.OuterManual"] } } : link) };
    assert.deepStrictEqual(api.setupFormModel(addressedCascade).prefills, [
      { instance: "P1.Inner", parameterRecord: "P1.InnerParameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.JacketSetpointC",
        processValue: "P1.JacketC", manualOutput: "P1.InnerManual" },
      { instance: "P1.Outer", parameterRecord: "P1.OuterParameters", manipulatedVariable: "P1.JacketSetpointC", setpoint: "P1.VesselSetpointC",
        processValue: "P1.VesselC", manualOutput: "P1.OuterManual" },
    ], "each controller's record, signals and manual output come from its own Configure and Update calls");
  });

  test("recognises a cascade when one PID's output feeds another's setpoint", () => {
    const api = setup();
    assert.strictEqual(typeof api.cascadePairs, "function");
    assert.deepStrictEqual(api.cascadePairs(api.pidCandidates(cascade)), [{ outer: "P1.Outer", inner: "P1.Inner" }]);
    assert.deepStrictEqual(api.cascadePairs(api.pidCandidates(singleLoop)), []);
  });

  test("leaves the identification duration to the engineer instead of assuming a process speed", () => {
    const api = setup();
    const draft = { ...scope, name: "Heater temperature", instance: "P1.Controller", action: "direct", parameterRecord: "P1.Parameters",
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 },
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
      setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualOutput" };
    assert.throws(() => api.pidLoopDefinition(singleLoop, draft), /identification duration/i);
    assert.throws(() => api.cascadeLoopDefinition(cascade, { ...cascadeTogetherDraft(), experimentDurationSeconds: undefined }), /identification duration/i);
    assert.throws(() => api.identifyLoopDefinition(singleLoop, { ...scope, name: "Plant",
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 } }), /identification duration/i);
    assert.strictEqual(api.identifyLoopDefinition(singleLoop, { ...scope, name: "Plant", experimentDurationSeconds: 120,
      manipulatedVariable: { symbol: "P1.AppliedPowerPct", unit: "%", minimum: 0, maximum: 100 },
      processVariable: { symbol: "P1.TemperatureC", unit: "degC", minimum: 0, maximum: 80 } }).experiment.durationSeconds, 120);
  });
});
