import * as assert from "assert";
import { GeneralModelSession } from "../../controlStudio/generalModelSession";
import { LeadLagSession } from "../../controlStudio/leadLagSession";
const scope = { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" };
const model = { kind: "transfer-function", numerator: [2], denominator: [1, 2], delaySeconds: 0, revision: "m", source: "provided", validation: "unvalidated", evidence: [], inputs: [{ id: "drive", unit: "V", operatingRange: { minimum: 0, maximum: 10 } }], outputs: [{ id: "speed", unit: "rpm", operatingRange: { minimum: 0, maximum: 100 } }] };
const message = (action: string, payload: Record<string, unknown>) => ({ schemaVersion: 1, kind: `control-studio.general-model.${action}`, messageId: "m", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", payload });
suite("Control Studio supplied model jobs", () => {
  test("keeps a saved lead/lag design only for the selected model it was made from", () => {
    let selected = model as any;
    const session = new LeadLagSession(() => selected);
    const saved = { model, mode: "lead", crossoverRadPerSecond: 1, value: 30,
      result: { status: "qualified", gain: 1, zeroRadPerSecond: 0.5, poleRadPerSecond: 2, phaseContributionDegrees: 30, openLoopMagnitudeAtCrossover: 1 } };
    try {
      session.restore(saved as any);
      assert.deepStrictEqual(session.snapshot(), saved);
      selected = { ...model, revision: "replacement" };
      assert.strictEqual(session.snapshot(), undefined, "A saved design must not survive a selected-model replacement");
    } finally { session.dispose(); }
  });
  test("imports a supplied model as the selected model and replaces it on a new import", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (payload: Record<string, unknown>) => { posts.push(payload); };
    try {
      assert.strictEqual(await session.handle(message("import", { json: JSON.stringify(model) }), scope, post), true);
      assert.strictEqual(posts[posts.length - 1]?.status, "imported");
      assert.deepStrictEqual(session.selectedModel(), model);
      const replacement = { ...model, revision: "replacement", numerator: [3] };
      await session.handle(message("import", { json: JSON.stringify(replacement) }), scope, post);
      assert.deepStrictEqual(session.selectedModel(), replacement);
    } finally { session.dispose(); }
  });
  test("converts independently accepted identification into the shared selected model without approximating delay", () => {
    const { IdentificationWorkflow } = require("../../controlStudio/identification");
    const bridge = require("../../controlStudio/selectedModelAnalysis");
    const workflow = new IdentificationWorkflow();
    const samples = (amplitude: number) => Array.from({ length: 301 }, (_, i) => ({
      timeSeconds: i * 0.1, input: i >= 20 ? amplitude : 0,
      output: i >= 25 ? 2 * amplitude * (1 - Math.exp(-(i - 25) * 0.1 / 3)) : 0,
      quality: "good", inputUnit: "V", outputUnit: "m" }));
    workflow.qualify(samples(1), [], "Simulation", "fopdt");
    workflow.fit();
    workflow.qualify(samples(1.5), [], "Simulation", "fopdt");
    workflow.fit();
    const accepted = workflow.accept();
    assert.strictEqual(typeof bridge.acceptedIdentificationModel, "function",
      "accepted identification must enter the same selected-model contract as supplied models");
    const selected = bridge.acceptedIdentificationModel(accepted, {
      inputs: [{ id: "command", unit: "V", operatingRange: { minimum: -10, maximum: 10 } }],
      outputs: [{ id: "position", unit: "m", operatingRange: { minimum: -10, maximum: 10 } }],
    });
    assert.strictEqual(selected.source, "fitted");
    assert.strictEqual(selected.validation, "validated");
    assert.strictEqual(selected.kind, "transfer-function");
    assert.deepStrictEqual(selected.numerator, [accepted.model.gain]);
    assert.deepStrictEqual(selected.denominator, [accepted.model.timeConstantSeconds, 1]);
    assert.strictEqual(selected.delaySeconds, accepted.model.deadTimeSeconds);
    assert.strictEqual(selected.evidence.length, 2);
    assert.notStrictEqual(selected.evidence[0], selected.evidence[1], "independent record identities remain distinct");
    assert.ok(selected.revision.startsWith("sha256:"));
  });
  test("rejects a blocked model replacement before discarding the selected model and permits retry", async () => {
    let busy = false;
    const session = new GeneralModelSession(() => { if (busy) throw new Error("Finish or cancel the running operation, then retry"); });
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    try {
      await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
      const replacement = { ...model, revision: "replacement", numerator: [3] };
      busy = true;
      await session.handle(message("import", { json: JSON.stringify(replacement) }), scope, post);
      assert.deepStrictEqual(session.selectedModel(), model, "Blocked import must not replace the model");
      assert.strictEqual(posts[posts.length - 1].status, "failed");
      busy = false;
      await session.handle(message("import", { json: JSON.stringify(replacement) }), scope, post);
      assert.deepStrictEqual(session.selectedModel(), replacement);
    } finally { session.dispose(); }
  });
  test("keeps the selected model when a restore is malformed", async () => {
    const session = new GeneralModelSession();
    const post = async (_p: Record<string, unknown>) => undefined;
    try {
      await session.handle(message("import", { json: JSON.stringify(model) }), scope, post);
      assert.throws(() => session.restore({ broken: true } as any), /invalid/i);
      assert.deepStrictEqual(session.selectedModel(), model, "a rejected restore must not replace the selected model");
    } finally { session.dispose(); }
  });
  test("rejects a supplied model message over the 256 kB limit", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    try {
      await session.handle(message("import", { json: JSON.stringify({ ...model, evidence: ["x".repeat(270000)] }) }), scope, post);
      assert.strictEqual(posts[posts.length - 1]?.status, "failed");
      assert.strictEqual(session.selectedModel(), undefined);
    } finally { session.dispose(); }
  });
  test("never accepts a model message from another editor scope", async () => {
    const session = new GeneralModelSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    try {
      assert.strictEqual(await session.handle(message("import", { json: JSON.stringify(model) }), { ...scope, editorSessionId: "other" }, post), false);
      assert.strictEqual(posts.length, 0);
      assert.strictEqual(session.selectedModel(), undefined);
    } finally { session.dispose(); }
  });
});
