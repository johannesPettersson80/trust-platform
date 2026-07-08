import { getTrustConfiguration } from "../../configuration";
import * as assert from "assert";
import * as vscode from "vscode";

suite("Runtime default settings integration (VS Code)", function () {
  test("activation does not seed runtime control endpoint into workspace folder settings", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected a workspace folder for tests.");

    const config = getTrustConfiguration(workspaceFolder.uri);
    const inspected = config.inspect<string>("runtime.controlEndpoint");

    assert.ok(inspected, "Expected runtime.controlEndpoint inspection metadata.");
    assert.strictEqual(
      inspected?.workspaceFolderValue,
      undefined,
      "runtime.controlEndpoint should not be written into workspace folder settings during activation."
    );
  });

  test("product Settings keys feed runtime config with trust-lsp fallback", async () => {
    const canonical = vscode.workspace.getConfiguration("trust");
    const target = vscode.ConfigurationTarget.Workspace;

    await canonical.update("runtime.executablePath", undefined, target);

    try {
      await canonical.update(
        "runtime.executablePath",
        "/tmp/trust-runtime-product",
        target
      );
      assert.strictEqual(
        getTrustConfiguration().get<string>("runtime.cli.path"),
        "/tmp/trust-runtime-product",
        "product-language trust.runtime.executablePath must take precedence"
      );
    } finally {
      await canonical.update("runtime.executablePath", undefined, target);
    }
  });
});
