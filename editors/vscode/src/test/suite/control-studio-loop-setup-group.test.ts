import * as assert from "assert";
import { validateControlLoopDefinition } from "../../controlStudio/loopDefinition";

/** Loaded dynamically so that a missing group generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const PARAMETER_FIELDS = ["ProportionalGain", "IntegralGain", "DerivativeGain", "ProportionalSetpointWeight", "DerivativeSetpointWeight",
  "Bias", "OutputMinimum", "OutputMaximum", "RateMinimum", "RateMaximum", "TrackingTime"];
const STATE_FIELDS = ["Setpoint", "SetpointRate", "ProcessValue", "ProcessValueRate", "FeedForward", "NominalOutput", "SaturatedOutput"];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
const pidSymbols = (instance: string, record: string) => [
  ...PARAMETER_FIELDS.map(field => ({ symbol: `${record}.${field}`, type: "LREAL" })),
  ...STATE_FIELDS.map(field => ({ symbol: `${instance}.StateValue.${field}`, type: "LREAL" })),
];
const zone = (n: number) => [
  { symbol: `P1.Zone${n}C`, type: "LREAL", address: `%IL${8 * (n - 1)}` }, { symbol: `P1.Heater${n}Pct`, type: "LREAL", address: `%QL${8 * (n - 1)}` },
  { symbol: `P1.Zone${n}SetpointC`, type: "LREAL", address: `%ML${8 * (n - 1)}` }, { symbol: `P1.Zone${n}ManualPct`, type: "LREAL", address: `%ML${16 + 8 * (n - 1)}` },
  ...pidSymbols(`P1.Zone${n}`, `P1.Zone${n}Parameters`),
];
/** Two heated zones sharing a wall, one library PID each and one Auto/Manual signal for both. */
const zones = {
  symbols: [...zone(1), ...zone(2), { symbol: "P1.ZonesAuto", type: "BOOL" }, { symbol: "P1.Initialized", type: "BOOL" }],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }, { instance: "P1.Zone1", type: "Control.PID", tasks: TASK },
    { instance: "P1.Zone2", type: "Control.PID", tasks: TASK }],
  links: [1, 2].flatMap(n => [
    { instance: `P1.Zone${n}`, method: "Configure", argument: "Parameters", record: `P1.Zone${n}Parameters`, output: null, fields: {} },
    { instance: `P1.Zone${n}`, method: "Update", argument: "Input", record: `P1.Zone${n}Input`, output: `P1.Heater${n}Pct`,
      fields: { ManualOutput: [`P1.Zone${n}ManualPct`], ProcessValue: [`P1.Zone${n}C`], Setpoint: [`P1.Zone${n}SetpointC`] } },
  ]),
  writes: ["P1.Heater1Pct", "P1.Heater2Pct", "P1.Initialized"],
};
const signal = (symbol: string, unit: string, minimum: number, maximum: number) => ({ symbol, unit, minimum, maximum });
const member = (n: number) => ({ instance: `P1.Zone${n}`, parameterRecord: `P1.Zone${n}Parameters`, processVariable: signal(`P1.Zone${n}C`, "degC", 20, 60),
  manipulatedVariable: signal(`P1.Heater${n}Pct`, "%", 0, 100), setpoint: { symbol: `P1.Zone${n}SetpointC` }, manualOutput: `P1.Zone${n}ManualPct` });
const draft = { ...scope, name: "Zone temperatures", action: "direct", mode: { symbol: "P1.ZonesAuto", whenTrue: "Auto" },
  experimentStep: 10, experimentDurationSeconds: 40, startupFlag: "P1.Initialized", members: [member(1), member(2)] };

