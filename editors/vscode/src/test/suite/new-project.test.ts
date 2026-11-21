import * as assert from "assert";
import { spawnSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { PROJECT_LANDING_FILE, consumeProjectLanding, prepareStarterProjectHandoff } from "../../projectLanding";

const NEW_PROJECT_COMMAND = "trust-lsp.newProject";

type CompletionResult =
  | vscode.CompletionList
  | vscode.CompletionItem[]
  | undefined;

/** The storage folders of the landing tests' contexts, removed when the suite ends. */
const landingStorages: string[] = [];

/** A landing dispatcher's context: the pending intent in its file, in a storage folder of the test's own, and the keys
 * an earlier version used in `state`, each write recorded in `updates`. `landing()` reads the file back, and
 * `publish(intent)` stands for another window replacing it. */
function landingContext(intent?: unknown, earlier: Array<[string, unknown]> = []) {
  const storage = fs.mkdtempSync(path.join(os.tmpdir(), "trust-landing-"));
  landingStorages.push(storage);
  const file = path.join(storage, PROJECT_LANDING_FILE);
  const publish = (next: unknown): void => fs.writeFileSync(file, JSON.stringify(next));
  if (intent !== undefined) publish(intent);
  const state = new Map<string, unknown>(earlier);
  const updates: Array<[string, unknown]> = [];
  const context = {
    globalStorageUri: vscode.Uri.file(storage),
    globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { updates.push([key, value]); state.set(key, value); },
    },
  } as unknown as vscode.ExtensionContext;
  const landing = (): unknown => (fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, "utf8")) : undefined);
  return { context, state, updates, landing, publish };
}

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

/** Resolves once VS Code holds the language server's first diagnostics of `uri`. Before then a file has no
 * diagnostics whatever it holds, so a check of them proves nothing. */
