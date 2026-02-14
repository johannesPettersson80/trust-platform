import * as assert from "assert";
import { spawnSync } from "child_process";
import * as vscode from "vscode";
import { STHmiApplyPatchTool, STHmiGetLayoutTool } from "../../lm-tools";

const NEW_PROJECT_COMMAND = "trust-lsp.newProject";

type CompletionResult =
  | vscode.CompletionList
  | vscode.CompletionItem[]
  | undefined;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

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

async function waitForNoErrors(
  uri: vscode.Uri,
  timeoutMs = 10000
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const diagnostics = vscode.languages
      .getDiagnostics(uri)
      .filter((diag) => diag.severity === vscode.DiagnosticSeverity.Error);
    if (diagnostics.length === 0) {
      return;
    }
    await delay(200);
  }
  const diagnostics = vscode.languages
    .getDiagnostics(uri)
    .filter((diag) => diag.severity === vscode.DiagnosticSeverity.Error);
  assert.strictEqual(
    diagnostics.length,
    0,
    `Expected no diagnostics, got: ${diagnostics
      .map((diag) => diag.message)
      .join("; ")}`
  );
}

function completionItems(result: CompletionResult): vscode.CompletionItem[] {
  if (!result) {
    return [];
  }
  return Array.isArray(result) ? result : result.items;
}

function toolResultText(result: unknown): string {
  if (typeof result === "string") {
    return result;
  }
  if (!result || typeof result !== "object") {
    return String(result);
  }
  const objectResult = result as Record<string, unknown>;
  if (typeof objectResult.text === "string") {
    return objectResult.text;
  }
  const content = objectResult.content;
  if (Array.isArray(content)) {
    for (const entry of content) {
      if (
        entry &&
        typeof entry === "object" &&
        typeof (entry as { value?: unknown }).value === "string"
      ) {
        return (entry as { value: string }).value;
      }
    }
  }
  try {
    return JSON.stringify(result);
  } catch {
    return String(result);
  }
}

