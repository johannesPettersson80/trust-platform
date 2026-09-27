import * as assert from "assert";
import { initializeChartExportBridge, requestChartExport } from "../../controlStudio/webview/chartExportBridge";
import { saveControlStudioChartExport } from "../../controlStudio/chartExportHost";
const { JSDOM } = require("jsdom");

suite("Control Studio chart export bridge", () => {
  test("carries a cross-bundle request through scoped host save and ignores stale replies", async () => {
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r"><div class="plot-toolbar"></div></main>');
    const doc = dom.window.document as Document;
    let sent: any;
    initializeChartExportBridge((kind, payload) => { sent = { schemaVersion: 1, messageId: "m", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", kind, payload }; }, doc);
    const id = requestChartExport("data:application/json;charset=utf-8,%7B%7D", "candidate.json", "application/json", doc);
    assert.strictEqual(sent.payload.requestId, id);
    let bytes: Uint8Array | undefined;
    const result = await saveControlStudioChartExport(sent, { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" }, {
      showSaveDialog: async () => ({ fsPath: "/tmp/my-candidate.json" }), writeFile: async (_, value) => { bytes = value; },
    });
    assert.strictEqual(result.ok, true);
    assert.strictEqual(new TextDecoder().decode(bytes), "{}");
    const response = { schemaVersion: 1, kind: "control-studio.chart.export.result", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", requestId: id, result };
    dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: { ...response, expectedLoopRevision: "stale" } }));
    assert.match(doc.querySelector('[data-chart-export-status]')!.textContent!, /Choose/);
    dom.window.dispatchEvent(new dom.window.MessageEvent("message", { data: response }));
    assert.strictEqual(doc.querySelector('[data-chart-export-status]')!.textContent, "Saved my-candidate.json");
  });
});
