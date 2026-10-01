import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { open } from "./control-studio-loop-setup-form-harness";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";

/** Loaded dynamically so that a missing generator fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}

const TASK = [{ name: "MainTask", intervalMs: 100 }];
const scope = { projectId: "project:setup", runtimeId: "runtime-1" };
/** An oven whose program computes its own PI: HeaterPct := Kp * (SetpointC - OvenC) + Integral. */
const oven = {
  symbols: [
    { symbol: "P1.OvenC", type: "LREAL", address: "%IL0" }, { symbol: "P1.HeaterPct", type: "LREAL", address: "%QL0" },
    { symbol: "P1.SetpointC", type: "LREAL", address: "%ML0" }, { symbol: "P1.ManualPct", type: "LREAL", address: "%ML8" },
    { symbol: "P1.AutoMode", type: "BOOL" }, { symbol: "P1.Started", type: "BOOL" },
    { symbol: "P1.Kp", type: "LREAL" }, { symbol: "P1.Ki", type: "REAL" }, { symbol: "P1.Integral", type: "LREAL" },
    { symbol: "P1.Kd", type: "LREAL" }, { symbol: "P1.PreviousErrorC", type: "LREAL" }, { symbol: "P1.PreviousOvenC", type: "LREAL" },
  ],
  controllers: [{ instance: "P1", type: "Main", tasks: TASK }],
  links: [],
  writes: ["P1.HeaterPct", "P1.Integral", "P1.Started"],
};
const draft = { ...scope, name: "Oven", action: "direct", law: "PI", instance: "P1",
  processVariable: { symbol: "P1.OvenC", unit: "degC", minimum: 0, maximum: 300 },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
  setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  proportionalGain: { symbol: "P1.Kp", minimum: 0, maximum: 20 }, integralGain: { symbol: "P1.Ki", minimum: 0, maximum: 2 },
  integralTerm: "P1.Integral", experimentStep: 10, experimentDurationSeconds: 600, startupFlag: "P1.Started" };
const task = { referenceChange: 10, maximumError: 1, settlingBand: 0.5, settlingTimeSeconds: 400, durationSeconds: 800 };
const model = { kind: "transfer-function", numerator: [2], denominator: [60, 1], delaySeconds: 5, revision: "oven", source: "fitted",
  validation: "validated", evidence: ["recording"], inputs: [{ id: "u", unit: "%", operatingRange: { minimum: 0, maximum: 100 } }],
  outputs: [{ id: "y", unit: "degC", operatingRange: { minimum: 0, maximum: 300 } }], operatingPoint: { inputs: [20], outputs: [60] } };