suite("New project command (VS Code)", function () {
  this.timeout(60000);
  let fixturesRoot: vscode.Uri;
  let secondWorkspaceRoot: vscode.Uri | undefined;

  suiteSetup(async () => {
    const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
    assert.ok(workspaceFolders.length > 0, "Expected a workspace folder for tests.");
    if (workspaceFolders.length > 1) {
      vscode.workspace.updateWorkspaceFolders(1, workspaceFolders.length - 1);
      await delay(200);
    }
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected primary workspace folder for tests.");
    fixturesRoot = vscode.Uri.joinPath(
      workspaceFolder.uri,
      "tmp",
      "vscode-new-project"
    );
    await vscode.workspace.fs.createDirectory(fixturesRoot);
  });

  suiteTeardown(async () => {
    if (secondWorkspaceRoot) {
      const index = vscode.workspace.workspaceFolders?.findIndex(
        (folder) => folder.uri.toString() === secondWorkspaceRoot?.toString()
      );
      if (index !== undefined && index >= 0) {
        vscode.workspace.updateWorkspaceFolders(index, 1);
      }
    }
    try {
      await vscode.workspace.fs.delete(fixturesRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore cleanup failures in test teardown.
    }
  });

  test("creates scaffold in an empty target directory", async () => {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "hello-project");
    const created = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri, openWorkspace: false }
    );
    assert.strictEqual(created, true, "Expected project command to succeed.");

    const mainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
    const tomlUri = vscode.Uri.joinPath(targetUri, "trust-lsp.toml");
    assert.strictEqual(await pathExists(mainUri), true, "Expected Main.st file.");
    assert.strictEqual(
      await pathExists(tomlUri),
      true,
      "Expected trust-lsp.toml file."
    );

    const source = await readText(mainUri);
    assert.ok(source.includes("PROGRAM Main"), "Expected PROGRAM scaffold.");
    const projectToml = await readText(tomlUri);
    assert.strictEqual(
      projectToml.trim(),
      'include_paths = ["src"]',
      "Expected include path scaffold."
    );
  });

  test("cancel at each prompt stage leaves filesystem unchanged", async () => {
    const cancelAtFolderTarget = vscode.Uri.joinPath(
      fixturesRoot,
      "cancel-at-folder"
    );
    const cancelledAtFolder = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        baseUri: fixturesRoot,
        projectName: "cancel-at-folder",
        simulateCancelAt: "folder",
        openWorkspace: false,
      }
    );
    assert.strictEqual(cancelledAtFolder, false);
    assert.strictEqual(await pathExists(cancelAtFolderTarget), false);

    const cancelAtNameTarget = vscode.Uri.joinPath(fixturesRoot, "cancel-at-name");
    const cancelledAtName = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        baseUri: fixturesRoot,
        projectName: "cancel-at-name",
        simulateCancelAt: "name",
        openWorkspace: false,
      }
    );
    assert.strictEqual(cancelledAtName, false);
    assert.strictEqual(await pathExists(cancelAtNameTarget), false);

    const existingTarget = vscode.Uri.joinPath(fixturesRoot, "cancel-at-overwrite");
    await vscode.workspace.fs.createDirectory(existingTarget);
    const sentinelUri = vscode.Uri.joinPath(existingTarget, "sentinel.txt");
    await vscode.workspace.fs.writeFile(sentinelUri, Buffer.from("keep-me"));

    const cancelledAtOverwrite = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        targetUri: existingTarget,
        simulateCancelAt: "overwrite",
        openWorkspace: false,
      }
    );
    assert.strictEqual(cancelledAtOverwrite, false);
    assert.strictEqual(await pathExists(sentinelUri), true);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(existingTarget, "src", "Main.st")),
      false
    );
  });

  test("existing target requires explicit confirmation behavior", async () => {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "existing-target");
    await vscode.workspace.fs.createDirectory(targetUri);
    const sentinelUri = vscode.Uri.joinPath(targetUri, "keep.txt");
    await vscode.workspace.fs.writeFile(sentinelUri, Buffer.from("do-not-delete"));

    const declined = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        targetUri,
        overwrite: false,
        openWorkspace: false,
      }
    );
    assert.strictEqual(declined, false, "Expected explicit decline to stop.");
    assert.strictEqual(await pathExists(sentinelUri), true);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(targetUri, "src", "Main.st")),
      false
    );

    const accepted = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        targetUri,
        overwrite: true,
        openWorkspace: false,
      }
    );
    assert.strictEqual(accepted, true, "Expected explicit confirm to continue.");
    assert.strictEqual(await pathExists(sentinelUri), true);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(targetUri, "src", "Main.st")),
      true
    );
  });

  test("generated ST parses cleanly and TOML is usable by build", async function () {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "compile-smoke-project");
    const created = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri, openWorkspace: false }
    );
    assert.strictEqual(created, true);

    const mainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
    const mainDoc = await vscode.workspace.openTextDocument(mainUri);
    await vscode.window.showTextDocument(mainDoc);
    await waitForNoErrors(mainUri);

    const completion = (await vscode.commands.executeCommand(
      "vscode.executeCompletionItemProvider",
      mainUri,
      new vscode.Position(0, 0)
    )) as CompletionResult;
    assert.ok(
      completionItems(completion).length > 0,
      "Expected completion provider to respond on generated file."
    );

    const runtimeBin = process.env.ST_RUNTIME_TEST_BIN;
    if (!runtimeBin) {
      this.skip();
      return;
    }

    const buildResult = spawnSync(
      runtimeBin,
      ["build", "--project", targetUri.fsPath],
      {
        encoding: "utf8",
      }
    );
    assert.strictEqual(
      buildResult.status,
      0,
      [
        "trust-runtime build failed.",
        buildResult.stdout ?? "",
        buildResult.stderr ?? "",
      ].join("\n")
    );
  });

  test("works in single-root and multi-root workspace setups", async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected primary workspace folder.");
    const singleRootTarget = vscode.Uri.joinPath(
      fixturesRoot,
      "single-root-project"
    );
    const createdSingleRoot = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri: singleRootTarget, openWorkspace: false }
    );
    assert.strictEqual(createdSingleRoot, true);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(singleRootTarget, "src", "Main.st")),
      true
    );

    secondWorkspaceRoot = vscode.Uri.joinPath(fixturesRoot, "workspace-root-b");
    await vscode.workspace.fs.createDirectory(secondWorkspaceRoot);
    const added = vscode.workspace.updateWorkspaceFolders(
      vscode.workspace.workspaceFolders?.length ?? 0,
      0,
      {
        name: "secondary-root",
        uri: secondWorkspaceRoot,
      }
    );
    if (!added) {
      const alreadyPresent = (vscode.workspace.workspaceFolders ?? []).some(
        (folder) => folder.uri.toString() === secondWorkspaceRoot?.toString()
      );
      assert.ok(
        alreadyPresent,
        "Expected second workspace root to be added or already present."
      );
    }
    await delay(200);

    const multiRootTarget = vscode.Uri.joinPath(
      secondWorkspaceRoot,
      "multi-root-project"
    );
    const createdMultiRoot = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri: multiRootTarget, openWorkspace: false }
    );
    assert.strictEqual(createdMultiRoot, true);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(multiRootTarget, "src", "Main.st")),
      true
    );

    const patchTool = new STHmiApplyPatchTool();
    const layoutTool = new STHmiGetLayoutTool();
    const tokenSource = new vscode.CancellationTokenSource();
    const hmiFile = `multi-root-hmi-${Date.now()}.toml`;

    const patchResult = await patchTool.invoke(
      {
        input: {
          dry_run: false,
          rootPath: secondWorkspaceRoot.fsPath,
          operations: [
            {
              op: "add",
              path: `/files/${hmiFile}`,
              value: 'title = "Secondary HMI"\n',
            },
          ],
        },
      },
      tokenSource.token
    );
    const patchPayload = JSON.parse(toolResultText(patchResult));
    assert.strictEqual(patchPayload.ok, true);
    assert.strictEqual(patchPayload.rootPath, secondWorkspaceRoot.fsPath);
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(secondWorkspaceRoot, "hmi", hmiFile)),
      true
    );
    assert.strictEqual(
      await pathExists(vscode.Uri.joinPath(workspaceFolder.uri, "hmi", hmiFile)),
      false
    );

    const layoutResult = await layoutTool.invoke(
      { input: { rootPath: secondWorkspaceRoot.fsPath } },
      tokenSource.token
    );
    const layoutPayload = JSON.parse(toolResultText(layoutResult));
    assert.strictEqual(layoutPayload.exists, true);
    assert.strictEqual(layoutPayload.rootPath, secondWorkspaceRoot.fsPath);
    assert.ok(
      Array.isArray(layoutPayload.files) &&
        layoutPayload.files.some((entry: { name?: string }) => entry.name === hmiFile)
    );
  });
});
