import { pidLoop, channel, pidInitialization } from "./control-studio-pid-fixtures";
import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";

/** Apply starts the reviewed controller from the validated operating point of the selected model. */
suite("Control Studio commissioning starting point", () => {
  /** Tunes, validates and starts Apply against a runtime whose process value follows `processValue` on every reading. */
  async function startApply(processValue: () => number, options: { outerMode?: 0 | 1 } = {}) {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    // An inner cascade loop: its setpoint is the outer controller's manual output while the outer one is in Manual.
    if (options.outerMode !== undefined) loop.cascade = { role: "inner", partnerLoopId: "outer-loop",
      referenceHold: { modeSymbol: "Main.OuterAuto", manualValue: false, manualOutputSymbol: "Main.SP" } };
    const bindings: any[] = [];
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")],
      operatingPoint: { inputs: [0], outputs: [2] } };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const runtime = require("../../controlStudio/runtimeConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldRuntime = runtime.bindRuntimeConnection, oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const observed = { values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
      active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null };
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected search operation ${operation}`);
    } });
    // No apply or rollback: the review phase must not write parameters.
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => observed,
      review: async (values: any) => ({ observed, values }),
    } });
    // Only status reads: the review phase must not transfer any state.
    runtime.bindRuntimeConnection = async () => ({ source: "simulation", request: async (operation: string, params: any) => {
      assert.strictEqual(operation, "control.transfer.status");
      bindings.push(params?.binding);
      return { program_revision: "program", binding_revision: "transfer", status: "idle", reason: null, transaction: null,
        supported_tracking: ["parallel-pid-v1"], values: { "Main.Manual": true, "Main.SP": 2, "Main.ManualCommand": 0,
          "Main.Integral": 0, "Main.PreviousError": 0, "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0, "Main.PV": processValue(), "Main.MV": 0,
          // A runtime reports exactly the members the binding declares.
          ...(options.outerMode !== undefined && params?.binding?.guards?.some((guard: any) => guard.symbol === "Main.OuterAuto")
            ? { "Main.OuterAuto": options.outerMode } : {}) } };
    } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "starting-point", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: `${action}-${posts.length}`, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    const automatic = { applicationMode: "auto" as const };
    await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
      settlingTimeSeconds: 2, durationSeconds: 4, ...automatic });
    await invoke("verify", automatic);
    const applying = invoke("apply", automatic);
    const restore = () => {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      runtime.bindRuntimeConnection = oldRuntime; verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    };
    return { posts, applying, invoke, restore, bindings };
  }
  const until = async (condition: () => boolean, message: string) => {
    for (let waited = 0; !condition(); waited += 20) {
      if (waited > 5000) throw new Error(message);
      await new Promise(resolve => setTimeout(resolve, 20));
    }
  };

  test("refuses Apply of an inner cascade loop while its outer controller is in Auto, before waiting or writing", async () => {
    const run = await startApply(() => 2.004, { outerMode: 1 });
    try {
      await run.applying;
      const last = run.posts[run.posts.length - 1];
      assert.deepStrictEqual([last?.status, last?.failure?.nextAction], ["failed", "reapply"], String(last?.reason));
      assert.strictEqual(last?.reason, "Put the outer controller in Manual before applying (Main.OuterAuto): its output is this loop's setpoint.");
      assert.ok(!run.posts.some(post => post.activity === "apply-settling"), "the refusal comes before any wait");
      assert.ok(run.bindings.length && run.bindings.every(binding => binding.guards.some((guard: any) =>
        guard.symbol === "Main.OuterAuto" && guard.iec_type === "BOOL")), "the transfer guards the outer mode, so a later switch to Auto is refused too");
    } finally { run.restore(); }
  });

  test("reviews Apply of an inner cascade loop with its outer controller in Manual", async () => {
    const run = await startApply(() => 2.004, { outerMode: 0 });
    try {
      await run.applying;
      const last = run.posts[run.posts.length - 1];
      assert.strictEqual(last?.status, "review", String(last?.reason));
    } finally { run.restore(); }
  });

  test("reviews Apply when the process has settled within the requirement settling band", async () => {
    const run = await startApply(() => 2.004);
    try {
      await run.applying;
      const last = run.posts[run.posts.length - 1];
      assert.strictEqual(last?.status, "review", `A settled process within ±0.02 m of the validated 2 m must reach the Apply review: ${last?.reason}`);
    } finally { run.restore(); }
  });

  test("waits outside the settling band with its progress, and Abort ends the wait without writing anything", async function () {
    this.timeout(15000);
    const run = await startApply(() => 2.5);
    try {
      await until(() => run.posts.some(post => post.activity === "apply-settling"), "Apply must show that it waits for the process to settle");
      assert.strictEqual(run.posts.find(post => post.activity === "apply-settling").reason, "Waiting for Main.PV to settle at 2 m ± 0.02 m (now 2.5 m)…");
      await run.invoke("cancel");
      await run.applying;
      const last = run.posts[run.posts.length - 1];
      assert.deepStrictEqual([last?.status, last?.reason, last?.failure?.nextAction],
        ["failed", "Apply cancelled while waiting for the process to settle; nothing was written.", "reapply"]);
      assert.ok(last?.initialization, "An ended wait keeps the verified controller-transfer evidence");
    } finally { run.restore(); }
  });

  test("keeps progress messages light: no recordings are resent while Apply waits", async function () {
    this.timeout(15000);
    let readings = 0;
    const run = await startApply(() => (readings++ < 8 ? 2.5 - readings * 0.01 : 2.004));
    try {
      await run.applying;
      const progress = run.posts.filter(post => post.activity === "apply-settling");
      assert.ok(progress.length > 0, "Apply waited while the process was outside its band");
      for (const post of progress) {
        assert.ok(!("campaigns" in post) && !("modelValidation" in post), `a progress message carries no recordings: ${Object.keys(post).join(", ")}`);
        assert.ok(JSON.stringify(post).length < 4000, `a progress message stays small: ${JSON.stringify(post).length} characters`);
      }
      assert.strictEqual(new Set(progress.map(post => post.reason)).size, progress.length, "only a changed reading is posted again");
    } finally { run.restore(); }
  });

  test("continues to the Apply review without another click once the process settles", async function () {
    this.timeout(15000);
    let readings = 0;
    const run = await startApply(() => (readings++ < 4 ? 2.5 : 2.004));
    try {
      await run.applying;
      const last = run.posts[run.posts.length - 1];
      assert.ok(run.posts.some(post => post.activity === "apply-settling"), "Apply waited while the process was outside its band");
      assert.strictEqual(last?.status, "review", `Apply continues once the process has settled: ${last?.reason}`);
    } finally { run.restore(); }
  });
});
