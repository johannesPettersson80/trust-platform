import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

function libraryLoop(): any {
  const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../src/test/fixtures/control-studio-pid/control-loop.json"), "utf8"));
  loop.controller.instance = "Main.Controller";
  const members = [
    ["Setpoint", "m", { source: "model-output", index: 0 }],
    ["SetpointRate", "m/s", 0],
    ["ProcessValue", "m", { source: "model-output", index: 0 }],
    ["ProcessValueRate", "m/s", 0],
    ["FeedForward", "V", 0],
    ["NominalOutput", "V", { source: "model-input", index: 0 }],
    ["SaturatedOutput", "V", { source: "model-input", index: 0 }],
  ];
  loop.controller.pidInitialization = { kind: "control-pid-v1", maximumSnapshotAgeMs: 100,
    states: members.map(([name, unit, initial]) => ({ symbol: `Main.Controller.StateValue.${name}`,
      iec_type: "LREAL", unit, minimum: -10000, maximum: 10000, initial, postScan: "bounded" })) };
  return loop;
}
const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -20, maximum: 20 } });
const model = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
  revision: "library-pid-model", source: "provided", validation: "validated", evidence: ["independent-run"],
  inputs: [channel("u", "V")], outputs: [channel("y", "m")], operatingPoint: { inputs: [3], outputs: [4] } };
const task = { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 };

