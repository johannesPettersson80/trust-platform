import * as assert from "assert";
import {
  saveControlStudioChartExport,
  type ChartExportEngine,
  type ChartExportHost,
  type ChartExportScope,
} from "../../controlStudio/chartExportHost";
import type { ChartExportAnswer } from "../../controlStudio/generated/ChartExportAnswer";
import type { EngineAnswer } from "../../controlStudio/engine/engineClient";

// The engine checks the page's message, decodes it and names the file (`controlStudio/chartExport`, specification 35 B10.5, pinned by
// trust-control's `chart_export` tests). These tests cover the host's wiring: the dialog shows the engine's name and filter, the bytes
// the engine produced are written, and the engine's refusal opens no dialog.

const pngAnswer: ChartExportAnswer = { fileName: "response.png", filterLabel: "PNG image", extension: "png", bytesBase64: "aGk=" };

suite("Control Studio chart export host", () => {
  test("shows the save dialog with the engine's name and filter, and writes the bytes the engine produced", async () => {
    const writes: Array<{ path: string; bytes: Uint8Array }> = [];
    const dialogs: unknown[] = [];
    const host: ChartExportHost = {
      showSaveDialog: async options => {
        dialogs.push(options);
        return { fsPath: `/tmp/${options.defaultFileName}` };
      },
      writeFile: async (uri, bytes) => { writes.push({ path: uri.fsPath, bytes }); },
    };
    const result = await saveControlStudioChartExport(engineAnswering({ result: pngAnswer }), message(), scope(), host);
    assert.deepStrictEqual(result, { ok: true, filename: "response.png", path: "/tmp/response.png" });
    assert.deepStrictEqual(dialogs, [{ defaultFileName: "response.png", filterLabel: "PNG image", extension: "png" }]);
    assert.strictEqual(writes[0].path, "/tmp/response.png");
    assert.deepStrictEqual([...writes[0].bytes], [104, 105]);
  });

  test("sends the page's message and the panel's session to the engine as they are", async () => {
    const sent: unknown[] = [];
    const engine: ChartExportEngine = {
      async chartExport(request) {
        sent.push(request);
        return { result: pngAnswer };
      },
    };
    await saveControlStudioChartExport(engine, message(), scope(), acceptingHost());
    assert.deepStrictEqual(sent, [{ message: message(), session: scope() }]);
  });

  test("opens no dialog when the engine refuses the message, and shows the engine's sentence", async () => {
    let dialogs = 0;
    const host: ChartExportHost = {
      showSaveDialog: async () => { dialogs++; return undefined; },
      writeFile: async () => undefined,
    };
    const refused = await saveControlStudioChartExport(engineAnswering({
      failure: { message: "The chart export session is stale or invalid.", detail: null },
    }), message(), scope(), host);
    assert.deepStrictEqual(refused, { ok: false, reason: "The chart export session is stale or invalid." });
    assert.strictEqual(dialogs, 0);
  });

  test("reports cancellation and a write failure", async () => {
    const cancelled = await saveControlStudioChartExport(engineAnswering({ result: pngAnswer }), message(), scope(), {
      showSaveDialog: async () => undefined, writeFile: async () => undefined,
    });
    assert.deepStrictEqual(cancelled, { ok: false, reason: "Export cancelled." });
    const failed = await saveControlStudioChartExport(engineAnswering({ result: pngAnswer }), message(), scope(), {
      showSaveDialog: async () => ({ fsPath: "/tmp/response.png" }),
      writeFile: async () => { throw new Error("disk full"); },
    });
    assert.deepStrictEqual(failed, { ok: false, reason: "disk full" });
  });
});

function message(): unknown {
  return {
    schemaVersion: 1, kind: "control-studio.chart.export", messageId: "m1", requestId: "r1",
    projectId: "project", editorSessionId: "editor", loopId: "loop", expectedLoopRevision: "rev",
    payload: { requestId: "export-1", data: "data:image/png;base64,aGk=", mimeType: "image/png", filename: "response.png" },
  };
}

function scope(): ChartExportScope {
  return { projectId: "project", editorSessionId: "editor", loopId: "loop", loopRevision: "rev" };
}

/** An engine double that answers `answer`, a result or a failure as the engine's answer carries it. */
function engineAnswering(answer: unknown): ChartExportEngine {
  return { async chartExport() { return answer as EngineAnswer<ChartExportAnswer>; } };
}

function acceptingHost(): ChartExportHost {
  return { showSaveDialog: async () => undefined, writeFile: async () => undefined };
}
