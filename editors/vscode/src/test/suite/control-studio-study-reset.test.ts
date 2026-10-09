import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { StudioAnalysisSessions } from "../../controlStudio/studioAnalysisSessions";
import { openProjectStudy } from "../../controlStudio/studyStore";
import { pidLoop } from "./control-studio-pid-fixtures";

suite("Control Studio study reset", () => {
  const model = { kind: "transfer-function" as const, numerator: [1], denominator: [4, 1], delaySeconds: 0,
    revision: "reset-model", source: "provided" as const, validation: "validated" as const, evidence: ["qualified-source"],
    inputs: [{ id: "u", unit: "V", operatingRange: { minimum: -20, maximum: 20 } }],
    outputs: [{ id: "y", unit: "m", operatingRange: { minimum: -20, maximum: 20 } }] };
  const scope = { projectId: "reset-project", editorSessionId: "reset-editor", loopId: "reset-loop", loopRevision: "reset-revision" };
  const request = { schemaVersion: 1, kind: "control-studio.study.reset", messageId: "reset-once", ...scope, expectedLoopRevision: scope.loopRevision, payload: {} };

  /** Reset keeps the previous working study as history, so the loop's project study is open first. */
  async function openStudy(): Promise<void> {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "control-studio-reset-"));
    await openProjectStudy(root, { ...pidLoop(), projectId: scope.projectId, id: scope.loopId, revision: scope.loopRevision } as ControlLoopDefinition);
  }

  test("clears the model and rejects a replay after a new model has been supplied", async () => {
    await openStudy();
    const sessions = new StudioAnalysisSessions(pidLoop()); const events: any[] = [];
    try {
      sessions.model.selectModel(model);
      assert.strictEqual(await sessions.handle(request, scope, async (_stage, payload) => { events.push(payload); }), true);
      assert.strictEqual(sessions.model.selectedModel(), undefined);
      assert.deepStrictEqual(events[0].stageEvidence, { hasModel: false, hasCandidate: false, validationPassed: false });
      sessions.model.selectModel({ ...model, revision: "new-model" });
      await sessions.handle(request, scope, async (_stage, payload) => { events.push(payload); });
      assert.strictEqual(sessions.model.selectedModel()?.revision, "new-model", "A duplicate Reset cannot erase work done after the original Reset");
      assert.strictEqual(events.length, 1);
    } finally { sessions.dispose(); }
  });
  test("preserves current evidence when a recording or controller operation is active", async () => {
    await openStudy();
    const sessions = new StudioAnalysisSessions(pidLoop()); const events: any[] = [];
    try {
      sessions.model.selectModel(model);
      await sessions.handle(request, scope, async (_stage, payload) => { events.push(payload); }, true);
      assert.strictEqual(events[0].status, "failed"); assert.strictEqual(sessions.model.selectedModel()?.revision, model.revision);
      (sessions.engineer as any).state.busy = "apply";
      await sessions.handle({ ...request, messageId: "reset-during-apply" }, scope, async (_stage, payload) => { events.push(payload); });
      assert.strictEqual(events[1].status, "failed"); assert.strictEqual(sessions.model.selectedModel()?.revision, model.revision);
      (sessions.engineer as any).state.busy = undefined;
      assert.strictEqual(await sessions.handle({ ...request, loopId: "another-loop" }, scope, async () => { throw new Error("Must ignore foreign reset"); }), false);
    } finally { sessions.dispose(); }
  });
  test("refuses a reset when the previous study cannot be kept as history", async () => {
    const unopened = { ...scope, loopId: "never-opened-loop" };
    const unopenedRequest = { ...request, loopId: unopened.loopId, messageId: "reset-unopened" };
    const sessions = new StudioAnalysisSessions(pidLoop()); const events: any[] = [];
    try {
      sessions.model.selectModel(model);
      await sessions.handle(unopenedRequest, unopened, async (_stage, payload) => { events.push(payload); });
      assert.strictEqual(events[0].status, "failed");
      assert.match(String(events[0].reason), /not open/);
      assert.strictEqual(sessions.model.selectedModel()?.revision, model.revision, "a reset that cannot keep history changes nothing");
    } finally { sessions.dispose(); }
  });
});
