import * as assert from "assert";
import { initializeChartExportBridge } from "../../controlStudio/webview/chartExportBridge";
import { saveControlStudioChartExport } from "../../controlStudio/chartExportHost";
const { JSDOM } = require("jsdom");

suite("Control Studio chart export bridge", () => {
  test("carries a cross-bundle request through scoped host save and ignores stale replies", async () => {
    const dom = new JSDOM('<main data-control-studio data-project-id="p" data-editor-session-id="e" data-loop-id="l" data-loop-revision="r"><div class="plot-toolbar"></div></main>');
    const doc = dom.window.document as Document;
    let sent: any;
    initializeChartExportBridge((kind, payload) => { sent = { schemaVersion: 1, messageId: "m", projectId: "p", editorSessionId: "e", loopId: "l", expectedLoopRevision: "r", kind, payload }; }, doc);
    const id = "export-1";
    dom.window.dispatchEvent(new dom.window.CustomEvent("trust-control-studio-export-request", { detail: {
      url: "data:application/json;charset=utf-8,%7B%7D", filename: "candidate.json", mimeType: "application/json", requestId: id } }));
    assert.strictEqual(sent.payload.requestId, id);
    let bytes: Uint8Array | undefined;
    // The engine names the file and gives its bytes; the host shows the dialog and writes them.
    const engine = { async chartExport() { return { result: { fileName: "candidate.json", filterLabel: "JSON data", extension: "json", bytesBase64: Buffer.from("{}").toString("base64") } }; } };
    const result = await saveControlStudioChartExport(engine, sent, { projectId: "p", editorSessionId: "e", loopId: "l", loopRevision: "r" }, {
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