suite("Control Studio PID group setup", () => {
  test("writes one group loop whose members are tuned together from the group model", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidGroupLoopDefinition, "function", "interacting PIDs need a group structure, not two separate loops");
    const loop = api.pidGroupLoopDefinition(zones, draft);
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    assert.doesNotThrow(() => controllerAdapter(loop), "the existing PID group adapter accepts the generated contract");
    assert.deepStrictEqual([loop.controller.project.seed, loop.controller.project.axes], [{ kind: "decentralized-pid", family: "PI" }, []],
      "each member is designed from the group model (Specification 35 §17.9), not from a declared grid");
    assert.deepStrictEqual(loop.controller.group.members.map((entry: any) => [entry.instance, entry.inputChannelId, entry.outputChannelId, entry.modeSymbol]),
      [["P1.Zone1", "P1.Heater1Pct", "P1.Zone1C", "P1.ZonesAuto"], ["P1.Zone2", "P1.Heater2Pct", "P1.Zone2C", "P1.ZonesAuto"]]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => parameter.symbol), [1, 2].flatMap(n =>
      ["ProportionalGain", "IntegralGain", "DerivativeGain"].map(field => `P1.Zone${n}Parameters.${field}`)),
    "only the gains are bound, so validation stays within its 32 recorded channels");
    assert.strictEqual(loop.controller.project.initialization.states.length, 15, "seven retained states per member and the startup flag");
    assert.deepStrictEqual([loop.channels.inputs[1].manualOutputSignal, loop.experiment.amplitude, loop.experiment.durationSeconds],
      ["P1.Zone2ManualPct", 10, 40]);
  });

  test("names the group's model channels by their signals, as identification names the identified model's channels", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidGroupLoopDefinition, "function");
    const loop = api.pidGroupLoopDefinition(zones, draft);
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: 0, maximum: 100 } });
    // What Identify returns for this loop: channels named after the heater and zone signals.
    const identified = { kind: "continuous-state-space", a: [[-0.15, 0.05], [0.05, -0.15]], b: [[0.03, 0], [0, 0.03]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]],
      states: [channel("x1", "degC"), channel("x2", "degC")], inputs: [channel("P1.Heater1Pct", "%"), channel("P1.Heater2Pct", "%")],
      outputs: [channel("P1.Zone1C", "degC"), channel("P1.Zone2C", "degC")], delaySeconds: 0, revision: "identified", source: "fitted",
      validation: "validated", evidence: ["recording"], operatingPoint: { inputs: [20, 20], outputs: [26, 26] } };
    const { connectedModelChannels } = require("../../controlStudio/loopChannels");
    assert.doesNotThrow(() => connectedModelChannels(loop, identified), "the identified model maps onto the group's channels");
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const values = projectControllerSeed(loop, identified, { referenceChange: 0.4, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 40,
      durationSeconds: 80, tuningObjective: "balanced" }, loop.controller.project.seed, "PI");
    assert.deepStrictEqual(Object.keys(values).sort(), [1, 2].flatMap(n => ["DerivativeGain", "IntegralGain", "ProportionalGain"].map(field => `P1.Zone${n}Parameters.${field}`)),
      "each member's gains come from the decentralized design on the identified model");
  });

  test("gives each member's input its own identification step, which Identify uses for that input", async () => {
    const api = setup();
    const loop = api.pidGroupLoopDefinition(zones, { ...draft, experimentStep: undefined,
      members: [{ ...member(1), experimentStep: 10 }, { ...member(2), experimentStep: 5 }] });
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual(loop.channels.inputs.map((input: any) => input.experimentStep), [10, 5], "Specification 35 §17.9");
    const { connectedIdentificationChannels } = require("../../controlStudio/connectedIdentificationChannels");
    const request = async (operation: string) => {
      assert.strictEqual(operation, "io.list");
      return { memory: [{ name: "P1.Zone1ManualPct", address: "%ML16" }, { name: "P1.Zone2ManualPct", address: "%ML24" }] };
    };
    const channels = await connectedIdentificationChannels(loop, request);
    assert.deepStrictEqual(channels.map((channel: any) => [channel.loop.signals.manipulatedVariable.symbol, channel.loop.experiment.amplitude]),
      [["P1.Heater1Pct", 10], ["P1.Heater2Pct", 5]]);
  });

  test("refuses members that do not belong to one compiled program", () => {
    const api = setup();
    assert.strictEqual(typeof api.pidGroupLoopDefinition, "function");
    assert.throws(() => api.pidGroupLoopDefinition(zones, { ...draft, members: [member(1)] }), /at least two/i);
    assert.throws(() => api.pidGroupLoopDefinition(zones, { ...draft, members: [member(1), { ...member(2), instance: "P1.Zone1" }] }), /twice|once/i);
  });
});

