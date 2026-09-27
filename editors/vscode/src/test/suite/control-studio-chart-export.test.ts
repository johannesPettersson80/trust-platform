import * as assert from "assert";
import { saveControlStudioChartExport, type ChartExportHost } from "../../controlStudio/chartExportHost";

suite("Control Studio chart export host", () => {
  test("writes a scoped PNG through the chosen save URI", async () => {
    const writes: Array<{ path: string; bytes: Uint8Array }> = [];
    const host: ChartExportHost = {
      showSaveDialog: async options => ({ fsPath: `/tmp/${options.defaultFileName}` }),
      writeFile: async (uri, bytes) => { writes.push({ path: uri.fsPath, bytes }); },
    };
    const result = await saveControlStudioChartExport({
      schemaVersion: 1, kind: "control-studio.chart.export", messageId: "m1", requestId: "r1",
      projectId: "project", editorSessionId: "editor", loopId: "loop", expectedLoopRevision: "rev",
      payload: { requestId: "export-1", data: "data:image/png;base64,aGk=", mimeType: "image/png", filename: "response.png" },
    }, { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "rev" }, host);
    assert.deepStrictEqual(result, { ok: true, filename: "response.png", path: "/tmp/response.png" });
    assert.strictEqual(writes[0].path, "/tmp/response.png");
    assert.deepStrictEqual([...writes[0].bytes], [104, 105]);
  });

  test("rejects stale scope and unsupported MIME without opening a dialog", async () => {
    let dialogs = 0;
    const host: ChartExportHost = {
      showSaveDialog: async () => { dialogs++; return undefined; },
      writeFile: async () => undefined,
    };
    const base = {
      schemaVersion: 1, kind: "control-studio.chart.export", messageId: "m1", requestId: "r1",
      projectId: "project", editorSessionId: "editor", loopId: "loop", expectedLoopRevision: "rev",
      payload: { requestId: "export-1", data: "data:image/png;base64,aGk=", mimeType: "image/png", filename: "response.png" },
    } as const;
    const stale = await saveControlStudioChartExport({ ...base, expectedLoopRevision: "old" },
      { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "rev" }, host);
    assert.strictEqual(stale.ok, false);
    const mime = await saveControlStudioChartExport({ ...base, payload: { ...base.payload, mimeType: "text/html" } },
      { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "rev" }, host);
    assert.strictEqual(mime.ok, false);
    assert.strictEqual(dialogs, 0);
  });

  test("supports plant scope and reports cancellation or write failure", async () => {
    const base = {
      schemaVersion: 1, kind: "control-studio.chart.export", messageId: "m1", requestId: "r1",
      projectId: "project", editorSessionId: "editor", plantId: "plant", expectedPlantRevision: "plant-rev",
      payload: { requestId: "export-1", data: "data:application/json;charset=utf-8,%7B%7D", mimeType: "application/json", filename: "candidate.json" },
    } as const;
    const scope = { projectId: "project", editorSessionId: "editor", plantId: "plant", plantRevision: "plant-rev" };
    const cancelled = await saveControlStudioChartExport(base, scope, {
      showSaveDialog: async () => undefined, writeFile: async () => undefined,
    });
    assert.deepStrictEqual(cancelled, { ok: false, reason: "Export cancelled." });
    const failed = await saveControlStudioChartExport(base, scope, {
      showSaveDialog: async () => ({ fsPath: "/tmp/candidate.json" }),
      writeFile: async () => { throw new Error("disk full"); },
    });
    assert.deepStrictEqual(failed, { ok: false, reason: "disk full" });
  });
});
