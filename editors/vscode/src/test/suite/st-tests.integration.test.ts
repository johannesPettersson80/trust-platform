import * as assert from "assert";
import * as vscode from "vscode";

type RuntimeSummary = {
  total: number;
  passed: number;
  failed: number;
  errors: number;
};

type RuntimeCase = {
  name: string;
  kind: string;
  status: string;
  file: string;
  line: number;
  message?: string | null;
};

type RuntimePayload = {
  version: number;
  project: string;
  summary: RuntimeSummary;
  tests: RuntimeCase[];
};

type LastResultEntry = {
  uri: string;
  line: number;
  name: string;
  kind: string;
  status: string;
  message?: string;
};

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

suite("ST test workflow integration (VS Code)", function () {
  this.timeout(30000);

  let fixturesRoot: vscode.Uri;
  let projectRoot: vscode.Uri;
  let testUri: vscode.Uri;

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected a workspace folder for tests.");

    fixturesRoot = vscode.Uri.joinPath(workspaceFolder.uri, "tmp", "vscode-st-tests");
    await vscode.workspace.fs.createDirectory(fixturesRoot);
    projectRoot = vscode.Uri.joinPath(fixturesRoot, "project");
    await vscode.workspace.fs.createDirectory(projectRoot);
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(projectRoot, "src"));

    const runtimeBin = process.env.ST_RUNTIME_TEST_BIN;
    if (runtimeBin && runtimeBin.trim().length > 0) {
      await vscode.workspace
        .getConfiguration("trust")
        .update(
          "runtime.executablePath",
          runtimeBin,
          vscode.ConfigurationTarget.Workspace
        );
    }
    const devBin = process.env.ST_DEV_TEST_BIN;
    if (devBin && devBin.trim().length > 0) {
      await vscode.workspace
        .getConfiguration("trust")
        .update(
          "testRunner.executablePath",
          devBin,
          vscode.ConfigurationTarget.Workspace
        );
    }

    const source = [
      "TEST_PROGRAM Pass_Case",
      "ASSERT_TRUE(TRUE);",
      "END_TEST_PROGRAM",
      "",
      "TEST_PROGRAM Fail_Case",
      "ASSERT_EQUAL(INT#1, INT#2);",
      "END_TEST_PROGRAM",
      "",
      "TEST_FUNCTION_BLOCK Pass_Fb",
      "ASSERT_FALSE(FALSE);",
      "END_TEST_FUNCTION_BLOCK",
      "",
    ].join("\n");

    testUri = vscode.Uri.joinPath(projectRoot, "src", "tests.st");
    await vscode.workspace.fs.writeFile(testUri, Buffer.from(source, "utf8"));

    const doc = await vscode.workspace.openTextDocument(testUri);
    await vscode.window.showTextDocument(doc);
    await delay(300);
  });

  suiteTeardown(async () => {
    try {
      await vscode.workspace.fs.delete(fixturesRoot, {
        recursive: true,
        useTrash: false,
      });
    } catch {
      // Ignore cleanup failures in test teardown.
    }
  });

  test("discovers TEST_PROGRAM and TEST_FUNCTION_BLOCK declarations", async () => {
    const discovered = (await vscode.commands.executeCommand(
      "trust-lsp.test._discover",
      { uri: testUri.toString() }
    )) as Array<{ name: string; kind: string; line: number }> | undefined;

    assert.ok(discovered && discovered.length === 3, "Expected 3 discovered ST tests.");
    assert.ok(
      discovered?.some((entry) => entry.name === "Pass_Case" && entry.kind === "TEST_PROGRAM"),
      "Expected Pass_Case TEST_PROGRAM discovery."
    );
    assert.ok(
      discovered?.some((entry) => entry.name === "Pass_Fb" && entry.kind === "TEST_FUNCTION_BLOCK"),
      "Expected Pass_Fb TEST_FUNCTION_BLOCK discovery."
    );
  });

  test("run all and run single commands execute expected tests", async () => {
    const all = (await vscode.commands.executeCommand(
      "trust-lsp.test.runAll",
      {
        projectUri: projectRoot.toString(),
      }
    )) as RuntimePayload | undefined;

    assert.ok(all, "Expected run-all payload.");
    assert.strictEqual(all?.summary.total, 3);
    assert.strictEqual(all?.summary.passed, 2);
    assert.strictEqual(all?.summary.failed, 1);
    assert.strictEqual(all?.summary.errors, 0);

    const single = (await vscode.commands.executeCommand(
      "trust-lsp.test.runOne",
      {
        uri: testUri.toString(),
        line: 1,
        kind: "TEST_PROGRAM",
        name: "Pass_Case",
      }
    )) as RuntimeCase | undefined;

    assert.ok(single, "Expected run-single payload.");
    assert.strictEqual(single?.name, "Pass_Case");
    assert.strictEqual(single?.status, "passed");
  });

  test("state updates track pass/fail results for UI decorations", async () => {
    const state = (await vscode.commands.executeCommand(
      "trust-lsp.test._state"
    )) as LastResultEntry[] | undefined;

    assert.ok(state && state.length >= 2, "Expected last-result state entries.");
    const pass = state?.find((entry) => entry.name === "Pass_Case");
    const fail = state?.find((entry) => entry.name === "Fail_Case");
    assert.ok(pass, "Expected Pass_Case in state.");
    assert.ok(fail, "Expected Fail_Case in state.");
    assert.strictEqual(pass?.status, "passed");
    assert.strictEqual(fail?.status, "failed");
  });

  test("refresh preserves results for tests that still exist", async () => {
    await vscode.commands.executeCommand("trust-lsp.test.runOne", {
      uri: testUri.toString(),
      line: 1,
      kind: "TEST_PROGRAM",
      name: "Pass_Case",
    });

    await vscode.commands.executeCommand("testing.refreshTests");
    await delay(500);

    const state = (await vscode.commands.executeCommand(
      "trust-lsp.test._state"
    )) as LastResultEntry[] | undefined;
    const pass = state?.find((entry) => entry.name === "Pass_Case");
    assert.ok(pass, "Expected Pass_Case state to survive a test refresh.");
    assert.strictEqual(pass?.status, "passed");
  });

  test("refresh clears stale results when a file no longer contains tests", async () => {
    const source = [
      "(*",
      "  Add a TEST_PROGRAM or TEST_FUNCTION_BLOCK here to create ST tests.",
      "*)",
      "",
    ].join("\n");
    await vscode.workspace.fs.writeFile(testUri, Buffer.from(source, "utf8"));

    await vscode.commands.executeCommand("testing.refreshTests");
    await delay(500);

    const discovered = (await vscode.commands.executeCommand(
      "trust-lsp.test._discover",
      { uri: testUri.toString() }
    )) as Array<{ name: string; kind: string; line: number }> | undefined;
    assert.deepStrictEqual(discovered, [], "Expected no ST tests after rewriting the file.");

    const state = (await vscode.commands.executeCommand(
      "trust-lsp.test._state"
    )) as LastResultEntry[] | undefined;
    assert.deepStrictEqual(
      state,
      [],
      "Refreshing tests after removing TEST_* declarations must clear stale PASS/FAIL state."
    );
  });
});
