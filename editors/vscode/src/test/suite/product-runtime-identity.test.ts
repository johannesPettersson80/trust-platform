import * as assert from "assert";
import * as vscode from "vscode";

import { productRuntimeIdentity } from "../../productRuntimeIdentity";

suite("Product runtime identity", () => {
  test("reports the owning extension context and each resolved product binary", () => {
    const context = {
      extensionMode: vscode.ExtensionMode.Production,
      extensionPath: "C:\\isolated\\extensions\\trust-platform.trust-lsp-0.24.33",
      extension: { packageJSON: { version: "0.24.33" } },
    } as unknown as vscode.ExtensionContext;
    const calls: string[] = [];
    const identity = productRuntimeIdentity(
      context,
      (_context, binary, key) => {
        calls.push(`${binary}:${key}`);
        return `C:\\isolated\\bin\\${binary}.exe`;
      },
      "C:\\isolated\\bin\\active-trust-lsp.exe"
    );

    assert.strictEqual(identity.schemaVersion, 1);
    assert.strictEqual(identity.extensionMode, vscode.ExtensionMode.Production);
    assert.strictEqual(identity.extensionPath, context.extensionPath);
    assert.strictEqual(identity.extensionVersion, "0.24.33");
    assert.deepStrictEqual(identity.binaries, {
      languageServer: "C:\\isolated\\bin\\active-trust-lsp.exe",
      debugAdapter: "C:\\isolated\\bin\\trust-debug.exe",
      runtime: "C:\\isolated\\bin\\trust-runtime.exe",
    });
    assert.deepStrictEqual(calls, [
      "trust-debug:debug.adapter.path",
      "trust-runtime:runtime.cli.path",
    ]);
  });
});
