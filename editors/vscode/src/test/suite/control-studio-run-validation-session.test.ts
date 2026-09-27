import * as assert from "assert";
import { RunValidationSession } from "../../controlStudio/runValidationSession";
import { isSavedRunValidation } from "../../controlStudio/runValidationPersistence";
const scope = { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" };
const run = { id: "engineer-run", evidence: "ST-executed", parameters: { gain: 2 },
  conditions: { initialState: { x: 0 }, events: [], durationSeconds: 2, sampleTimesSeconds: [0, 1, 2], seed: 7,
    modelRevision: "m", programRevision: "p", parameterSchemaRevision: "s" },
  channels: [{ id: "axis", unit: "m" }], samples: [0, 1, 2].map(i => ({ timeSeconds: i, sequence: i, values: { axis: { value: 1, quality: "good" } } })) };
const requirements = [{ id: "error", kind: "error", channel: "axis", window: { start: 0, end: 2 }, reference: 1, statistic: "maximum", maximum: 0.1 }];
const message = (action: string, payload: Record<string, unknown>) => ({ schemaVersion: 1, kind: `control-studio.run-validation.${action}`, messageId: "request", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", payload });
suite("Control Studio imported-run validation session", () => {
  test("evaluates complete imported contracts with explicit provenance and saves typed evidence", async () => {
    const session = new RunValidationSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    assert.strictEqual(await session.handle(message("import-current", { json: JSON.stringify(run) }), scope, post), true);
    await session.handle(message("evaluate", { json: JSON.stringify(requirements) }), scope, post);
    assert.strictEqual(posts[posts.length - 1]?.status, "complete");
    assert.ok(isSavedRunValidation(session.snapshot()));
    const forged = session.snapshot()!;
    (forged.requirements[0] as any).kind = "unknown-calculation";
    (forged.evaluation.specification[0] as any).kind = "unknown-calculation";
    assert.strictEqual(isSavedRunValidation(forged), false);
    assert.strictEqual(session.snapshot()?.evaluation.status, "Pass");
    assert.strictEqual(session.snapshot()?.origin, "imported-record-contract");
    assert.strictEqual(await session.handle(message("invalidate", {}), scope, post), true);
    assert.strictEqual(session.snapshot(), undefined);
    session.dispose();
  });
  test("never invents missing experiment conditions", async () => {
    const session = new RunValidationSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    await session.handle(message("import-current", { json: JSON.stringify({ ...run, conditions: undefined }) }), scope, post);
    assert.strictEqual(posts[posts.length - 1]?.status, "failed");
    assert.strictEqual(session.snapshot(), undefined);
    session.dispose();
  });
  test("rejects malformed restore without discarding the saved validation", async () => {
    const session = new RunValidationSession();
    const posts: Record<string, unknown>[] = [];
    const post = async (p: Record<string, unknown>) => { posts.push(p); };
    await session.handle(message("import-current", { json: JSON.stringify(run) }), scope, post);
    await session.handle(message("evaluate", { json: JSON.stringify(requirements) }), scope, post);
    const before = session.snapshot();
    assert.ok(before);
    assert.throws(() => session.restore({ malformed: true } as any), /invalid/i);
    assert.deepStrictEqual(session.snapshot(), before, "invalid restore must preserve the current validation");
    session.dispose();
  });
});