suite("Control Studio actual library PID state adapter", () => {
  test("imports compiled nested PID fields through the public adapter setup", async () => {
    const vscode = require("vscode");
    const os = require("os");
    const { configureControllerAdapter } = require("../../controlStudio/configureControllerAdapter");
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const sourceLoop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../../../examples/control-pid-commissioning/control-loop.json"), "utf8"));
    const loop = structuredClone(sourceLoop);
    const mapping = { controller: loop.controller, parameterBinding: loop.parameterBinding };
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "trust-pid-adapter-"));
    const filename = path.join(directory, "adapter.json");
    fs.writeFileSync(filename, JSON.stringify(mapping));
    const originalDialog = vscode.window.showOpenDialog;
    const catalog = [
      ...loop.parameterBinding.parameters.map((value: any) => ({ symbol: value.symbol, type: value.iec_type })),
      { symbol: loop.parameterBinding.mode.symbol, type: "BOOL" },
      { symbol: loop.controller.manualOutputSignal, type: loop.controller.manualOutputType },
      ...controllerAdapter(loop).transferBinding().states.map((value: any) => ({ symbol: value.symbol, type: value.iec_type })),
    ];
    vscode.window.showOpenDialog = async () => [vscode.Uri.file(filename)];
    try {
      assert.strictEqual(await configureControllerAdapter(loop, catalog, loop.parameterBinding.task_name), true);
      assert.strictEqual(loop.controller.pidInitialization.kind, "control-pid-v1");
      assert.strictEqual(controllerAdapter(loop).transferBinding().states.length, 10);
      const missingState = catalog.filter((value: any) => !value.symbol.endsWith(".StateValue.NominalOutput"));
      await assert.rejects(configureControllerAdapter(structuredClone(sourceLoop), missingState, loop.parameterBinding.task_name), /compiled scalar/,
        "every required nested state must actually exist in the compiled catalog");
    } finally { vscode.window.showOpenDialog = originalDialog; fs.rmSync(directory, { recursive: true, force: true }); }
  });
  test("finishes an input edit before tuning a replacement candidate", async () => {
    const { EngineerWorkflowSession } = require("../../controlStudio/engineerWorkflowSession");
    const loop = libraryLoop();
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    let release!: () => void;
    const retainedOwner = new Promise<void>(resolve => { release = resolve; });
    session.cancellationOwner = { cancel: () => retainedOwner, snapshot: () => undefined };
    const scope = { projectId: loop.projectId, editorSessionId: "edit-order", loopId: loop.id, loopRevision: loop.revision };
    const posts: string[] = [];
    const invoke = (action: string) => session.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`,
      messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
      expectedLoopRevision: scope.loopRevision, payload: { ...task, family: "PI" } }, scope,
    async (payload: { status: string }) => { posts.push(payload.status); });
    const editing = invoke("invalidate");
    const tuning = invoke("tune");
    try {
      await new Promise(resolve => setImmediate(resolve));
      const prematureCandidate = posts.includes("candidate");
      release();
      await Promise.all([editing, tuning]);
      assert.strictEqual(prematureCandidate, false, "retuning must wait for the previous edit's retained-operation reply");
      assert.deepStrictEqual(posts, ["edited", "candidate"], "an old edit cannot supersede the new candidate");
    } finally { release(); await Promise.allSettled([editing, tuning]); session.dispose(); }
  });
  test("accepts equivalent operating coordinates regardless of JSON member order", () => {
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const loop = libraryLoop();
    for (const state of loop.controller.pidInitialization.states) {
      if (typeof state.initial === "object") state.initial = { index: state.initial.index, source: state.initial.source };
    }
    assert.doesNotThrow(() => controllerAdapter(loop).checkInitialization());
  });
  test("accepts all seven actual library states and carries them into Verify and boundary tracking", () => {
    const { decodeControlLoopDefinition } = require("../../controlStudio/loopDefinition");
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const loop = libraryLoop();
    assert.strictEqual(decodeControlLoopDefinition(loop).kind, "accepted", "the actual seven-state Control.PID contract must be accepted");
    const adapter = controllerAdapter(loop);
    assert.deepStrictEqual(adapter.families, ["P", "PI", "PD", "PID"]);
    adapter.checkInitialization();
    const values = adapter.materialize({ proportionalGain: 2, integralGain: 1, derivativeGain: 0 }, {});
    const plan = engineerVerificationPlan(loop, model, task, values);
    const expected = { Setpoint: 4, SetpointRate: 0, ProcessValue: 4, ProcessValueRate: 0,
      FeedForward: 0, NominalOutput: 3, SaturatedOutput: 3 };
    for (const [member, value] of Object.entries(expected)) {
      const symbol = `Main.Controller.StateValue.${member}`;
      assert.strictEqual(plan.request.initial_values[symbol], value, `${member} uses its own library state semantics`);
      assert.ok(plan.request.observations.some((item: any) => item.symbol === symbol));
      assert.ok(plan.requirements.some((item: any) => item.channel === symbol && item.kind === "state-bounds"));
    }
    const initialization = adapter.initialization(model, task, values);
    assert.strictEqual(initialization.request.initial_values["Main.Controller.StateValue.NominalOutput"], 3);
    assert.strictEqual(adapter.transferBinding().states.length, 9, "seven library states plus reference and Manual output");
    assert.strictEqual(adapter.recoveryTracking(model).kind, "declared-state-v1", "reuse the existing runtime transaction contract");
  });

  test("rejects incomplete, duplicate, wrong-owner and wrong-unit library state contracts", () => {
    const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
    for (const mutation of [
      (states: any[]) => states.pop(),
      (states: any[]) => { states[6] = states[5]; },
      (states: any[]) => { states[0].symbol = "Main.Unrelated.StateValue.Setpoint"; },
      (states: any[]) => { states[3].unit = "V"; },
      (states: any[]) => { states[0].initial = 0; },
    ]) {
      const loop = libraryLoop(); mutation(loop.controller.pidInitialization.states);
      assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "rejected");
    }
  });

  test("carries declared application startup state alongside the seven library states", () => {
    const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
    const { controllerAdapter } = require("../../controlStudio/controllerAdapters");
    const { engineerVerificationPlan } = require("../../controlStudio/engineerVerificationPlan");
    const loop = libraryLoop();
    loop.controller.pidInitialization.applicationStates = [{ symbol: "Main.Initialized", iec_type: "BOOL", unit: "1",
      minimum: 0, maximum: 1, initial: 1, postScan: "unchanged" }];
    assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted");
    const adapter = controllerAdapter(loop);
    const plan = engineerVerificationPlan(loop, model, task, { "Main.Kp": 2, "Main.Ki": 1, "Main.Kd": 0 });
    assert.strictEqual(plan.request.initial_values["Main.Initialized"], 1, "startup cannot overwrite the reviewed PID history");
    assert.ok(adapter.transferBinding().states.some((state: any) => state.symbol === "Main.Initialized" && state.iec_type === "BOOL"));
  });
});
