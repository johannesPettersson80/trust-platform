import * as assert from "assert";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { pidLoop, channel } from "./control-studio-pid-fixtures";

suite("Control Studio connected PID engineer path", () => {
  test("retains the chosen application mode when later requests omit it", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "application-mode", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "application-mode", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    let messageSequence = 0;
    const invoke = (extra: Record<string, unknown>) => session.handle({ schemaVersion: 1,
      kind: "control-studio.engineer.tune", messageId: `tune-${++messageSequence}`, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, ...extra } }, scope, async value => { posts.push(value); });
    try {
      await invoke({});
      assert.strictEqual(posts[posts.length - 1].status, "failed", "default Auto must require its initialization adapter");
      assert.match(posts[posts.length - 1].reason, /initialization/i);
      await invoke({ applicationMode: "manual" });
      assert.strictEqual(posts[posts.length - 1].status, "candidate");
      await invoke({});
      assert.strictEqual(posts[posts.length - 1].status, "candidate");
      assert.strictEqual(session.snapshotStudy()?.applicationMode, "manual");
      assert.strictEqual(await invoke({ applicationMode: "invalid" }), false);
      assert.strictEqual(session.snapshotStudy()?.applicationMode, "manual");
    } finally { session.dispose(); }
  });

  test("reports current workflow stages for a typed PID controller", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "pid-stages", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-stages", loopId: loop.id, loopRevision: loop.revision };
    try {
      assert.deepStrictEqual(session.currentStageEvidence(), { loopBound: true, hasModel: true,
        hasCandidate: false, validationHasResult: false, validationPassed: false });
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "tune",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "PI", applicationMode: "manual",
          referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 } },
        scope, async () => undefined);
      assert.strictEqual(session.currentStageEvidence()?.hasCandidate, true);
      assert.strictEqual(session.currentStageEvidence()?.validationPassed, false);
      session.modelChanged();
      assert.strictEqual(session.currentStageEvidence()?.hasCandidate, false);
      model.validation = "unvalidated";
      session.modelChanged();
      assert.strictEqual(session.currentStageEvidence()?.hasModel, false, "unvalidated model cannot make Tune ready");
    } finally { session.dispose(); }
  });

  test("saves failed automatic controller comparisons with all rejection reasons", async () => {
    const loop = pidLoop();
    for (const parameter of loop.parameterBinding.parameters) { parameter.minimum = 0; parameter.maximum = 0; }
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "rejected-study", source: "provided", validation: "validated", evidence: ["record"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "failed-study", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "failed-study",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", applicationMode: "manual",
          referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 } },
        scope, async payload => { posts.push(payload); });
      assert.strictEqual(posts[posts.length - 1].status, "failed");
      assert.strictEqual(posts[posts.length - 1].attempts?.length, 4, "failure must publish the retained comparison");
      const saved = session.snapshotStudy();
      assert.strictEqual(saved?.familyEvaluations?.length, 4);
      assert.deepStrictEqual(saved?.task, { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4 }, "failed comparison must retain the exact task");
      assert.strictEqual((saved as any)?.controllerChoice, "automatic");
      const reopened = new EngineerWorkflowSession(loop, () => model, () => undefined);
      try {
        reopened.restoreStudy(saved);
        assert.deepStrictEqual(reopened.snapshotStudy()?.task, saved?.task);
        assert.strictEqual((reopened.snapshotStudy() as any)?.controllerChoice, "automatic");
      } finally { reopened.dispose(); }
      assert.ok(saved?.familyEvaluations?.every(attempt => attempt.status === "unavailable" && attempt.reason));
    } finally { session.dispose(); }
  });

});
