import * as assert from "assert";
import { spawnSync } from "child_process";
import * as vscode from "vscode";

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

async function deleteTreeIfExists(uri: vscode.Uri): Promise<void> {
  try {
    await vscode.workspace.fs.delete(uri, {
      recursive: true,
      useTrash: false,
    });
  } catch {
    // Test cleanup only.
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

suite("New project command (VS Code)", function () {
  this.timeout(60000);

  test("starter handoff clears a stale example README landing", async () => {
    const api = require("../../newProject") as {
      prepareStarterProjectHandoff?: (context: unknown, mainPath: string) => Promise<void>;
    };
    assert.strictEqual(typeof api.prepareStarterProjectHandoff, "function");
    if (!api.prepareStarterProjectHandoff) return;
    const updates: Array<[string, unknown]> = [];
    await api.prepareStarterProjectHandoff({
      globalState: { update: async (key: string, value: unknown) => updates.push([key, value]) },
    }, "/tmp/starter/src/Main.st");
    assert.strictEqual(updates[0]?.[0], "trust.projectLanding");
    assert.deepStrictEqual(updates[0]?.[1], {
      intentId: updates[0]?.[1] && (updates[0][1] as { intentId?: string }).intentId,
      projectPath: "/tmp/starter",
      kind: "starter",
      filePath: "/tmp/starter/src/Main.st",
    });
    assert.ok(typeof (updates[0]?.[1] as { intentId?: unknown }).intentId === "string");
    assert.deepStrictEqual(updates.slice(1), [
      ["trust.newProject.focusMain", undefined],
      ["trust.examples.openLessonReadme", undefined],
    ]);
  });

  test("structured landing dispatcher scopes and retains only an unrelated intent", async () => {
    const api = require("../../projectLanding") as {
      consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }>;
    };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const updates: Array<[string, unknown]> = [];
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-1", projectPath: "/tmp/project-a", kind: "starter", filePath: "/tmp",
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { updates.push([key, value]); state.set(key, value); },
    }};
    const result = await api.consumeProjectLanding(context);
    assert.strictEqual(result.kind, "waiting");
    assert.deepStrictEqual(updates, []);
    assert.ok(state.get("trust.projectLanding"));
  });

  test("structured landing dispatcher concurrently consumes one legacy lesson handoff", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "legacy-lesson");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Legacy lesson\n"));
    const state = new Map<string, unknown>([["trust.examples.openLessonReadme", readme.fsPath]]);
    const updates: Array<[string, unknown]> = [];
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { updates.push([key, value]); state.set(key, value); },
    }};
    try {
      const results = await Promise.all([
        api.consumeProjectLanding(context),
        api.consumeProjectLanding(context),
      ]);
      assert.deepStrictEqual(results.map((result) => result.kind), ["opened", "opened"]);
      assert.strictEqual(updates.filter(([key, value]) => key === "trust.examples.openLessonReadme" && value === undefined).length, 1);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher retains a newer intent after opening the older one", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "post-open-supersede");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Superseded lesson\n"));
    const newer = { intentId: "intent-newer", projectPath: project.fsPath, kind: "lesson", filePath: vscode.Uri.joinPath(project, "Other.md").fsPath, exampleId: "newer" };
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-older", projectPath: project.fsPath, kind: "lesson", filePath: readme.fsPath, exampleId: "older",
    }]]);
    const originalExecute = vscode.commands.executeCommand;
    (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = async <T>() => {
      state.set("trust.projectLanding", newer);
      return undefined as T;
    };
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    try {
      assert.strictEqual((await api.consumeProjectLanding(context)).kind, "opened");
      assert.deepStrictEqual(state.get("trust.projectLanding"), newer);
    } finally {
      (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = originalExecute;
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher opens one matching starter and clears its intent", async () => {
    const api = require("../../projectLanding") as {
      consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }>;
    };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const mainUri = vscode.Uri.joinPath(fixturesRoot, "project-b", "src", "Main.st");
    // VS Code keeps this opened buffer live across later tests even after its
    // temporary path is deleted. Give the dispatcher-only fixture its own POU
    // identity so it cannot pollute scaffold diagnostics in the shared host.
    await vscode.workspace.fs.writeFile(mainUri, Buffer.from("PROGRAM LandingFixture\nEND_PROGRAM\n"));
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-2", projectPath: vscode.Uri.joinPath(fixturesRoot, "project-b").fsPath, kind: "starter", filePath: mainUri.fsPath,
    }]]);
    const updates: Array<[string, unknown]> = [];
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { updates.push([key, value]); state.set(key, value); },
    }};
    try {
      const result = await api.consumeProjectLanding(context);
      assert.strictEqual(result.kind, "opened");
      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), mainUri.toString());
      assert.deepStrictEqual(updates, [["trust.projectLanding", undefined]]);
    } finally {
      await vscode.workspace.fs.delete(vscode.Uri.joinPath(fixturesRoot, "project-b"), { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher retains an intent outside its exact destination project", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const file = vscode.Uri.joinPath(fixturesRoot, "outside-scope", "src", "Main.st");
    await vscode.workspace.fs.writeFile(file, Buffer.from("PROGRAM Main\nEND_PROGRAM\n"));
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-scope", projectPath: "/tmp/not-this-workspace", kind: "starter", filePath: file.fsPath,
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    try {
      assert.strictEqual((await api.consumeProjectLanding(context)).kind, "waiting");
      assert.ok(state.get("trust.projectLanding"));
    } finally {
      await vscode.workspace.fs.delete(vscode.Uri.joinPath(fixturesRoot, "outside-scope"), { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher does not clear a missing file from an unrelated activation", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-missing-scope", projectPath: "/tmp/not-this-workspace", kind: "starter", filePath: "/tmp/not-this-workspace/src/Missing.st",
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    assert.strictEqual((await api.consumeProjectLanding(context)).kind, "waiting");
    assert.ok(state.get("trust.projectLanding"));
  });

  test("structured landing dispatcher clears a missing file only in its destination project with recovery guidance", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string; message?: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "missing-destination");
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-missing-local", projectPath: project.fsPath, kind: "starter",
      filePath: vscode.Uri.joinPath(project, "src", "Missing.st").fsPath,
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    const result = await api.consumeProjectLanding(context);
    assert.strictEqual(result.kind, "cleared");
    assert.match(result.message ?? "", /recreate|copy.*again/i);
    assert.strictEqual(state.get("trust.projectLanding"), undefined);
  });

  test("structured landing dispatcher retains an open failure and exposes retry guidance", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string; message?: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "open-failure");
    const directory = vscode.Uri.joinPath(project, "src");
    await vscode.workspace.fs.createDirectory(directory);
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-open-failure", projectPath: project.fsPath, kind: "starter", filePath: directory.fsPath,
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    try {
      const result = await api.consumeProjectLanding(context);
      assert.strictEqual(result.kind, "failed");
      assert.match(result.message ?? "", /retry/i);
      assert.ok(state.get("trust.projectLanding"));
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher rechecks intent identity before opening an older file", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "superseded-landing");
    const oldFile = vscode.Uri.joinPath(project, "src", "Old.st");
    await vscode.workspace.fs.writeFile(oldFile, Buffer.from("PROGRAM Old\nEND_PROGRAM\n"));
    const oldIntent = { intentId: "intent-old", projectPath: project.fsPath, kind: "starter", filePath: oldFile.fsPath };
    const newIntent = { intentId: "intent-new", projectPath: project.fsPath, kind: "starter", filePath: vscode.Uri.joinPath(project, "src", "New.st").fsPath };
    let reads = 0;
    const state = new Map<string, unknown>([["trust.projectLanding", oldIntent]]);
    const context = { globalState: {
      get: <T>(key: string) => {
        if (key === "trust.projectLanding" && reads++ === 1) state.set(key, newIntent);
        return state.get(key) as T;
      },
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    try {
      assert.strictEqual((await api.consumeProjectLanding(context)).kind, "waiting");
      assert.deepStrictEqual(state.get("trust.projectLanding"), newIntent);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher opens a lesson README through the native markdown command", async () => {
    const api = require("../../projectLanding") as { consumeProjectLanding?: (context: unknown) => Promise<{ kind: string }> };
    assert.strictEqual(typeof api.consumeProjectLanding, "function");
    if (!api.consumeProjectLanding) return;
    const project = vscode.Uri.joinPath(fixturesRoot, "lesson-landing");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Native lesson landing\n"));
    const state = new Map<string, unknown>([["trust.projectLanding", {
      intentId: "intent-lesson", projectPath: project.fsPath, kind: "lesson", filePath: readme.fsPath,
      exampleId: "control-lesson",
    }]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    }};
    try {
      assert.strictEqual((await api.consumeProjectLanding(context)).kind, "opened");
      assert.strictEqual(state.get("trust.projectLanding"), undefined);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });
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

  // Keep the clean-first-open assertion ahead of tests that deliberately
  // create and remove other Main/Config scaffolds in this shared Extension
  // Host. Recursive fixture deletion does not retract every already-indexed
  // child declaration from the running VS Code/LSP session.
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
    assert.ok(
      runtimeBin,
      "ST_RUNTIME_TEST_BIN must be provided by the extension test harness."
    );

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
    const legacyMainUri = vscode.Uri.joinPath(targetUri, "sources", "Main.st");
    const tomlUri = vscode.Uri.joinPath(targetUri, "trust-lsp.toml");
    assert.strictEqual(await pathExists(mainUri), true, "Expected Main.st file.");
    assert.strictEqual(
      await pathExists(legacyMainUri),
      false,
      "Expected no legacy sources/ scaffold."
    );
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

    // §0.5.15 — a RUNNABLE simulator project also writes runtime.toml + io.toml so Start works and
    // Devices & Connections can load the offline topology with zero manual TOML editing.
    const runtimeTomlUri = vscode.Uri.joinPath(targetUri, "runtime.toml");
    const ioTomlUri = vscode.Uri.joinPath(targetUri, "io.toml");
    assert.strictEqual(
      await pathExists(runtimeTomlUri),
      true,
      "Expected runtime.toml in the runnable scaffold."
    );
    assert.strictEqual(
      await pathExists(ioTomlUri),
      true,
      "Expected io.toml in the runnable scaffold."
    );
    // F-02 — a CONFIGURATION instantiates Main so a brand-new project opens clean (no W009 "unused
    // program" lint). Without the program instance the LSP flags Main on first open.
    const configUri = vscode.Uri.joinPath(targetUri, "src", "config.st");
    assert.strictEqual(
      await pathExists(configUri),
      true,
      "Expected src/config.st in the scaffold."
    );
    const configSource = await readText(configUri);
    assert.ok(
      /PROGRAM\s+MainInstance\s+WITH\s+\w+\s*:\s*Main/.test(configSource),
      "config.st must instantiate Main with a distinct instance name so it is not flagged unused or duplicated (F-02)."
    );
    const runtimeToml = await readText(runtimeTomlUri);
    assert.ok(
      runtimeToml.includes("[runtime.control]"),
      "runtime.toml must declare [runtime.control]."
    );
    // The runtime config parser REQUIRES these sections — a compact file is rejected (Codex review).
    for (const section of ["[runtime.retain]", "[runtime.watchdog]", "[runtime.fault]"]) {
      assert.ok(
        runtimeToml.includes(section),
        `runtime.toml must declare ${section} or the runtime parser rejects the project.`
      );
    }
    // The runtime runs the configuration's resource that runtime.toml names, so the two agree.
    const resource = /RESOURCE\s+(\w+)\s+ON/.exec(configSource)?.[1];
    assert.ok(resource, "config.st declares a resource");
    assert.match(runtimeToml, new RegExp(`\\[resource\\]\\nname = "${resource}"`));
    // The runtime refuses a token that is present but empty, so the disabled mesh declares none.
    assert.doesNotMatch(runtimeToml, /auth_token\s*=\s*""/, "a new project's runtime starts as written");
    // The HMI can be opened on this computer at once, and commissioned values survive a restart.
    assert.match(runtimeToml, /\[runtime\.web\]\nenabled = true\nlisten = "127\.0\.0\.1:8080"/);
    assert.match(runtimeToml, /\[runtime\.retain\]\nmode = "file"\npath = "retain\.bin"/);
    const ioToml = await readText(ioTomlUri);
    assert.ok(
      /driver\s*=\s*"simulated"/.test(ioToml),
      "io.toml must default to the simulated driver (runs with no hardware)."
    );
    await deleteTreeIfExists(targetUri);
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

    // A folder holding a project's manifest: a new project would replace it, so it asks.
    const existingTarget = vscode.Uri.joinPath(fixturesRoot, "cancel-at-overwrite");
    await vscode.workspace.fs.createDirectory(existingTarget);
    const sentinelUri = vscode.Uri.joinPath(existingTarget, "trust-lsp.toml");
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
    assert.strictEqual(Buffer.from(await vscode.workspace.fs.readFile(sentinelUri)).toString(), "keep-me");
    assert.strictEqual(await pathExists(vscode.Uri.joinPath(existingTarget, "src", "Main.st")), false);
  });

  test("a folder without truST files becomes a project without a question", async () => {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "plain-folder");
    await vscode.workspace.fs.createDirectory(targetUri);
    const otherUri = vscode.Uri.joinPath(targetUri, "notes.txt");
    await vscode.workspace.fs.writeFile(otherUri, Buffer.from("mine"));
    // No overwrite answer is given: a question would be refused by the test host.
    const created = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
      targetUri,
      openWorkspace: false,
    });
    assert.strictEqual(created, true);
    assert.strictEqual(Buffer.from(await vscode.workspace.fs.readFile(otherUri)).toString(), "mine");
    assert.strictEqual(await pathExists(vscode.Uri.joinPath(targetUri, "src", "Main.st")), true);
    await deleteTreeIfExists(targetUri);
  });

  test("existing target requires explicit confirmation behavior", async () => {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "existing-target");
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(targetUri, "src"));
    const ownMainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
    await vscode.workspace.fs.writeFile(ownMainUri, Buffer.from("PROGRAM Mine END_PROGRAM\n"));
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
      Buffer.from(await vscode.workspace.fs.readFile(ownMainUri)).toString(),
      "PROGRAM Mine END_PROGRAM\n",
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
    await deleteTreeIfExists(targetUri);
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
    assert.strictEqual(await pathExists(vscode.Uri.joinPath(multiRootTarget, "src", "Main.st")), true);
  });
});
