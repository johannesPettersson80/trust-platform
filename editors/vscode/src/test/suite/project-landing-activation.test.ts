import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import * as landing from "../../projectLanding";
import { focusPendingMain } from "../../newProject";
import { openPendingControlLessonReadme } from "../../examples/copy";

suite("Project landing activation", () => {
  test("legacy activation cannot open a stale destination or overwrite a newly published intent", async () => {
    const workspace = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspace);
    const project = vscode.Uri.file(fs.mkdtempSync(path.join(workspace.uri.fsPath, "landing-migration-")));
    const readme = vscode.Uri.joinPath(project, "README.md");
    await vscode.workspace.fs.writeFile(readme, Buffer.from("# Legacy lesson\n"));
    const state = new Map<string, unknown>([[landing.OPEN_LESSON_README_KEY, readme.fsPath]]);
    const context = { globalState: {
      get: <T>(key: string) => state.get(key) as T,
      update: async (key: string, value: unknown) => { state.set(key, value); },
    } } as unknown as vscode.ExtensionContext;
    const newer: landing.ProjectLandingIntent = {
      intentId: "newer-than-legacy", kind: "lesson", projectPath: project.fsPath,
      filePath: readme.fsPath, exampleId: "selected-again",
    };
    const originalExecute = vscode.commands.executeCommand;
    const opened: unknown[][] = [];
    try {
      vscode.commands.executeCommand = (async (...args: unknown[]) => { opened.push(args); }) as typeof vscode.commands.executeCommand;
      const consuming = landing.consumeProjectLanding(context);
      state.set(landing.PROJECT_LANDING_KEY, newer);
      await consuming;
      assert.deepStrictEqual(state.get(landing.PROJECT_LANDING_KEY), newer, "legacy migration must not replace or clear a newly selected destination");
      assert.deepStrictEqual(opened, [], "superseded legacy landing must not open over the newly selected destination");
    } finally {
      vscode.commands.executeCommand = originalExecute;
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });
  for (const [name, key, file, open] of [
    ["starter", landing.FOCUS_MAIN_KEY, path.join(__dirname, "unrelated", "src", "Main.st"), focusPendingMain],
    ["lesson", landing.OPEN_LESSON_README_KEY, path.join(__dirname, "unrelated", "README.md"), openPendingControlLessonReadme],
  ] as const) {
    test(`legacy ${name} entrypoint leaves an unrelated pending destination untouched`, async () => {
      assert.strictEqual(vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file)), undefined);
      const state = new Map<string, unknown>([[key, file]]);
      const context = { globalState: {
        get: <T>(key: string) => state.get(key) as T,
        update: async (key: string, value: unknown) => { state.set(key, value); },
      } } as unknown as vscode.ExtensionContext;
      await open(context);
      assert.strictEqual(state.get(key), file, "unrelated legacy intent must not be consumed during migration");
      assert.strictEqual(state.get(landing.PROJECT_LANDING_KEY), undefined);
    });
  }
  test("reports missing destination recovery without offering a futile retry", async () => {
    const modulePath = path.join(__dirname, "../../projectLandingActivation.js");
    const api = fs.existsSync(modulePath) ? require(modulePath) as {
      openPendingProjectLanding?: (context: vscode.ExtensionContext) => Promise<void>;
    } : undefined;
    assert.strictEqual(typeof api?.openPendingProjectLanding, "function");
    const landingModule = landing as { consumeProjectLanding: typeof landing.consumeProjectLanding };
    const originalConsume = landingModule.consumeProjectLanding;
    const originalWarning = vscode.window.showWarningMessage;
    const notices: unknown[][] = [];
    try {
      landingModule.consumeProjectLanding = async () => ({ kind: "cleared", message: "The selected README is missing. Restore it or copy the example again." });
      vscode.window.showWarningMessage = (async (...args: unknown[]) => {
        notices.push(args);
        return undefined;
      }) as typeof vscode.window.showWarningMessage;
      await api!.openPendingProjectLanding!({} as vscode.ExtensionContext);
      assert.deepStrictEqual(notices, [["The selected README is missing. Restore it or copy the example again."]]);
    } finally {
      landingModule.consumeProjectLanding = originalConsume;
      vscode.window.showWarningMessage = originalWarning;
    }
  });
  test("offers an explicit retry after opening fails and consumes again only when selected", async () => {
    const modulePath = path.join(__dirname, "../../projectLandingActivation.js");
    const api = fs.existsSync(modulePath) ? require(modulePath) as {
      openPendingProjectLanding?: (context: vscode.ExtensionContext) => Promise<void>;
    } : undefined;
    assert.strictEqual(typeof api?.openPendingProjectLanding, "function", "activation must handle a retryable project landing");
    const originalConsume = landing.consumeProjectLanding;
    const landingModule = landing as { consumeProjectLanding: typeof landing.consumeProjectLanding };
    const originalWarning = vscode.window.showWarningMessage;
    let calls = 0;
    const notices: unknown[][] = [];
    try {
      landingModule.consumeProjectLanding = async () => ++calls === 1
        ? { kind: "failed", message: "Could not open the selected README.", retryable: true }
        : { kind: "opened" };
      vscode.window.showWarningMessage = (async (...args: unknown[]) => {
        notices.push(args);
        return "Retry";
      }) as typeof vscode.window.showWarningMessage;
      await api!.openPendingProjectLanding!({} as vscode.ExtensionContext);
      assert.strictEqual(calls, 2);
      assert.deepStrictEqual([...notices], [["Could not open the selected README.", "Retry"]]);
      calls = 0;
      notices.length = 0;
      vscode.window.showWarningMessage = (async (...args: unknown[]) => {
        notices.push(args);
        return undefined;
      }) as typeof vscode.window.showWarningMessage;
      await api!.openPendingProjectLanding!({} as vscode.ExtensionContext);
      assert.strictEqual(calls, 1, "dismissing the notice must not retry automatically");
      assert.strictEqual(notices.length, 1);
    } finally {
      landingModule.consumeProjectLanding = originalConsume;
      vscode.window.showWarningMessage = originalWarning;
    }
  });
});
