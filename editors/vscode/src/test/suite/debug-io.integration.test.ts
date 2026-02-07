import * as assert from "assert";
import * as vscode from "vscode";

import {
  __testCreateDefaultConfigurationAuto,
  selectWorkspaceFolderPathForMode,
} from "../../debug";
import {
  __testApplySettingsUpdate,
  __testCollectSettingsSnapshot,
} from "../../ioPanel";

async function pathExists(uri: vscode.Uri): Promise<boolean> {
  try {
    await vscode.workspace.fs.stat(uri);
    return true;
  } catch {
    return false;
  }
}

async function readText(uri: vscode.Uri): Promise<string> {
  const data = await vscode.workspace.fs.readFile(uri);
  return Buffer.from(data).toString("utf8");
}

suite("Debug/IO DRY flows", function () {
  this.timeout(60000);

  let fixturesRoot: vscode.Uri;
  let originalSettings: Record<string, unknown>;

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder for extension tests.");
    fixturesRoot = vscode.Uri.joinPath(
      workspaceFolder.uri,
      "tmp",
      "vscode-debug-io"
    );
    await vscode.workspace.fs.createDirectory(fixturesRoot);
    originalSettings = __testCollectSettingsSnapshot() as Record<string, unknown>;
  });

  suiteTeardown(async () => {
    await __testApplySettingsUpdate(originalSettings as any);
    try {
      await vscode.workspace.fs.delete(fixturesRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore teardown cleanup failures.
    }
  });

  test("unit: interactive vs auto folder selection", () => {
    assert.strictEqual(
      selectWorkspaceFolderPathForMode("interactive", ["/a", "/b"], undefined, "/b"),
      undefined
    );
    assert.strictEqual(
      selectWorkspaceFolderPathForMode("auto", ["/a", "/b"], undefined, "/b"),
      "/b"
    );
    assert.strictEqual(
      selectWorkspaceFolderPathForMode("auto", ["/a", "/b"], undefined, "/missing"),
      "/a"
    );
  });

  test("integration: auto default configuration creation", async () => {
    const projectRoot = vscode.Uri.joinPath(fixturesRoot, "auto-config-project");
    const srcRoot = vscode.Uri.joinPath(projectRoot, "src");
    await vscode.workspace.fs.createDirectory(srcRoot);
    const mainUri = vscode.Uri.joinPath(srcRoot, "Main.st");
    await vscode.workspace.fs.writeFile(
      mainUri,
      Buffer.from(
        [
          "PROGRAM Main",
          "VAR",
          "    run : BOOL := TRUE;",
          "END_VAR",
          "END_PROGRAM",
          "",
        ].join("\n"),
        "utf8"
      )
    );

    const created = await __testCreateDefaultConfigurationAuto("Main", mainUri);
    assert.ok(created, "Expected auto configuration creation to return a URI.");
    assert.ok(await pathExists(created!), "Expected created configuration file.");
    const text = await readText(created!);
    assert.ok(text.includes("CONFIGURATION Conf"));
    assert.ok(text.includes("PROGRAM P1 WITH MainTask : Main;"));
  });

  test("integration: settings update persists values", async () => {
    const payload = {
      serverPath: "/tmp/trust-lsp-test",
      traceServer: "messages",
      debugAdapterPath: "/tmp/trust-debug-test",
      debugAdapterArgs: ["--stdio"],
      debugAdapterEnv: { TRUST_TEST: "1" },
      runtimeControlEndpoint: "tcp://127.0.0.1:50123",
      runtimeControlAuthToken: "token-123",
      runtimeIncludeGlobs: ["**/*.st"],
      runtimeExcludeGlobs: ["**/generated/**"],
      runtimeIgnorePragmas: ["@ignore-me"],
      runtimeInlineValuesEnabled: false,
    };

    await __testApplySettingsUpdate(payload as any);
    const snapshot = __testCollectSettingsSnapshot() as Record<string, unknown>;
    for (const [key, value] of Object.entries(payload)) {
      assert.deepStrictEqual(snapshot[key], value, `Mismatch for ${key}`);
    }
  });
});
