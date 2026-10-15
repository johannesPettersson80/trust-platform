import * as assert from "assert";
import { decodeControlLoopDefinition, validateControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { scope } from "./control-studio-cascade-fixtures";
import { feedforwardDraft, heater, heaterModel, heaterTask } from "./control-studio-feedforward-fixtures";
import { open } from "./control-studio-loop-setup-form-harness";

/** Loaded dynamically so that a missing owner fails at the behaviour assertion. */
function setup(): any {
  try { return require("../../controlStudio/loopSetup/pidLoopSetup"); } catch { return {}; }
}
function design(): any {
  try { return require("../../controlStudio/feedforwardPidDesign"); } catch { return {}; }
}
function feedforwardLoop(withLeadLag = true): any {
  const api = setup();
  assert.strictEqual(typeof api.feedforwardLoopDefinition, "function", "a feedforward loop needs its own generator");
  return api.feedforwardLoopDefinition(heater, feedforwardDraft(withLeadLag));
}
function near(actual: number, expected: number, tolerance: number, label: string): void {
  assert.ok(Math.abs(actual - expected) <= tolerance, `${label}: ${actual}, expected ${expected} ± ${tolerance}`);
}
const values = { "P1.Parameters.ProportionalGain": 5, "P1.Parameters.IntegralGain": 1, "P1.Parameters.DerivativeGain": 0,
  "P1.FeedforwardGain": 0.5, "P1.LeadS": 8, "P1.LagS": 3 };

suite("Control Studio feedforward from a measured disturbance", () => {
  test("writes one loop from the controller output and the measured disturbance to the measurement", () => {
    const loop = feedforwardLoop();
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    const validation: any = validateControlLoopDefinition(loop, scope);
    assert.strictEqual(validation.kind, "accepted", JSON.stringify(validation.reason ?? validation.errors ?? validation));
    assert.deepStrictEqual(loop.channels.inputs.map((input: any) => [input.modelChannelId, input.signalId, input.manualOutputSignal, input.experimentStep]),
      [["P1.HeaterPct", "manipulatedVariable", undefined, 10], ["P1.LoadKw", "measured-disturbance", "P1.LoadKw", 10]],
      "Identify steps the controller output and then the disturbance, each by its own step");
    assert.deepStrictEqual(loop.signals.additional.map((signal: any) => [signal.id, signal.role, signal.symbol]),
      [["measured-disturbance", "disturbance", "P1.LoadKw"]]);
    assert.deepStrictEqual(loop.channels.outputs.map((output: any) => [output.modelChannelId, output.setpointSignalId]), [["P1.WaterC", "setpoint"]]);
    assert.deepStrictEqual([loop.identificationPolicy.maximumOrder, loop.identificationPolicy.maximumNormalizedError], [2, 0.05],
      "each path keeps its own time constant: one shared by paths that differ does not fit within 5 %");
    const project = loop.controller.project;
    assert.deepStrictEqual([project.family, project.seed], ["feedforward", { kind: "feedforward-pid", family: "PI",
      proportionalSymbol: "P1.Parameters.ProportionalGain", integralSymbol: "P1.Parameters.IntegralGain", derivativeSymbol: "P1.Parameters.DerivativeGain",
      gainSymbol: "P1.FeedforwardGain", leadSymbol: "P1.LeadS", lagSymbol: "P1.LagS" }]);
    assert.deepStrictEqual(loop.parameterBinding.parameters.map((parameter: any) => [parameter.symbol, parameter.unit]), [
      ["P1.Parameters.ProportionalGain", "%/degC"], ["P1.Parameters.IntegralGain", "%/(degC*s)"], ["P1.Parameters.DerivativeGain", "%*s/degC"],
      ["P1.FeedforwardGain", "%/kW"], ["P1.LeadS", "s"], ["P1.LagS", "s"]]);
    const initial = (symbol: string) => project.initialization.states.find((state: any) => state.symbol === symbol)?.initial;
    assert.deepStrictEqual(initial("P1.Loop.StateValue.FeedForward"), { source: "scaled-model-input", index: 1, gainSymbol: "P1.FeedforwardGain" },
      "the controller's feedforward state starts at K_ff times the disturbance");
    assert.deepStrictEqual(initial("P1.FilteredLoadKw"), { source: "model-input", index: 1 }, "the lag starts at the disturbance");
    assert.strictEqual(project.objective.requirementId, "target-error");
  });

  test("a feedforward without lead/lag values tunes only its gain", () => {
    const loop = feedforwardLoop(false);
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted");
    assert.deepStrictEqual(loop.parameterBinding.parameters.slice(3).map((parameter: any) => parameter.symbol), ["P1.FeedforwardGain"]);
    assert.strictEqual(loop.controller.project.seed.leadSymbol, undefined);
    assert.ok(!loop.controller.project.initialization.states.some((state: any) => state.symbol === "P1.FilteredLoadKw"));
  });

  test("Identify takes the disturbance's experiment range from Limits like any model input's", () => {
    const loop = feedforwardLoop();
    const { configuredAcquisition } = require("../../controlStudio/acquisitionSettings");
    const configured = configuredAcquisition(loop, { method: "configured", durationSeconds: 120, amplitude: 10,
      inputRange: { minimum: 0, maximum: 100 }, outputRange: { minimum: 0, maximum: 80 },
      inputRanges: { "P1.HeaterPct": { minimum: 0, maximum: 100 }, "P1.LoadKw": { minimum: 0, maximum: 40 } } });
    assert.deepStrictEqual(configured.channels.inputs.map((input: any) => [input.modelChannelId, input.actuator.minimum, input.actuator.maximum]),
      [["P1.HeaterPct", 0, 100], ["P1.LoadKw", 0, 40]]);
  });

  test("the switch reads the disturbance as a measurement and never writes it", () => {
    const loop = feedforwardLoop();
    const { declaredTransferBinding, declaredCommissioningProfile } = require("../../controlStudio/controllerInitialization");
    const initialization = loop.controller.project.initialization;
    const binding = declaredTransferBinding(loop, initialization);
    assert.deepStrictEqual([binding.output.symbol, binding.additional_outputs], ["P1.HeaterPct", undefined]);
    assert.ok(binding.guards.some((guard: any) => guard.symbol === "P1.LoadKw"), "the measured disturbance is guarded as a measurement");
    assert.ok(!binding.states.some((state: any) => state.symbol === "P1.LoadKw"), "Apply never writes the disturbance");
    const tracking = declaredCommissioningProfile(loop, heaterModel, initialization, "Feedforward", values).tracking(loop, binding);
    assert.deepStrictEqual(tracking.initialization_formulas["P1.Loop.StateValue.FeedForward"],
      { kind: "scaled-measurement", process_variable: "P1.LoadKw", gain_symbol: "P1.FeedforwardGain" });
    assert.deepStrictEqual(tracking.initialization_formulas["P1.FilteredLoadKw"], { kind: "model-output", process_variable: "P1.LoadKw" });
    assert.ok(tracking.additional_measurements.includes("P1.LoadKw"));
  });

  test("validation starts the feedforward at the operating point and never checks the disturbance as an output", () => {
    const loop = feedforwardLoop();
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const plan = engineerVerificationPlan(loop, heaterModel, heaterTask, values);
    assert.deepStrictEqual([plan.request.initial_values["P1.Loop.StateValue.FeedForward"], plan.request.initial_values["P1.FilteredLoadKw"],
      plan.request.initial_values["P1.LoadKw"]], [5, 10, 10]);
    assert.ok(!plan.requirements.some((requirement: any) => requirement.channel === "P1.LoadKw"), JSON.stringify(plan.requirements.map((requirement: any) => requirement.id)));
  });

  test("validates a step of the measured disturbance with the setpoint unchanged", () => {
    const loop = feedforwardLoop();
    const { engineerVerificationCampaign } = require("../../controlStudio/engineerVerificationCampaign");
    const campaign = engineerVerificationCampaign(loop, heaterModel, heaterTask, values);
    const disturbance = campaign.find((scenario: any) => scenario.id === "measured-disturbance");
    assert.ok(disturbance, `scenarios: ${campaign.map((scenario: any) => scenario.id)}`);
    assert.deepStrictEqual(disturbance.request.events, [{ at_ms: 0, symbol: "P1.LoadKw", value: 20 }]);
    assert.strictEqual(disturbance.request.initial_values["P1.SetpointC"], 35);
    const deadline = disturbance.requirements.find((requirement: any) => requirement.id === "response-deadline");
    assert.deepStrictEqual([deadline?.target, deadline?.band, deadline?.deadlineSeconds], [35, 0.3, 40]);
  });

  test("designs the PI on the output's path and the feedforward from both paths", () => {
    const loop = feedforwardLoop();
    const { projectControllerSeed } = require("../../controlStudio/projectControllerSeed");
    const seeded = projectControllerSeed(loop, heaterModel, heaterTask, loop.controller.project.seed);
    near(seeded["P1.FeedforwardGain"], 0.5, 0.005, "K_ff = −K_d/K_u");
    near(seeded["P1.LeadS"], 8, 0.08, "T_lead = τ_u");
    near(seeded["P1.LagS"], 3, 0.03, "T_lag = τ_d");
    assert.ok(seeded["P1.Parameters.ProportionalGain"] > 0 && seeded["P1.Parameters.IntegralGain"] > 0, JSON.stringify(seeded));
    assert.strictEqual(seeded["P1.Parameters.DerivativeGain"], 0);
    const api = design();
    assert.strictEqual(typeof api.feedforwardDesign, "function", "Tune shows the feedforward's design");
    const result = api.feedforwardDesign(loop, heaterModel, heaterTask, loop.controller.project.seed);
    near(result.outputPath.gain, 0.2, 0.002, "K_u"); near(result.disturbancePath.gain, -0.1, 0.001, "K_d");
    assert.strictEqual(result.disturbanceStep, 10);
    assert.ok(result.largestDeviation.feedforward < result.largestDeviation.feedback / 10,
      `the feedforward cancels most of the disturbance on the model: ${JSON.stringify(result.largestDeviation)}`);
  });

  test("a disturbance slower in reaching the measurement than the output gets the extra delay as lag", () => {
    const loop = feedforwardLoop();
    // The extraction acts through two 1.5 s lags in series, so its path looks delayed; the heater's path has no delay.
    const model = { ...heaterModel, revision: "slow-load", a: [[-1 / 8, 0, 0], [0, -1 / 1.5, 0], [0, 1 / 1.5, -1 / 1.5]],
      b: [[0.2 / 8, 0], [0, -0.1 / 1.5], [0, 0]], c: [[1, 0, 1]],
      states: [...heaterModel.states, { id: "load-2", unit: "degC", operatingRange: { minimum: -80, maximum: 80 } }] };
    const result = design().feedforwardDesign(loop, model, heaterTask, loop.controller.project.seed);
    assert.ok(result.disturbancePath.delaySeconds > result.outputPath.delaySeconds, JSON.stringify(result));
    near(result.lagSeconds, result.disturbancePath.timeConstantSeconds + result.disturbancePath.delaySeconds - result.outputPath.delaySeconds, 1e-9,
      "T_lag = τ_d + θ_d − θ_u");
  });

  test("a feedforward candidate carries the PI's margins on the output's path", async () => {
    const loop = feedforwardLoop();
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
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({ values: { ...values, "P1.FeedforwardGain": 0 },
      program_revision: "program", binding_revision: "binding", active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any[]) {
      const samples = [0, 1, 2].map(time => ({ timeSeconds: time, sequence: time, values: { "P1.WaterC": { value: 35 + 0.1 * time, quality: "good" } } }));
      return { projectRevision: "project", request, run: { samples, conditions: { modelRevision: "heater", durationSeconds: 2 } },
        evaluation: { status: "Pass", run: { samples, conditions: { modelRevision: "heater", durationSeconds: 2 } }, requirements: [], specification: requirements ?? [] } };
    };
    const feedforward = { outputPath: { gain: 0.2, timeConstantSeconds: 8, delaySeconds: 0 }, disturbancePath: { gain: -0.1, timeConstantSeconds: 3, delaySeconds: 0 },
      gain: 0.5, leadSeconds: 8, lagSeconds: 3, disturbanceStep: 10, largestDeviation: { feedback: 0.61, feedforward: 0.003 } };
    const designed = { ...values, "P1.Parameters.ProportionalGain": 3.75, "P1.Parameters.IntegralGain": 0.469 };
    project.tuneProjectController = async () => ({ status: "provisional", candidate: { applied: false, structure: "feedforward", parameters: designed },
      feedforward, selectedModel: { revision: "heater", provenance: "declared-fitted-validated" }, actualSTVerify: true, scope: "designed from both paths" });
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const session = new EngineerWorkflowSession(loop, () => heaterModel, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "feedforward-margins", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune", projectId: scope.projectId,
        editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "feedforward", applicationMode: "manual", ...heaterTask } }, scope, async (value: any) => { posts.push(value); });
      const candidate = posts.find(post => post.status === "candidate");
      assert.ok(candidate, `Tune produces a candidate: ${JSON.stringify(posts.map(post => post.reason ?? post.status))}`);
      const margins = candidate.robustness;
      // A PI on 0.2/(8s + 1) has no phase crossover: an unbounded gain margin, a finite phase margin and Ms above one.
      assert.ok(margins && margins.gainMarginDb === "unbounded" && typeof margins.phaseMarginDegrees === "number" && margins.peakSensitivity >= 1 - 1e-9,
        `the PI with Kp 3.75 and Ki 0.469 has its margins on the output's path: ${JSON.stringify(margins)}`);
    } finally {
      session.dispose(); search.connectRuntimeSearch = previous.search; parameters.connectRuntimeParameters = previous.parameters;
      verification.RuntimeVerificationClient.prototype.verify = previous.verify; project.tuneProjectController = previous.tune;
    }
  });

  test("Tune shows the engine's feedforward answer: both paths, K_ff, lead and lag, and the two largest deviations", () => {
    const loop = feedforwardLoop();
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      // The engine's FeedforwardAnswer in the names its JSON gives (messages.rs): the page shows these and computes none of them.
      const feedforward = { outputPath: { gain: 0.2, timeConstantSeconds: 8, delaySeconds: 0 }, disturbancePath: { gain: -0.1, timeConstantSeconds: 3, delaySeconds: 0 },
        gain: 0.5, leadSeconds: 8, lagSeconds: 3, disturbanceStep: 10, feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "feedforward", values, design: { feedforward } });
      const document = dom.window.document;
      assert.deepStrictEqual([...document.querySelectorAll("dt")].map((label: any) => label.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Feedforward gain", "Lead time", "Lag time"]);
      const table = document.querySelector("[data-studio-feedforward-design]");
      assert.ok(table, "the design names both path models");
      assert.deepStrictEqual([...table.querySelectorAll("tbody tr")].map((row: any) => [...row.cells].map((cell: any) => cell.textContent)), [
        ["Controller output P1.HeaterPct", "0.2 °C/%", "8 s", "0 s"],
        ["Disturbance P1.LoadKw", "-0.1 °C/kW", "3 s", "0 s"]]);
      assert.strictEqual(document.querySelector("[data-studio-feedforward-values]")?.textContent,
        "Feedforward gain K_ff 0.5 %/kW · lead time T_lead 8 s · lag time T_lag 3 s");
      assert.strictEqual(document.querySelector("[data-studio-feedforward-deviation]")?.textContent,
        "Largest deviation after a 10 kW disturbance step, on the model: 0.61 °C with the PI alone, 0.004 °C with feedforward");
    } finally { (globalThis as any).document = saved; }
  });

  test("Tune names only the gain of a feedforward without lead and lag, as the engine answers it", () => {
    const loop = feedforwardLoop(false);
    const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
    const dom = new JSDOM("<div data-studio-tuning-result></div>");
    const saved = (globalThis as any).document;
    (globalThis as any).document = dom.window.document;
    try {
      const { renderTunedController } = require("../../controlStudio/webview/threePageResults");
      const feedforward = { outputPath: { gain: 0.2, timeConstantSeconds: 8, delaySeconds: 0 }, disturbancePath: { gain: -0.1, timeConstantSeconds: 3, delaySeconds: 0 },
        gain: 0.5, disturbanceStep: 10, feedbackDeviation: 0.61, feedforwardDeviation: 0.004 };
      renderTunedController(dom.window.document.body, loop, { status: "candidate", family: "feedforward", values: { "P1.Parameters.ProportionalGain": 5,
        "P1.Parameters.IntegralGain": 1, "P1.Parameters.DerivativeGain": 0, "P1.FeedforwardGain": 0.5 }, design: { feedforward } });
      assert.strictEqual(dom.window.document.querySelector("[data-studio-feedforward-values]")?.textContent, "Feedforward gain K_ff 0.5 %/kW",
        "without lead and lag the answer gives no lead or lag time");
    } finally { (globalThis as any).document = saved; }
  });
});