suite("Control Studio own P or PI law setup", () => {
  test("writes the program's own PI as a project controller whose seed names its gain variables", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    const loop = api.ownLawLoopDefinition(oven, draft);
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted", "the loop contract accepts a PI seed without a derivative gain");
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual([loop.controller.instance, loop.controller.type, loop.controller.design, loop.controller.law],
      ["P1", "Main", "project", undefined], "the law belongs to the program, not to a library PID");
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.convention, project.seed],
      ["PI", "project-owned-native-controller", { kind: "parallel-pid", family: "PI", proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki" }]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.iec_type, parameter.unit, parameter.minimum, parameter.maximum]),
      [["P1.Kp", "LREAL", "%/degC", 0, 20], ["P1.Ki", "REAL", "%/(degC*s)", 0, 2]]);
    assert.deepStrictEqual(project.initialization.states.map((state: any) => [state.symbol, state.iec_type, state.unit, state.initial]),
      [["P1.Integral", "LREAL", "%", { source: "pid-integral", index: 0, inputIndex: 0, gainSymbol: "P1.Kp" }], ["P1.Started", "BOOL", "1", 1]],
      "the integral term starts at the output of the switch to Auto; the startup flag starts TRUE");
    assert.deepStrictEqual([loop.signals.setpoint.symbol, loop.signals.processVariable.symbol, loop.signals.manipulatedVariable.symbol],
      ["P1.SetpointC", "P1.OvenC", "P1.HeaterPct"]);
    assert.deepStrictEqual([loop.controller.modeSignal, loop.controller.manualOutputSignal, loop.parameterBinding.task_name],
      ["P1.AutoMode", "P1.ManualPct", "MainTask"]);
    assert.deepStrictEqual([loop.experiment.kind, loop.experiment.amplitude, loop.experiment.durationSeconds], ["mv_step", 10, 600]);
  });

  test("offers Tune only the families whose gains the program has and designs them from the model", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    const { projectControllerFamilies } = require("../../controlStudio/projectController");
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const pi = api.ownLawLoopDefinition(oven, draft);
    assert.deepStrictEqual(projectControllerFamilies(pi.controller.project), ["P", "PI"]);
    const designed = projectControllerSeed(pi, model, task, pi.controller.project.seed, "PI");
    assert.deepStrictEqual(Object.keys(designed).sort(), ["P1.Ki", "P1.Kp"], "a PI design writes exactly the program's two gains");
    assert.ok(designed["P1.Kp"] > 0 && designed["P1.Ki"] > 0, JSON.stringify(designed));
    assert.throws(() => projectControllerSeed(pi, model, task, pi.controller.project.seed, "PID"), /no derivative gain/);

    const p = api.ownLawLoopDefinition(oven, { ...draft, name: "Oven P", law: "P", integralGain: undefined, integralTerm: undefined });
    assert.strictEqual(validateControlLoopDefinition(p, scope).kind, "accepted");
    assert.deepStrictEqual(p.controller.project.seed, { kind: "parallel-pid", family: "P", proportionalSymbol: "P1.Kp" });
    assert.deepStrictEqual(p.parameterBinding.parameters.map((parameter: any) => parameter.symbol), ["P1.Kp"]);
    assert.deepStrictEqual(p.controller.project.initialization.states.map((state: any) => state.symbol), ["P1.Started"]);
    assert.deepStrictEqual(projectControllerFamilies(p.controller.project), ["P"]);
    // Studio's P design, for a library PID too, takes a delay-free first-order model and requirements that allow P's steady offset.
    assert.deepStrictEqual(Object.keys(projectControllerSeed(p, { ...model, delaySeconds: 0 }, { ...task, maximumError: 3 }, p.controller.project.seed, "P")),
      ["P1.Kp"]);
  });

  test("designs the program's PI to the requirements, as Automatic does for Control.PID", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const { engineerPrediction } = require("../../controlStudio/engineerPrediction");
    const loop = api.ownLawLoopDefinition(oven, draft);
    // 1.5 °C/%, 4 s lag and 0.5 s delay: the More robust PI settles too slowly for a 12 s deadline.
    const fast = { ...model, numerator: [1.5], denominator: [4, 1], delaySeconds: 0.5 };
    const tight = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.05, settlingTimeSeconds: 12, durationSeconds: 24, tuningObjective: "robust" };
    const gains = projectControllerSeed(loop, fast, tight, loop.controller.project.seed, "PI");
    const prediction = engineerPrediction(fast, loop, { proportionalGain: gains["P1.Kp"], integralGain: gains["P1.Ki"] }, tight);
    const target = fast.operatingPoint.outputs[0] + tight.referenceChange;
    const error = Math.max(...prediction.timeResponse.filter((point: any) => point.timeSeconds >= 12 - 1e-9).map((point: any) => Math.abs(point.candidatePv - target)));
    assert.ok(error <= tight.maximumError, `the designed PI is ${error} °C from the target after 12 s; 0.1 °C is allowed (Kp ${gains["P1.Kp"]}, Ki ${gains["P1.Ki"]})`);
  });

  test("names as its tuning objective a requirement the loop's verification campaign measures", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    const { engineerVerificationCampaign } = require("../../controlStudio/engineerVerificationCampaign");
    const loop = api.ownLawLoopDefinition(oven, draft);
    // Validation runs at most 600 s.
    const campaign = engineerVerificationCampaign(loop, model, { ...task, settlingTimeSeconds: 200, durationSeconds: 400 }, { "P1.Kp": 1, "P1.Ki": 0.5 });
    const measured = campaign.flatMap((scenario: any) => scenario.requirements.map((requirement: any) => requirement.id));
    assert.ok(measured.includes(loop.controller.project.objective.requirementId),
      `Tune ranks candidates by ${loop.controller.project.objective.requirementId}, which the campaign must measure: ${measured.join(", ")}`);
  });

  test("writes the program's own PID with the previous value its derivative is taken from", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    const { projectControllerFamilies } = require("../../controlStudio/projectController");
    const pid = { ...draft, name: "Oven PID", law: "PID", derivativeGain: { symbol: "P1.Kd", minimum: 0, maximum: 50 },
      derivativeState: "P1.PreviousErrorC", derivativeOn: "error" };
    const loop = api.ownLawLoopDefinition(oven, pid);
    assert.strictEqual(validateControlLoopDefinition(loop, scope).kind, "accepted");
    assert.deepStrictEqual(loop.controller.project.seed,
      { kind: "parallel-pid", family: "PID", proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki", derivativeSymbol: "P1.Kd" });
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]),
      [["P1.Kp", "%/degC"], ["P1.Ki", "%/(degC*s)"], ["P1.Kd", "%*s/degC"]]);
    const state = (symbol: string) => loop.controller.project.initialization.states.find((entry: any) => entry.symbol === symbol);
    assert.deepStrictEqual([state("P1.PreviousErrorC").unit, state("P1.PreviousErrorC").initial], ["degC", { source: "weighted-error", index: 0, weight: 1 }],
      "the previous error starts at the error of the switch, zero with the setpoint at the measurement");
    assert.deepStrictEqual(projectControllerFamilies(loop.controller.project), ["P", "PI", "PD", "PID"]);
    const onMeasurement = api.ownLawLoopDefinition(oven, { ...pid, name: "Oven PID measured", derivativeState: "P1.PreviousOvenC", derivativeOn: "measurement" });
    const measured = onMeasurement.controller.project.initialization.states.find((entry: any) => entry.symbol === "P1.PreviousOvenC");
    assert.deepStrictEqual([measured.initial, measured.minimum, measured.maximum], [{ source: "model-output", index: 0 }, 0, 300],
      "the previous measurement starts at the measurement of the switch");
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...pid, derivativeGain: undefined }), /derivative gain/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...pid, derivativeState: undefined }), /previous error or measurement/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...pid, derivativeState: "P1.Integral" }), /different variable/);
  });

  test("allows the Auto entry of the program's PID the measurement noise its gains amplify", () => {
    const { declaredInitializationPlan } = require("../../controlStudio/controllerInitialization");
    const { engineerRequirements } = require("../../controlStudio/engineerVerificationPlan");
    const loop = setup().ownLawLoopDefinition(oven, { ...draft, name: "Oven PID noisy", law: "PID", derivativeGain: { symbol: "P1.Kd", minimum: 0, maximum: 50 },
      derivativeState: "P1.PreviousOvenC", derivativeOn: "measurement" });
    const noise = 0.02, cycle = 0.1, values = { "P1.Kp": 2, "P1.Ki": 0.125, "P1.Kd": 5 };
    const plan = declaredInitializationPlan(loop, model, engineerRequirements({ ...task, settlingTimeSeconds: 250, durationSeconds: 500 }), values,
      loop.controller.project.initialization, noise);
    const continuity = plan.requirements.find((requirement: any) => requirement.id === "transfer-continuity");
    const allowance = 2 * 2 * noise + 0.125 * cycle * noise + 4 * 5 * noise / cycle;
    assert.ok(Math.abs((continuity.maximum - continuity.minimum) / 2 - allowance) < 1e-9,
      `the first output may move by what the noise does through Kp, Ki and Kd (${allowance}), got ${JSON.stringify(continuity)}`);
  });

  test("refuses a PI without its integral term and gains that are not distinct REAL variables", () => {
    const api = setup();
    assert.strictEqual(typeof api.ownLawLoopDefinition, "function", "a law the program computes itself needs its own setup structure");
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...draft, integralTerm: undefined }), /integral term/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...draft, integralGain: undefined }), /integral gain/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...draft, integralGain: { ...draft.integralGain, symbol: "P1.Kp" } }), /different variable/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...draft, proportionalGain: { ...draft.proportionalGain, symbol: "P1.AutoMode" } }), /REAL or LREAL/);
    assert.throws(() => api.ownLawLoopDefinition(oven, { ...draft, proportionalGain: { ...draft.proportionalGain, minimum: 5, maximum: 1 } }), /range/);
  });
});