suite("Control Studio Tune prediction for PID groups", () => {
  test("predicts every member's response with the current and the tuned gains by running the actual ST on the group model", async () => {
    const loop = setup().pidGroupLoopDefinition(zones, draft);
    const channel = (id: string, unit: string, minimum: number, maximum: number) => ({ id, unit, operatingRange: { minimum, maximum } });
    const model: any = { kind: "continuous-state-space", a: [[-0.15, 0.05], [0.05, -0.15]], b: [[0.03, 0], [0, 0.03]], c: [[1, 0], [0, 1]], d: [[0, 0], [0, 0]],
      states: [channel("x1", "degC", 20, 60), channel("x2", "degC", 20, 60)], inputs: [channel("P1.Heater1Pct", "%", 0, 100), channel("P1.Heater2Pct", "%", 0, 100)],
      outputs: [channel("P1.Zone1C", "degC", 20, 60), channel("P1.Zone2C", "degC", 20, 60)], delaySeconds: 0, revision: "zones", source: "fitted",
      validation: "validated", evidence: ["recording"], operatingPoint: { inputs: [20, 20], outputs: [26, 26] } };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters, oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const current = Object.fromEntries(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.symbol.endsWith("ProportionalGain") ? 2 : 0]));
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected search operation ${operation}`);
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => ({ values: current, program_revision: "program", binding_revision: "binding", active_revision: "active", status: "applied",
        previous_revision: null, previous_values: null, reason: null }) } });
    const requests: any[] = [];
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any[]) {
      requests.push(request);
      const samples = [0, 1, 2].map(time => ({ timeSeconds: time, sequence: time, values: {
        "P1.Zone1C": { value: 26 + 0.1 * time, quality: "good" }, "P1.Zone2C": { value: 26, quality: "good" } } }));
      return { projectRevision: "project", request, run: { samples, conditions: { modelRevision: "zones", durationSeconds: 2 } },
        evaluation: { status: "Pass", run: { samples, conditions: { modelRevision: "zones", durationSeconds: 2 } }, requirements: [], specification: requirements ?? [] } };
    };
    // The decentralized design has its own tests; this one follows its result to the prediction.
    const project = require("../../controlStudio/projectController");
    const oldTune = project.tuneProjectController;
    const tuned = Object.fromEntries([1, 2].flatMap(n => [[`P1.Zone${n}Parameters.ProportionalGain`, 3.75], [`P1.Zone${n}Parameters.IntegralGain`, 0.47],
      [`P1.Zone${n}Parameters.DerivativeGain`, 0]]));
    project.tuneProjectController = async () => ({ status: "provisional", candidate: { applied: false, structure: "PI", parameters: tuned },
      selectedModel: { revision: "zones", provenance: "declared-fitted-validated" }, actualSTVerify: true, scope: "decentralized" });
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "group-prediction", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
        editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 0.4, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 40, durationSeconds: 80, applicationMode: "auto",
          channelRequirements: Object.fromEntries(["P1.Zone1C", "P1.Zone2C"].map(id => [id,
            { referenceChange: 0.4, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 40 }])) } },
      scope, async (value: any) => { posts.push(value); });
      const candidate = posts.find(post => post.status === "candidate");
      assert.ok(candidate, `Tune produces a group candidate: ${JSON.stringify(posts.map(post => post.reason ?? post.status))}`);
      assert.ok(candidate.modelPrediction?.candidate?.run?.samples?.length && candidate.modelPrediction?.baseline?.run?.samples?.length,
        "the candidate carries the predicted responses of the tuned and the current gains (Specification 35 §9)");
      const gain = (request: any) => request.parameter_overrides.find((parameter: any) => parameter.symbol === "P1.Zone1Parameters.ProportionalGain")?.value;
      const predicted = requests.slice(-2).map(gain);
      assert.deepStrictEqual(predicted, [2, 3.75], "the current and the tuned gains run on the same model");
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify; project.tuneProjectController = oldTune;
    }
  });
});