/** The heater program as the setup form sees it: one library PID and the feedforward's variables (Specification 35 §17.12). */
const formModel = {
  structure: "pid", addressedReal: ["P1.HeaterPct", "P1.LoadKw", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"], booleans: ["P1.Auto"],
  records: ["P1.Parameters"], writes: [], initializes: [], pids: [{ instance: "P1.Loop", task: { name: "MainTask", intervalMs: 100 } }],
  instances: ["P1"], reals: ["P1.FeedforwardGain", "P1.FilteredLoadKw", "P1.HeaterPct", "P1.LagS", "P1.LeadS", "P1.LoadKw", "P1.ManualPct", "P1.SetpointC", "P1.WaterC"],
  prefill: { instance: "P1.Loop", parameterRecord: "P1.Parameters", manipulatedVariable: "P1.HeaterPct", setpoint: "P1.SetpointC", manualOutput: "P1.ManualPct" },
};

suite("Control Studio feedforward setup", () => {
  test("sets up the PID and the program's feedforward from a measured disturbance in one form", () => {
    const view = open(formModel);
    try {
      const structures = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="structure"] option')).map(option => option.value);
      assert.ok(structures.includes("feedforward"), `a feedforward has its own structure: ${structures}`);
      view.set("structure", "feedforward");
      assert.strictEqual(view.root.querySelector<HTMLElement>('[data-setup-section="feedforward"]')?.hidden, false);
      for (const [field, value] of [["name", "Water temperature"], ["processVariable", "P1.WaterC"], ["processVariable.unit", "degC"],
        ["processVariable.minimum", "0"], ["processVariable.maximum", "80"], ["manipulatedVariable.unit", "%"], ["manipulatedVariable.minimum", "0"],
        ["manipulatedVariable.maximum", "100"], ["mode", "P1.Auto"]]) view.set(field, value);
      const save = view.root.querySelector<HTMLButtonElement>("[data-setup-save]")!, message = view.root.querySelector<HTMLElement>("[data-setup-message]")!;
      assert.strictEqual(save.disabled, true);
      assert.match(message.textContent ?? "", /measured disturbance/i, "the disturbance is the next missing item");
      for (const [field, value] of [["ff.disturbance", "P1.LoadKw"], ["ff.disturbance.unit", "kW"], ["ff.disturbance.minimum", "0"],
        ["ff.disturbance.maximum", "50"], ["ff.disturbanceCommand", "P1.LoadKw"], ["ff.gain", "P1.FeedforwardGain"]]) view.set(field, value);
      // The draft carries the disturbance's identification step, in its unit (the engine's disturbanceStep); the form asks for it (C8).
      assert.strictEqual(save.disabled, true, "the step is still missing");
      assert.match(message.textContent ?? "", /identification step/i, "the step is the next missing item");
      view.set("ff.disturbanceStep", "10");
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      view.set("ff.lead", "P1.LeadS");
      assert.strictEqual(save.disabled, true, "a lead time needs its lag time and the filtered disturbance");
      assert.match(message.textContent ?? "", /lag time/i);
      view.set("ff.lag", "P1.LagS"); view.set("ff.filtered", "P1.FilteredLoadKw");
      assert.strictEqual(save.disabled, false, message.textContent ?? "");
      save.click();
      assert.deepStrictEqual(view.posts, [{ kind: "control-studio.setup.save", draft: {
        structure: "feedforward", name: "Water temperature", instance: "P1.Loop", action: "direct", parameterRecord: "P1.Parameters",
        processVariable: { symbol: "P1.WaterC", unit: "degC", minimum: 0, maximum: 80 },
        manipulatedVariable: { symbol: "P1.HeaterPct", unit: "%", minimum: 0, maximum: 100 },
        setpoint: { symbol: "P1.SetpointC" }, mode: { symbol: "P1.Auto", whenTrue: "Auto" }, manualOutput: "P1.ManualPct",
        disturbance: { symbol: "P1.LoadKw", unit: "kW", minimum: 0, maximum: 50 }, disturbanceCommand: "P1.LoadKw", disturbanceStep: 10,
        feedforward: { gain: "P1.FeedforwardGain", lead: "P1.LeadS", lag: "P1.LagS", filtered: "P1.FilteredLoadKw" },
      } }]);
    } finally { view.close(); }
  });

  test("offers a disturbance the program reads from memory, as a simulated or entered load is", () => {
    // Measurements for the controller are what a simulated plant can write; a disturbance is only read by the program.
    const view = open({ ...formModel, signals: formModel.addressedReal, measurements: ["P1.WaterC"] });
    try {
      const offered = Array.from(view.root.querySelectorAll<HTMLOptionElement>('[data-setup-field="ff.disturbance"] option')).map(option => option.value);
      assert.ok(offered.includes("P1.LoadKw"), `offered: ${offered}`);
    } finally { view.close(); }
  });

  test("saves the feedforward loop from the confirmed draft", async () => {
    const fs = require("fs"), os = require("os"), path = require("path");
    const { saveLoopSetup } = require("../../controlStudio/loopSetup/loopSetupSave");
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-feedforward-setup-"));
    try {
      const { projectId: _project, runtimeId: _runtime, ...draft } = feedforwardDraft();
      const requests = await saveLoopSetup(root, scope, heater, { structure: "feedforward", ...draft });
      assert.deepStrictEqual(requests.map((request: any) => request.loopDefinition.controller.project?.family), ["feedforward"]);
      assert.ok(fs.existsSync(path.join(root, "control-loops", "water-temperature.json")));
    } finally { fs.rmSync(root, { recursive: true, force: true }); }
  });
});
