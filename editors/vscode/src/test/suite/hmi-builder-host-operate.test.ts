import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { openBuilder, postedOfType } from "./hmiBuilderHost";

suite("HMI Builder host: named Live operation", function () {
  this.timeout(15000);
  test("the editor routes its operating connection and closes it with its panel", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-hmi-live-"));
    fs.mkdirSync(path.join(root, "hmi", "pages"), { recursive: true });
    const file = path.join(root, "hmi", "pages", "overview.toml");
    fs.writeFileSync(file, 'format = 1\ntitle = "Cell"\n');
    const requests: string[] = [];
    const { panel, webview } = await openBuilder(file, undefined, undefined, undefined, {
      dirty: () => false,
      bundle: async () => ({ ok: true, version: "v1" }),
      session: {
        target: async () => ({ url: "http://plc:8080", endpoint: "tcp://plc:9000", label: "Cell" }),
        request: async (_target, _method, route) => {
          requests.push(route);
          return { status: 200, body: { bundle: "v1", page: { name: "overview" } } };
        },
      },
    });
    try {
      webview.send({ type: "operateOpen", id: "one", page: "overview" });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.deepStrictEqual(
        postedOfType(webview, "operateReady"),
        [{ type: "operateReady", id: "one", target: "Cell" }],
        "the provider connects the Live operating view",
      );
      const edit = new vscode.WorkspaceEdit();
      edit.insert(vscode.Uri.file(file), new vscode.Position(0, 0), "# changed\n");
      await vscode.workspace.applyEdit(edit);
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.ok(
        postedOfType(webview, "operateEnded").some((message) => /changed/i.test(message.reason)),
        "editing the HMI ends its operating connection immediately",
      );
      panel.dispose();
      webview.send({ type: "operateRequest", id: "one", request: 1, method: "POST", path: "/api/hmi/login", body: {} });
      await new Promise((resolve) => setTimeout(resolve, 100));
      assert.deepStrictEqual(requests, ["/api/hmi/page?name=overview"]);
    } finally {
      panel.dispose();
    }
  });
});
