import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { CHECK_PROGRAM_COMMAND } from "../../checkProgram";

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForDiagnostics(
  uri: vscode.Uri,
  predicate: (diagnostics: vscode.Diagnostic[]) => boolean,
  timeoutMs = 10000
): Promise<vscode.Diagnostic[]> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const diagnostics = vscode.languages.getDiagnostics(uri);
    if (predicate(diagnostics)) {
      return diagnostics;
    }
    await delay(200);
  }
  const diagnostics = vscode.languages.getDiagnostics(uri);
  throw new Error(
    `Timed out waiting for diagnostics. Last diagnostics: ${diagnostics
      .map((diag) => `${diag.source ?? ""} ${diag.message}`.trim())
      .join("; ")}`
  );
}

suite("Compile diagnostics integration (VS Code)", function () {
  this.timeout(30000);

  let projectRoot: vscode.Uri;
  let runtimeToml: vscode.Uri;

  suiteSetup(async () => {
    const projectPath = fs.mkdtempSync(
      path.join(os.tmpdir(), "trust-compile-diagnostics-")
    );
    projectRoot = vscode.Uri.file(projectPath);
    runtimeToml = vscode.Uri.joinPath(projectRoot, "runtime.toml");
    const runtimeBin = process.env.ST_RUNTIME_TEST_BIN;
    if (runtimeBin && runtimeBin.trim().length > 0) {
      await vscode.workspace
        .getConfiguration("trust-lsp")
        .update(
          "runtime.cli.path",
          runtimeBin,
          vscode.ConfigurationTarget.Workspace
        );
    }

    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(projectRoot, "src"));
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(projectRoot, "trust-lsp.toml"),
      Buffer.from('include_paths = ["src"]\n', "utf8")
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(projectRoot, "io.toml"),
      Buffer.from('[io]\ndriver = "simulated"\nparams = {}\n', "utf8")
    );
    await vscode.workspace.fs.writeFile(
      vscode.Uri.joinPath(projectRoot, "src", "main.st"),
      Buffer.from("PROGRAM Main\nEND_PROGRAM\n", "utf8")
    );
    await vscode.workspace.fs.writeFile(
      runtimeToml,
      Buffer.from("[runtime]\nextra =\n", "utf8")
    );

    const doc = await vscode.workspace.openTextDocument(runtimeToml);
    await vscode.window.showTextDocument(doc);
  });

  suiteTeardown(async () => {
    try {
      await vscode.workspace.fs.delete(projectRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore cleanup failures in test teardown.
    }
  });

  test("Compile diagnostics use the truST Problems source", async () => {
    const response = (await vscode.commands.executeCommand(
      CHECK_PROGRAM_COMMAND
    )) as { ok: boolean } | undefined;
    assert.ok(response, "Expected Compile command to return a report.");
    assert.strictEqual(response?.ok, false);

    const diagnostics = await waitForDiagnostics(
      runtimeToml,
      (items) => items.some((diag) => diag.source === "truST")
    );
    const runtimeDiagnostic = diagnostics.find((diag) =>
      diag.message.includes("runtime.toml is invalid")
    );
    assert.ok(runtimeDiagnostic, "Expected a runtime.toml config diagnostic.");
    assert.strictEqual(runtimeDiagnostic?.source, "truST");
  });
});
