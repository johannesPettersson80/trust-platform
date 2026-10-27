import * as assert from "assert";
import * as fs from "fs/promises";
import * as path from "path";
import * as vscode from "vscode";
import { copyExample } from "../../examples/copy";
import { PROJECT_LANDING_KEY } from "../../projectLanding";

suite("Created project folder recovery", () => {
  test("retained lesson picker opens the explicitly selected copy", async () => {
    const api = require("../../examples/projectReopening") as {
      openRetainedControlLessonProject?: (
        context: unknown,
        lessonId: string,
        records: ReadonlyArray<{ lessonId: string; projectPath: string }>,
        choose: (paths: ReadonlyArray<string>) => Promise<string | undefined>,
        exists: (projectPath: string) => Promise<boolean>,
        open: (projectPath: string) => Promise<boolean>
      ) => Promise<{ kind: string }>;
    };
    assert.strictEqual(typeof api.openRetainedControlLessonProject, "function");
    if (!api.openRetainedControlLessonProject) return;
    const selected = "/projects/lesson-copy-two";
    const opened: string[] = [];
    const result = await api.openRetainedControlLessonProject(
      {},
      "lesson-1",
      [
        { lessonId: "lesson-1", projectPath: "/projects/lesson-copy-one" },
        { lessonId: "lesson-1", projectPath: selected },
      ],
      async (paths) => {
        assert.deepStrictEqual(paths, ["/projects/lesson-copy-one", selected]);
        return selected;
      },
      async () => true,
      async (projectPath) => { opened.push(projectPath); return true; }
    );
    assert.deepStrictEqual(result, { kind: "opened", projectPath: selected });
    assert.deepStrictEqual(opened, [selected]);
  });

  test("retained lesson picker offers recovery for a missing copy without recreating it", async () => {
    const api = require("../../examples/projectReopening") as {
      openRetainedControlLessonProject?: (
        context: unknown,
        lessonId: string,
        records: ReadonlyArray<{ lessonId: string; projectPath: string }>,
        choose: (paths: ReadonlyArray<string>) => Promise<string | undefined>,
        exists: (projectPath: string) => Promise<boolean>,
        open: (projectPath: string) => Promise<boolean>
      ) => Promise<{ kind: string; projectPath?: string }>;
    };
    assert.strictEqual(typeof api.openRetainedControlLessonProject, "function");
    if (!api.openRetainedControlLessonProject) return;
    const result = await api.openRetainedControlLessonProject(
      {}, "lesson-1", [{ lessonId: "lesson-1", projectPath: "/projects/moved-copy" }],
      async () => "/projects/moved-copy", async () => false, async () => {
        throw new Error("missing projects must not be recreated");
      }
    );
    assert.deepStrictEqual(result, { kind: "missing", projectPath: "/projects/moved-copy" });
  });

  test("retained lesson picker rejects a path that was not persisted for the lesson", async () => {
    const api = require("../../examples/projectReopening") as {
      openRetainedControlLessonProject?: (
        context: unknown, lessonId: string,
        records: ReadonlyArray<{ lessonId: string; projectPath: string }>,
        choose: (paths: ReadonlyArray<string>) => Promise<string | undefined>,
        exists: (projectPath: string) => Promise<boolean>,
        open: (projectPath: string) => Promise<boolean>
      ) => Promise<{ kind: string; projectPath?: string }>;
    };
    assert.strictEqual(typeof api.openRetainedControlLessonProject, "function");
    if (!api.openRetainedControlLessonProject) return;
    const opened: string[] = [];
    const result = await api.openRetainedControlLessonProject(
      {}, "lesson-1", [{ lessonId: "lesson-1", projectPath: "/projects/known" }],
      async () => "/projects/not-persisted", async () => true,
      async (projectPath) => { opened.push(projectPath); return true; }
    );
    assert.deepStrictEqual(result, { kind: "cancelled" });
    assert.deepStrictEqual(opened, []);
  });

  test("retained project open failure uses existing-project recovery wording", async () => {
    const api = require("../../examples/projectReopening") as {
      openRetainedControlLessonProject?: (
        context: unknown, lessonId: string,
        records: ReadonlyArray<{ lessonId: string; projectPath: string }>,
        choose: (paths: ReadonlyArray<string>) => Promise<string | undefined>,
        exists: (projectPath: string) => Promise<boolean>
      ) => Promise<{ kind: string }>;
    };
    assert.strictEqual(typeof api.openRetainedControlLessonProject, "function");
    if (!api.openRetainedControlLessonProject) return;
    const originalExecute = vscode.commands.executeCommand;
    const originalError = vscode.window.showErrorMessage;
    const messages: string[] = [];
    vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => {
      if (command === "vscode.openFolder") throw new Error("reopen rejected");
      return originalExecute(command, ...args);
    }) as typeof vscode.commands.executeCommand;
    vscode.window.showErrorMessage = (async (message: string) => { messages.push(message); return undefined; }) as typeof vscode.window.showErrorMessage;
    try {
      const result = await api.openRetainedControlLessonProject(
        {}, "lesson-1", [{ lessonId: "lesson-1", projectPath: "/projects/existing-copy" }],
        async () => "/projects/existing-copy", async () => true
      );
      assert.deepStrictEqual(result, { kind: "cancelled" });
      assert.ok(messages.some((message) => message.includes("Could not open existing project at /projects/existing-copy")));
      assert.ok(messages.every((message) => !message.includes("Project created at")));
    } finally {
      vscode.commands.executeCommand = originalExecute;
      vscode.window.showErrorMessage = originalError;
    }
  });

  test("lesson copies persist two locations before a folder-opening failure", async () => {
    const extensionRoot = path.resolve(__dirname, "..", "..", "..");
    const destination = vscode.Uri.file(await fs.mkdtemp(path.join(require("os").tmpdir(), "trust-cpex012-")));
    const state = new Map<string, unknown>();
    const context = {
      extensionUri: vscode.Uri.file(extensionRoot),
      globalState: {
        get: <T>(key: string) => state.get(key) as T,
        update: async (key: string, value: unknown) => { state.set(key, value); },
      },
    } as unknown as vscode.ExtensionContext;
    const previousDestination = process.env.TRUST_UX_EXAMPLE_DESTINATION;
    const previousName = process.env.TRUST_UX_EXAMPLE_NAME;
    const previousOpen = process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
    const previousExecute = vscode.commands.executeCommand;
    const previousError = vscode.window.showErrorMessage;
    let opens = 0;
    process.env.TRUST_UX_EXAMPLE_DESTINATION = destination.fsPath;
    process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = "0";
    const lesson = {
      id: "control-cold-storage-loop-anatomy", title: "Control · Cold storage loop anatomy", description: "",
      path: "control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy", hardware: "none",
      tags: ["control"], lesson: { sequence: 1, level: "Foundation", equipment: "Simulator", principalTechnique: "Feedback", prerequisites: [] },
    };
    try {
      process.env.TRUST_UX_EXAMPLE_NAME = "lesson-copy-one";
      assert.strictEqual(await copyExample(context, lesson), true);
      process.env.TRUST_UX_EXAMPLE_NAME = "lesson-copy-two";
      assert.strictEqual(await copyExample(context, lesson), true);
      const locations = state.get("trust.examples.controlLessonProjects") as unknown[];
      assert.deepStrictEqual(locations, [
        { lessonId: lesson.id, projectPath: path.join(destination.fsPath, "lesson-copy-one") },
        { lessonId: lesson.id, projectPath: path.join(destination.fsPath, "lesson-copy-two") },
      ]);

      process.env.TRUST_UX_EXAMPLE_NAME = "lesson-copy-failed-open";
      delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER;
      vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => {
        if (command !== "vscode.openFolder") return previousExecute(command, ...args);
        opens++;
        const retained = state.get("trust.examples.controlLessonProjects") as Array<{ projectPath: string }> | undefined;
        assert.ok(retained?.some((record) => record.projectPath.endsWith("lesson-copy-failed-open")),
          "lesson destination must be persisted before folder opening can reload or fail");
        throw new Error("folder switch rejected");
      }) as typeof vscode.commands.executeCommand;
      vscode.window.showErrorMessage = (async () => undefined) as typeof vscode.window.showErrorMessage;
      assert.strictEqual(await copyExample(context, lesson), false);
      assert.ok((state.get("trust.examples.controlLessonProjects") as unknown[]).some((record) =>
        (record as { projectPath: string }).projectPath.endsWith("lesson-copy-failed-open")
      ));
      assert.strictEqual(opens, 1);
    } finally {
      vscode.commands.executeCommand = previousExecute;
      vscode.window.showErrorMessage = previousError;
      if (previousDestination === undefined) delete process.env.TRUST_UX_EXAMPLE_DESTINATION; else process.env.TRUST_UX_EXAMPLE_DESTINATION = previousDestination;
      if (previousName === undefined) delete process.env.TRUST_UX_EXAMPLE_NAME; else process.env.TRUST_UX_EXAMPLE_NAME = previousName;
      if (previousOpen === undefined) delete process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER; else process.env.TRUST_UX_EXAMPLE_OPEN_FOLDER = previousOpen;
      await fs.rm(destination.fsPath, { recursive: true, force: true });
    }
  });

  test("starter creation preserves its scaffold and does not report success when opening recovery is dismissed", async () => {
    const root = vscode.workspace.workspaceFolders?.[0]?.uri;
    assert.ok(root);
    const project = vscode.Uri.joinPath(root, "starter-opening-failure");
    const originalExecute = vscode.commands.executeCommand;
    const originalError = vscode.window.showErrorMessage;
    const originalInfo = vscode.window.showInformationMessage;
    let attempts = 0;
    const notices: string[] = [];
    const successes: string[] = [];
    try {
      vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => {
        if (command !== "vscode.openFolder") return originalExecute(command, ...args);
        attempts++;
        throw new Error("folder switch rejected");
      }) as typeof vscode.commands.executeCommand;
      vscode.window.showErrorMessage = (async (message: string) => { notices.push(message); return undefined; }) as typeof vscode.window.showErrorMessage;
      vscode.window.showInformationMessage = (async (message: string) => { successes.push(message); return undefined; }) as typeof vscode.window.showInformationMessage;
      assert.strictEqual(await vscode.commands.executeCommand("trust-lsp.newProject", { targetUri: project, openWorkspace: true }), false);
      assert.strictEqual(attempts, 1);
      assert.strictEqual(notices.length, 1);
      assert.ok(notices[0].includes(project.fsPath));
      assert.deepStrictEqual(successes, []);
      assert.match(Buffer.from(await vscode.workspace.fs.readFile(vscode.Uri.joinPath(project, "src", "Main.st"))).toString(), /PROGRAM Main/);
    } finally {
      vscode.commands.executeCommand = originalExecute;
      vscode.window.showErrorMessage = originalError;
      vscode.window.showInformationMessage = originalInfo;
      await vscode.workspace.fs.delete(project, { recursive: true, useTrash: false });
    }
  });
  test("a copied project survives failed folder opening and retries without copying again", async () => {
    const root = vscode.workspace.workspaceFolders?.[0]?.uri;
    assert.ok(root);
    const fixture = vscode.Uri.joinPath(root, "project-opening-fixture");
    const source = vscode.Uri.joinPath(fixture, "extension", "media", "examples", "starter");
    const destination = vscode.Uri.joinPath(fixture, "projects");
    await vscode.workspace.fs.createDirectory(source);
    await vscode.workspace.fs.createDirectory(destination);
    await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(source, "marker.txt"), Buffer.from("original"));
    const originalDialog = vscode.window.showOpenDialog;
    const originalInput = vscode.window.showInputBox;
    const originalError = vscode.window.showErrorMessage;
    const originalExecute = vscode.commands.executeCommand;
    const state = new Map<string, unknown>();
    let opens = 0;
    const notices: string[] = [];
    const context = {
      extensionUri: vscode.Uri.joinPath(fixture, "extension"),
      globalState: {
        get: (key: string) => state.get(key),
        update: async (key: string, value: unknown) => { state.set(key, value); },
      },
    } as unknown as vscode.ExtensionContext;
    try {
      vscode.window.showOpenDialog = async () => [destination];
      vscode.window.showInputBox = async () => "chosen-project";
      vscode.commands.executeCommand = (async (command: string, ...args: unknown[]) => {
        // The copy asks the language server (`trust-lsp.copyExample`); only the folder switch is stubbed.
        if (command !== "vscode.openFolder") return originalExecute(command, ...args);
        assert.strictEqual((args[0] as vscode.Uri).fsPath, vscode.Uri.joinPath(destination, "chosen-project").fsPath);
        assert.strictEqual(args[1], false);
        if (++opens === 1) {
          await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(destination, "chosen-project", "marker.txt"), Buffer.from("edited after copy"));
          throw new Error("folder switch rejected");
        }
      }) as typeof vscode.commands.executeCommand;
      vscode.window.showErrorMessage = (async (message: string, ...actions: unknown[]) => {
        notices.push(message);
        assert.ok(message.includes(vscode.Uri.joinPath(destination, "chosen-project").fsPath));
        assert.match(message, /created/i);
        assert.ok(actions.includes("Retry opening"));
        return "Retry opening";
      }) as typeof vscode.window.showErrorMessage;
      assert.strictEqual(await copyExample(context, { id: "starter", title: "Starter", path: "starter", description: "Starter", tags: [], hardware: "none" }), true);
      assert.strictEqual(opens, 2);
      assert.strictEqual(notices.length, 1);
      assert.ok(state.get(PROJECT_LANDING_KEY));
      assert.strictEqual(Buffer.from(await vscode.workspace.fs.readFile(vscode.Uri.joinPath(destination, "chosen-project", "marker.txt"))).toString(), "edited after copy");
    } finally {
      vscode.window.showOpenDialog = originalDialog;
      vscode.window.showInputBox = originalInput;
      vscode.window.showErrorMessage = originalError;
      vscode.commands.executeCommand = originalExecute;
      await vscode.workspace.fs.delete(fixture, { recursive: true, useTrash: false });
    }
  });
});