function firstDiagnostics(uri: vscode.Uri, timeoutMs = 10000): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let listener: vscode.Disposable | undefined;
  return new Promise<void>((resolve, reject) => {
    listener = vscode.languages.onDidChangeDiagnostics((event) => {
      if (event.uris.some((changed) => changed.fsPath === uri.fsPath)) resolve();
    });
    timer = setTimeout(
      () => reject(new Error(`The language server gave no diagnostics for ${uri.fsPath} within ${timeoutMs} ms.`)),
      timeoutMs
    );
  }).finally(() => {
    clearTimeout(timer);
    listener?.dispose();
  });
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
    // Spec 25 §3.2: creating a starter schedules its src/Main.st in the intent's file, replacing a lesson's README
    // landing, and clears the README and structured keys an earlier version kept. Catches a handoff that leaves the
    // README to open instead of the starter.
    const { context, state, landing } = landingContext(
      { intentId: "intent-readme", projectPath: "/tmp/lesson", kind: "lesson", filePath: "/tmp/lesson/README.md", exampleId: "lesson" },
      [
        ["trust.examples.openLessonReadme", "/tmp/lesson/README.md"],
        ["trust.projectLanding", { intentId: "intent-earlier-version", projectPath: "/tmp/lesson", kind: "lesson", filePath: "/tmp/lesson/README.md" }],
      ],
    );
    await prepareStarterProjectHandoff(context, "/tmp/starter/src/Main.st");
    const intent = landing() as { intentId?: unknown } | undefined;
    assert.strictEqual(typeof intent?.intentId, "string");
    assert.deepStrictEqual(intent, {
      intentId: intent?.intentId,
      projectPath: "/tmp/starter",
      kind: "starter",
      filePath: "/tmp/starter/src/Main.st",
    });
    assert.strictEqual(state.get("trust.examples.openLessonReadme"), undefined);
    assert.strictEqual(state.get("trust.projectLanding"), undefined);
  });

  test("a landing lands although globalState shows the value from before a write (checklist W49)", async () => {
    // Spec 25 §3.2: the intent reads back as it was last written, in the window that wrote it. VS Code 1.147's
    // globalState shows the value from before a write for about 150 ms when the write comes 100 to 200 ms after an
    // earlier one, so a Rust start created then did not land on its crate 1 time in 32 (W49). Here globalState takes
    // every write and keeps showing what it held before: catches an intent kept in globalState again.
    const project = vscode.Uri.joinPath(fixturesRoot, "earlier-value");
    const mainUri = vscode.Uri.joinPath(project, "src", "Main.st");
    await vscode.workspace.fs.writeFile(mainUri, Buffer.from("PROGRAM EarlierValue\nEND_PROGRAM\n"));
    const { context, landing } = landingContext();
    (context.globalState as unknown as { update: unknown }).update = async () => undefined;
    try {
      await prepareStarterProjectHandoff(context, mainUri.fsPath);
      assert.strictEqual((await consumeProjectLanding(context)).kind, "opened");
      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), mainUri.toString());
      assert.strictEqual(landing(), undefined, "the landing clears its intent");
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("an intent an earlier version left in globalState lands, unless this version has written one since", async () => {
    // Spec 25 §3.2: the intent moves to the file at the next landing, and an intent of this version wins. Catches an
    // upgrade that loses a pending landing, or an earlier intent that replaces a newer one.
    const project = vscode.Uri.joinPath(fixturesRoot, "earlier-version");
    const mainUri = vscode.Uri.joinPath(project, "src", "Main.st");
    await vscode.workspace.fs.writeFile(mainUri, Buffer.from("PROGRAM EarlierVersion\nEND_PROGRAM\n"));
    const earlier = { intentId: "intent-earlier-version", projectPath: project.fsPath, kind: "starter", filePath: mainUri.fsPath };
    try {
      const upgraded = landingContext(undefined, [["trust.projectLanding", earlier]]);
      assert.strictEqual((await consumeProjectLanding(upgraded.context)).kind, "opened");
      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), mainUri.toString());
      assert.strictEqual(upgraded.state.get("trust.projectLanding"), undefined);
      assert.strictEqual(upgraded.landing(), undefined);

      const newer = { intentId: "intent-this-version", projectPath: "/tmp/not-this-workspace", kind: "starter", filePath: "/tmp/not-this-workspace/src/Main.st" };
      const both = landingContext(newer, [["trust.projectLanding", earlier]]);
      assert.strictEqual((await consumeProjectLanding(both.context)).kind, "waiting");
      assert.strictEqual(both.state.get("trust.projectLanding"), undefined);
      assert.deepStrictEqual(both.landing(), newer);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher scopes and retains only an unrelated intent", async () => {
    const { context, updates, landing } = landingContext({
      intentId: "intent-1", projectPath: "/tmp/project-a", kind: "starter", filePath: "/tmp",
    });
    const result = await consumeProjectLanding(context);
    assert.strictEqual(result.kind, "waiting");
    assert.deepStrictEqual(updates, []);
    assert.ok(landing());
  });

  test("structured landing dispatcher concurrently consumes one legacy lesson handoff", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "legacy-lesson");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Legacy lesson\n"));
    const { context, updates } = landingContext(undefined, [["trust.examples.openLessonReadme", readme.fsPath]]);
    try {
      const results = await Promise.all([
        consumeProjectLanding(context),
        consumeProjectLanding(context),
      ]);
      assert.deepStrictEqual(results.map((result) => result.kind), ["opened", "opened"]);
      assert.strictEqual(updates.filter(([key, value]) => key === "trust.examples.openLessonReadme" && value === undefined).length, 1);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher retains a newer intent after opening the older one", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "post-open-supersede");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Superseded lesson\n"));
    const newer = { intentId: "intent-newer", projectPath: project.fsPath, kind: "lesson", filePath: vscode.Uri.joinPath(project, "Other.md").fsPath, exampleId: "newer" };
    const { context, landing, publish } = landingContext({
      intentId: "intent-older", projectPath: project.fsPath, kind: "lesson", filePath: readme.fsPath, exampleId: "older",
    });
    const originalExecute = vscode.commands.executeCommand;
    // Only the preview the dispatcher opens: the extension's own background commands go through,
    // so that none of them stands for the preview.
    (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = async <T>(
      command: string,
      ...rest: unknown[]
    ) => {
      if (command !== "markdown.showPreview") return originalExecute<T>(command, ...rest);
      publish(newer);
      return undefined as T;
    };
    try {
      assert.strictEqual((await consumeProjectLanding(context)).kind, "opened");
      assert.deepStrictEqual(landing(), newer);
    } finally {
      (vscode.commands as unknown as { executeCommand: typeof vscode.commands.executeCommand }).executeCommand = originalExecute;
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher opens one matching starter and clears its intent", async () => {
    const mainUri = vscode.Uri.joinPath(fixturesRoot, "project-b", "src", "Main.st");
    // VS Code keeps this opened buffer live across later tests even after its
    // temporary path is deleted. Give the dispatcher-only fixture its own POU
    // identity so it cannot pollute scaffold diagnostics in the shared host.
    await vscode.workspace.fs.writeFile(mainUri, Buffer.from("PROGRAM LandingFixture\nEND_PROGRAM\n"));
    const { context, updates, landing } = landingContext({
      intentId: "intent-2", projectPath: vscode.Uri.joinPath(fixturesRoot, "project-b").fsPath, kind: "starter", filePath: mainUri.fsPath,
    });
    try {
      const result = await consumeProjectLanding(context);
      assert.strictEqual(result.kind, "opened");
      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.toString(), mainUri.toString());
      assert.strictEqual(landing(), undefined);
      assert.deepStrictEqual(updates, []);
    } finally {
      await vscode.workspace.fs.delete(vscode.Uri.joinPath(fixturesRoot, "project-b"), { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher retains an intent outside its exact destination project", async () => {
    const file = vscode.Uri.joinPath(fixturesRoot, "outside-scope", "src", "Main.st");
    await vscode.workspace.fs.writeFile(file, Buffer.from("PROGRAM Main\nEND_PROGRAM\n"));
    const { context, landing } = landingContext({
      intentId: "intent-scope", projectPath: "/tmp/not-this-workspace", kind: "starter", filePath: file.fsPath,
    });
    try {
      assert.strictEqual((await consumeProjectLanding(context)).kind, "waiting");
      assert.ok(landing());
    } finally {
      await vscode.workspace.fs.delete(vscode.Uri.joinPath(fixturesRoot, "outside-scope"), { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher does not clear a missing file from an unrelated activation", async () => {
    const { context, landing } = landingContext({
      intentId: "intent-missing-scope", projectPath: "/tmp/not-this-workspace", kind: "starter", filePath: "/tmp/not-this-workspace/src/Missing.st",
    });
    assert.strictEqual((await consumeProjectLanding(context)).kind, "waiting");
    assert.ok(landing());
  });

  test("structured landing dispatcher clears a missing file only in its destination project with recovery guidance", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "missing-destination");
    const { context, landing } = landingContext({
      intentId: "intent-missing-local", projectPath: project.fsPath, kind: "starter",
      filePath: vscode.Uri.joinPath(project, "src", "Missing.st").fsPath,
    });
    const result = await consumeProjectLanding(context);
    assert.strictEqual(result.kind, "cleared");
    assert.match((result as { message?: string }).message ?? "", /recreate|copy.*again/i);
    assert.strictEqual(landing(), undefined);
  });

  test("structured landing dispatcher retains an open failure and exposes retry guidance", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "open-failure");
    const directory = vscode.Uri.joinPath(project, "src");
    await vscode.workspace.fs.createDirectory(directory);
    const { context, landing } = landingContext({
      intentId: "intent-open-failure", projectPath: project.fsPath, kind: "starter", filePath: directory.fsPath,
    });
    try {
      const result = await consumeProjectLanding(context);
      assert.strictEqual(result.kind, "failed");
      assert.match((result as { message?: string }).message ?? "", /retry/i);
      assert.ok(landing());
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher rechecks intent identity before opening an older file", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "superseded-landing");
    const oldFile = vscode.Uri.joinPath(project, "src", "Old.st");
    await vscode.workspace.fs.writeFile(oldFile, Buffer.from("PROGRAM Old\nEND_PROGRAM\n"));
    const oldIntent = { intentId: "intent-old", projectPath: project.fsPath, kind: "starter", filePath: oldFile.fsPath };
    const newIntent = { intentId: "intent-new", projectPath: project.fsPath, kind: "starter", filePath: vscode.Uri.joinPath(project, "src", "New.st").fsPath };
    const { context, landing, publish } = landingContext(oldIntent);
    try {
      // The dispatcher reads the intent before its first wait, the check of the file; another window's newer intent,
      // published then, is the one it finds when it reads again before it opens.
      const consuming = consumeProjectLanding(context);
      publish(newIntent);
      assert.strictEqual((await consuming).kind, "waiting");
      assert.deepStrictEqual(landing(), newIntent);
    } finally {
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });

  test("structured landing dispatcher opens a lesson README through the native markdown command", async () => {
    const project = vscode.Uri.joinPath(fixturesRoot, "lesson-landing");
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Native lesson landing\n"));
    const { context, landing } = landingContext({
      intentId: "intent-lesson", projectPath: project.fsPath, kind: "lesson", filePath: readme.fsPath,
      exampleId: "control-lesson",
    });
    try {
      assert.strictEqual((await consumeProjectLanding(context)).kind, "opened");
      assert.strictEqual(landing(), undefined);
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
    const mainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
    // The command opens src/Main.st itself, so the wait for the language server's answer starts before it runs.
    const answered = firstDiagnostics(mainUri);
    answered.catch(() => undefined); // Awaited below; a test that stops earlier leaves no unhandled rejection.
    const created = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri, openWorkspace: false }
    );
    assert.strictEqual(created, true);

    const mainDoc = await vscode.workspace.openTextDocument(mainUri);
    await vscode.window.showTextDocument(mainDoc);
    await answered;
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
    for (const storage of landingStorages.splice(0)) {
      fs.rmSync(storage, { recursive: true, force: true });
    }
  });

  test("creates scaffold in an empty target directory", async () => {
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "hello-project");
    const created = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      { targetUri, openWorkspace: false }
    );
    assert.strictEqual(created, true, "Expected project command to succeed.");

    // `trust-runtime new` writes the project (spec 25 §3.2), so the program is src/Main.st.
    const mainUri = vscode.Uri.joinPath(targetUri, "src", "Main.st");
    const legacyMainUri = vscode.Uri.joinPath(targetUri, "sources", "Main.st");
    assert.strictEqual(await pathExists(mainUri), true, "Expected src/Main.st file.");
    assert.strictEqual(
      await pathExists(legacyMainUri),
      false,
      "Expected no legacy sources/ scaffold."
    );

    const source = await readText(mainUri);
    assert.ok(source.includes("PROGRAM Main"), "Expected PROGRAM scaffold.");

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
    const instance = /PROGRAM\s+(\w+)\s+WITH\s+\w+\s*:\s*Main\b/.exec(configSource);
    assert.ok(
      instance && instance[1] !== "Main",
      "config.st must instantiate Main with a distinct instance name so it is not flagged unused or duplicated (F-02)."
    );
    const projectToml = await readText(vscode.Uri.joinPath(targetUri, "trust-lsp.toml"));
    assert.ok(
      /include_paths\s*=\s*\[\s*"src"\s*\]/.test(projectToml),
      "trust-lsp.toml must include the src folder (spec 25 §3.2)."
    );
    // The native simulator launch target and the status bar setting (spec 25 §3.2).
    const launch = JSON.parse(
      await readText(vscode.Uri.joinPath(targetUri, ".vscode", "launch.json"))
    ) as { configurations?: Array<Record<string, unknown>> };
    assert.ok(
      launch.configurations?.some(
        (config) =>
          config.type === "structured-text" &&
          config.request === "launch" &&
          config.name === "truST Simulator" &&
          config.program === "${workspaceFolder}/src/config.st"
      ),
      "launch.json must target ${workspaceFolder}/src/config.st with the truST Simulator configuration."
    );
    const settings = JSON.parse(
      await readText(vscode.Uri.joinPath(targetUri, ".vscode", "settings.json"))
    ) as Record<string, unknown>;
    assert.strictEqual(
      settings["debug.showInStatusBar"],
      "never",
      "settings.json must hide the debug status bar selector (spec 25 §3.2)."
    );
    const gitignore = await readText(vscode.Uri.joinPath(targetUri, ".gitignore"));
    assert.ok(
      gitignore.split(/\r?\n/).map((line) => line.trim()).includes("/.trust-runtime/"),
      ".gitignore must list /.trust-runtime/ (spec 11 §6.8.0a)."
    );
    const runtimeToml = await readText(runtimeTomlUri);
    assert.ok(
      runtimeToml.includes("[runtime.control]"),
      "runtime.toml must declare [runtime.control]."
    );
    assert.ok(
      !/^\s*endpoint\s*=/m.test(runtimeToml),
      "runtime.toml must leave the control endpoint to the platform default (spec 11 §6.8.0a)."
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
      "io.toml must use the simulated driver (spec 25 §3.2)."
    );
    await deleteTreeIfExists(targetUri);
  });

  test("the name box opens with a selected suggestion and no message", async () => {
    // Acceptance finding F10: the box said "Project name is required." before anything was typed, because VS Code checks
    // the initial value as soon as the box opens. Each start suggests its own name (spec 25 §3.2).
    const originalInput = vscode.window.showInputBox;
    const shown: vscode.InputBoxOptions[] = [];
    vscode.window.showInputBox = async (options?: vscode.InputBoxOptions) => {
      shown.push(options ?? {});
      return undefined;
    };
    const starts = [
      ["structured-text", "my-st-project", "Enter a name for the new Structured Text project"],
      ["rust", "my-rust-project", "Enter a name for the new Structured Text project with Rust"],
      ["rust-first", "my-rust-cell", "Enter a name for the new Rust-first project"],
    ] as const;
    try {
      for (const [start] of starts) {
        const created = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
          start,
          baseUri: fixturesRoot,
          openWorkspace: false,
        });
        assert.strictEqual(created, false, `a cancelled name box creates nothing (${start})`);
      }
    } finally {
      vscode.window.showInputBox = originalInput;
    }
    assert.strictEqual(shown.length, starts.length, "the command asks for the name once for each start");
    for (const [index, [start, suggestion, prompt]] of starts.entries()) {
      const box = shown[index];
      assert.strictEqual(box.prompt, prompt, start);
      assert.strictEqual(box.value, suggestion, start);
      assert.deepStrictEqual(box.valueSelection, [0, suggestion.length], `the first key replaces the suggestion (${start})`);
      assert.strictEqual(await box.validateInput?.(box.value ?? ""), undefined, `the box opens without a message (${start})`);
    }

    // `trust-runtime new` checks the name (spec 25 §3.2), so an emptied name still says why, with the command's
    // message, and creates nothing.
    const originalError = vscode.window.showErrorMessage;
    const errors: string[] = [];
    vscode.window.showErrorMessage = (async (message: string) => {
      errors.push(message);
      return undefined;
    }) as typeof vscode.window.showErrorMessage;
    try {
      const refused = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
        start: "structured-text",
        baseUri: fixturesRoot,
        projectName: "   ",
        openWorkspace: false,
      });
      assert.strictEqual(refused, false, "an empty name creates nothing");
    } finally {
      vscode.window.showErrorMessage = originalError;
    }
    assert.deepStrictEqual(errors, ["the project name is required"], "an emptied name still says why");
  });

  test("Create Project asks for the start first and lands on a Rust start's crate", async () => {
    // Spec 25 §3.2, spec 48 RU-51: the picker offers the three starts in this order before the folder and the name,
    // and passes a Rust start's flag to `trust-runtime new`, whose files are its; the window lands on the file the
    // command names, the crate's source, in the project the command names. Catches a picker that drops a start,
    // passes no flag (a Structured Text project would land on src/Main.st), or a landing that takes rust/ for the
    // project.
    const originalPick = vscode.window.showQuickPick;
    const offered: string[][] = [];
    vscode.window.showQuickPick = (async (items: readonly vscode.QuickPickItem[] | Thenable<readonly vscode.QuickPickItem[]>) => {
      const choices = await items;
      offered.push(choices.map((item) => item.label));
      return choices.find((item) => item.label === "Structured Text with Rust");
    }) as unknown as typeof vscode.window.showQuickPick;
    const withRust = vscode.Uri.joinPath(fixturesRoot, "with-rust");
    try {
      const created = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
        baseUri: fixturesRoot,
        projectName: "with-rust",
        openWorkspace: false,
      });
      assert.strictEqual(created, true, "the picked start creates its project");
    } finally {
      vscode.window.showQuickPick = originalPick;
    }
    assert.deepStrictEqual(offered, [["Structured Text", "Structured Text with Rust", "Rust-first"]]);
    for (const file of ["rust/Cargo.toml", "rust/src/lib.rs", "src/Main.st", "src/generated/rust_pous.st"]) {
      assert.strictEqual(await pathExists(vscode.Uri.joinPath(withRust, ...file.split("/"))), true, file);
    }
    assert.strictEqual(
      vscode.window.activeTextEditor?.document.uri.fsPath,
      vscode.Uri.joinPath(withRust, "rust", "src", "lib.rs").fsPath,
      "the window lands on the crate's source"
    );

    const cell = vscode.Uri.joinPath(fixturesRoot, "rust-cell");
    const createdCell = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
      start: "rust-first",
      targetUri: cell,
      openWorkspace: false,
    });
    assert.strictEqual(createdCell, true, "a Rust-first project is created");
    for (const file of ["Cargo.toml", "src/lib.rs", "trust.toml", "src/generated/trust/configuration.st"]) {
      assert.strictEqual(await pathExists(vscode.Uri.joinPath(cell, ...file.split("/"))), true, file);
    }
    assert.strictEqual(await pathExists(vscode.Uri.joinPath(cell, "src", "Main.st")), false, "no src/Main.st");
    assert.strictEqual(
      vscode.window.activeTextEditor?.document.uri.fsPath,
      vscode.Uri.joinPath(cell, "src", "lib.rs").fsPath,
      "the window lands on the crate's source"
    );
    await deleteTreeIfExists(withRust);
    await deleteTreeIfExists(cell);
  });

  test("cancel at each prompt stage leaves filesystem unchanged", async () => {
    const cancelAtStartTarget = vscode.Uri.joinPath(fixturesRoot, "cancel-at-start");
    const cancelledAtStart = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        baseUri: fixturesRoot,
        projectName: "cancel-at-start",
        simulateCancelAt: "start",
        openWorkspace: false,
      }
    );
    assert.strictEqual(cancelledAtStart, false);
    assert.strictEqual(await pathExists(cancelAtStartTarget), false);

    const cancelAtFolderTarget = vscode.Uri.joinPath(
      fixturesRoot,
      "cancel-at-folder"
    );
    const cancelledAtFolder = await vscode.commands.executeCommand<boolean>(
      NEW_PROJECT_COMMAND,
      {
        start: "structured-text",
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
        start: "structured-text",
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

  test("a folder with other files asks before it becomes a project and keeps them", async () => {
    // Spec 25 §3.2 and §11.6: a folder that is not empty is used only after confirmation (`trust-runtime new`
    // answers `exists`), and then the project's files are written while the folder's other files stay.
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "plain-folder");
    await vscode.workspace.fs.createDirectory(targetUri);
    const otherUri = vscode.Uri.joinPath(targetUri, "notes.txt");
    await vscode.workspace.fs.writeFile(otherUri, Buffer.from("mine"));
    const cancelled = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
      targetUri,
      simulateCancelAt: "overwrite",
      openWorkspace: false,
    });
    assert.strictEqual(cancelled, false, "a folder that is not empty is not used without confirmation");
    assert.strictEqual(await pathExists(vscode.Uri.joinPath(targetUri, "src", "Main.st")), false);
    const created = await vscode.commands.executeCommand<boolean>(NEW_PROJECT_COMMAND, {
      targetUri,
      overwrite: true,
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
