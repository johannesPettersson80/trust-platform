import * as assert from "assert";
import * as vscode from "vscode";

suite("Runtime default settings integration (VS Code)", function () {
  test("activation does not seed runtime control endpoint into workspace folder settings", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected a workspace folder for tests.");

    const config = vscode.workspace.getConfiguration("trust-lsp", workspaceFolder.uri);
    const inspected = config.inspect<string>("runtime.controlEndpoint");

    assert.ok(inspected, "Expected runtime.controlEndpoint inspection metadata.");
    assert.strictEqual(
      inspected?.workspaceFolderValue,
      undefined,
      "runtime.controlEndpoint should not be written into workspace folder settings during activation."
    );
  });
});