/** The oven's compiled facts as the setup form sees them: the program computes its own PI. */
const ownLawModel = { structure: "identify", pids: [], prefills: [], records: [], initializes: [], instances: ["P1"],
  addressedReal: ["P1.HeaterPct", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"], booleans: ["P1.AutoMode", "P1.Started"],
  reals: ["P1.HeaterPct", "P1.Integral", "P1.Ki", "P1.Kp", "P1.ManualPct", "P1.OvenC", "P1.SetpointC"], writes: ["P1.HeaterPct", "P1.Integral"] };
const ownLawFill: [string, string][] = [["structure", "own-law"], ["name", "Oven"], ["action", "direct"], ["own.instance", "P1"], ["own.law", "PI"],
  ["own.processVariable", "P1.OvenC"], ["own.processVariable.unit", "degC"], ["own.processVariable.minimum", "0"], ["own.processVariable.maximum", "300"],
  ["own.setpoint", "P1.SetpointC"], ["own.manipulatedVariable", "P1.HeaterPct"], ["own.manipulatedVariable.unit", "%"],
  ["own.manipulatedVariable.minimum", "0"], ["own.manipulatedVariable.maximum", "100"], ["own.mode", "P1.AutoMode"], ["own.manualOutput", "P1.ManualPct"],
  ["own.proportionalGain", "P1.Kp"], ["own.proportionalGain.minimum", "0"], ["own.proportionalGain.maximum", "20"],
  ["own.integralGain", "P1.Ki"], ["own.integralGain.minimum", "0"], ["own.integralGain.maximum", "2"]];
const ownLawDraft = { structure: "own-law", name: "Oven", action: "direct", law: "PI", instance: "P1",
  processVariable: { symbol: "P1.OvenC", unit: "degC", minimum: 0, maximum: 300 }, setpoint: { symbol: "P1.SetpointC" },
  manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
  mode: { symbol: "P1.AutoMode", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
  proportionalGain: { symbol: "P1.Kp", minimum: 0, maximum: 20 }, integralGain: { symbol: "P1.Ki", minimum: 0, maximum: 2 },
  integralTerm: "P1.Integral", experimentStep: 10, experimentDurationSeconds: 600 };

suite("Control Studio own P or PI law on Tune", () => {
  test("a candidate of the program's PI carries the margins of its designed gains", async () => {
    const loop = setup().ownLawLoopDefinition(oven, draft);
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const project = require("../../controlStudio/projectController");
    const previous = { search: search.connectRuntimeSearch, parameters: parameters.connectRuntimeParameters,
      verify: verification.RuntimeVerificationClient.prototype.verify, tune: project.tuneProjectController };
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected operation ${operation}`);
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({ values: { "P1.Kp": 1, "P1.Ki": 0.5 },
      program_revision: "program", binding_revision: "binding", active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any[]) {
      const samples = [0, 1, 2].map(time => ({ timeSeconds: time, sequence: time, values: { "P1.OvenC": { value: 60 + 0.1 * time, quality: "good" } } }));
      return { projectRevision: "project", request, run: { samples, conditions: { modelRevision: "oven", durationSeconds: 2 } },
        evaluation: { status: "Pass", run: { samples, conditions: { modelRevision: "oven", durationSeconds: 2 } }, requirements: [], specification: requirements ?? [] } };
    };
    project.tuneProjectController = async () => ({ status: "provisional", candidate: { applied: false, structure: "PI", parameters: { "P1.Kp": 2, "P1.Ki": 0.5 } },
      selectedModel: { revision: "oven", provenance: "declared-fitted-validated" }, actualSTVerify: true, scope: "designed to the requirements" });
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "own-margins", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
        editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "PI", applicationMode: "manual", referenceChange: 10, maximumError: 1, settlingBand: 0.5, settlingTimeSeconds: 200, durationSeconds: 400 } },
      scope, async (value: any) => { posts.push(value); });
      const candidate = posts.find(post => post.status === "candidate");
      assert.ok(candidate, `Tune produces a candidate: ${JSON.stringify(posts.map(post => post.reason ?? post.status))}`);
      const margins = candidate.robustness;
      assert.ok(margins && typeof margins.phaseMarginDegrees === "number" && typeof margins.delayMarginSeconds === "number" && margins.peakSensitivity >= 1,
        `the program's PI with Kp 2 and Ki 0.5 has its margins on the oven model: ${JSON.stringify(margins)}`);
    } finally {
      session.dispose(); search.connectRuntimeSearch = previous.search; parameters.connectRuntimeParameters = previous.parameters;
      verification.RuntimeVerificationClient.prototype.verify = previous.verify; project.tuneProjectController = previous.tune;
    }
  });
});

suite("Control Studio own P or PI law Tune result", () => {
  test("names the program's gains as Control.PID's are named", () => {
    const { JSDOM } = require("jsdom");
    const dom = new JSDOM(`<main><div data-studio-tuning-result hidden></div></main>`);
    const previous = (globalThis as any).document; (globalThis as any).document = dom.window.document;
    try {
      const root = dom.window.document.querySelector("main");
      renderTunedController(root, setup().ownLawLoopDefinition(oven, draft), { status: "candidate", family: "PI", values: { "P1.Kp": 0.8438, "P1.Ki": 0.2109 } });
      assert.deepStrictEqual(Array.from(root.querySelectorAll("dt")).map((term: any) => term.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)"]);
    } finally { (globalThis as any).document = previous; dom.window.close(); }
  });
});

suite("Control Studio own P or PI law setup form", () => {
  test("sets up a P or PI law the program computes itself from its own gain variables", () => {
    const view = open(ownLawModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("own-law"), `a law the program computes itself has its own structure: ${structures}`);
      for (const [field, value] of ownLawFill) view.set(field, value);
      const message = () => view.root.querySelector("[data-setup-message]")!.textContent ?? "";
      assert.match(message(), /integral term/i, "a PI law names the variable holding its integral term");
      view.set("own.integralTerm", "P1.Integral");
      assert.match(message(), /identification duration/i);
      assert.strictEqual(view.root.querySelector<HTMLInputElement>('[data-setup-field="own.experimentStep"]')!.value, "10",
        "the identification step defaults to a tenth of the output range");
      view.set("own.experimentDuration", "600");
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!;
      assert.strictEqual(save.disabled, false, message());
      save.click();
      assert.deepStrictEqual(view.posts[0].draft, ownLawDraft);
    } finally { view.close(); }
  });

  test("asks a PID law for its derivative gain, the previous value it differentiates and whether that is the error", () => {
    const view = open({ ...ownLawModel, reals: [...ownLawModel.reals, "P1.Kd", "P1.PreviousErrorC"].sort() });
    try {
      const laws = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="own.law"] option')).map(option => option.value);
      assert.ok(laws.includes("PID") && view.root.querySelector("[data-own-derivative]"), `the law choice offers PID with its derivative fields: ${laws}`);
      for (const [field, value] of [...ownLawFill, ["own.law", "PID"], ["own.integralTerm", "P1.Integral"], ["own.experimentDuration", "600"]]) view.set(field, value);
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, false, "a PID law shows its derivative fields");
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /derivative gain/i);
      for (const [field, value] of [["own.derivativeGain", "P1.Kd"], ["own.derivativeGain.minimum", "0"], ["own.derivativeGain.maximum", "50"],
        ["own.derivativeState", "P1.PreviousErrorC"], ["own.derivativeOn", "error"]]) view.set(field, value);
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...ownLawDraft, law: "PID", derivativeGain: { symbol: "P1.Kd", minimum: 0, maximum: 50 },
        derivativeState: "P1.PreviousErrorC", derivativeOn: "error" });
      view.set("own.law", "PI");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-derivative]")!.hidden, true);
    } finally { view.close(); }
  });

  test("asks a P law for neither an integral gain nor an integral term", () => {
    const view = open(ownLawModel);
    try {
      for (const [field, value] of ownLawFill.filter(([field]) => !field.startsWith("own.integralGain"))) view.set(field, value);
      view.set("own.law", "P");
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-own-integral]")!.hidden, true, "a P law has no integral fields");
      view.set("own.experimentDuration", "600");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      const { integralGain: _gain, integralTerm: _term, ...pDraft } = ownLawDraft;
      assert.deepStrictEqual(view.posts[0].draft, { ...pDraft, law: "P" });
    } finally { view.close(); }
  });

  test("requires the startup flag when the program writes a gain itself", () => {
    const view = open({ ...ownLawModel, writes: [...ownLawModel.writes, "P1.Kp"] });
    try {
      for (const [field, value] of [...ownLawFill, ["own.integralTerm", "P1.Integral"], ["own.experimentDuration", "600"]]) view.set(field, value);
      assert.match(view.root.querySelector("[data-setup-message]")!.textContent ?? "", /writes P1\.Kp.*startup/i,
        "a program that sets its own gain would replace the candidate being validated");
      view.set("startupFlag", "P1.Started");
      view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.click();
      assert.deepStrictEqual(view.posts[0].draft, { ...ownLawDraft, startupFlag: "P1.Started" });
    } finally { view.close(); }
  });

  test("the integral term the program updates every scan needs no startup flag", () => {
    const view = open(ownLawModel);
    try {
      for (const [field, value] of [...ownLawFill, ["own.integralTerm", "P1.Integral"], ["own.experimentDuration", "600"]]) view.set(field, value);
      assert.strictEqual(view.root.querySelector<HTMLElement>("[data-setup-startup]")!.hidden, true);
      assert.strictEqual(view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!.disabled, false);
    } finally { view.close(); }
  });
});

suite("Control Studio own P or PI law saving", () => {
  test("saves the program's own PI as one loop file and opens it", async () => {
    const { saveLoopSetup } = require("../../controlStudio/loopSetup/loopSetupSave");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-own-law-"));
    try {
      const { projectId: _project, runtimeId: _runtime, ...unscoped } = draft;
      const requests = await saveLoopSetup(root, scope, oven, { structure: "own-law", ...unscoped });
      assert.deepStrictEqual(fs.readdirSync(path.join(root, "control-loops")), ["oven.json"]);
      const saved = JSON.parse(fs.readFileSync(path.join(root, "control-loops", "oven.json"), "utf8"));
      assert.strictEqual(validateControlLoopDefinition(saved, scope).kind, "accepted");
      assert.deepStrictEqual(saved.controller.project.seed, { kind: "parallel-pid", family: "PI", proportionalSymbol: "P1.Kp", integralSymbol: "P1.Ki" });
      assert.deepStrictEqual(requests.map((request: any) => request.loopDefinition.id), ["loop:oven"]);
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
