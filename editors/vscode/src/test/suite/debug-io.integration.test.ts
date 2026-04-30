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

async function waitForStructuredTextSession(timeoutMs = 10000): Promise<vscode.DebugSession> {
  const active = vscode.debug.activeDebugSession;
  if (active && active.type === "structured-text") {
    return active;
  }

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      disposable.dispose();
      reject(new Error("Timed out waiting for structured-text debug session."));
    }, timeoutMs);
    const disposable = vscode.debug.onDidStartDebugSession((session) => {
      if (session.type !== "structured-text") {
        return;
      }
      clearTimeout(timer);
      disposable.dispose();
      resolve(session);
    });
  });
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function ensureNoStructuredTextSession(): Promise<void> {
  const active = vscode.debug.activeDebugSession;
  if (active && active.type === "structured-text") {
    await vscode.debug.stopDebugging(active);
  }
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
    const mainUri = vscode.Uri.joinPath(srcRoot, "AutoConfigMain.st");
    await vscode.workspace.fs.writeFile(
      mainUri,
      Buffer.from(
        [
          "PROGRAM AutoConfigMain",
          "VAR",
          "    run : BOOL := TRUE;",
          "END_VAR",
          "END_PROGRAM",
          "",
        ].join("\n"),
        "utf8"
      )
    );

    const created = await __testCreateDefaultConfigurationAuto("AutoConfigMain", mainUri);
    assert.ok(created, "Expected auto configuration creation to return a URI.");
    assert.ok(await pathExists(created!), "Expected created configuration file.");
    const text = await readText(created!);
    assert.ok(text.includes("CONFIGURATION Conf"));
    assert.ok(text.includes("PROGRAM P1 WITH MainTask : AutoConfigMain;"));
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

    try {
      await __testApplySettingsUpdate(payload as any);
      const snapshot = __testCollectSettingsSnapshot() as Record<string, unknown>;
      for (const [key, value] of Object.entries(payload)) {
        assert.deepStrictEqual(snapshot[key], value, `Mismatch for ${key}`);
      }
    } finally {
      await __testApplySettingsUpdate(originalSettings as any);
    }
  });

  test("integration: debug command surface is registered", async () => {
    const commands = await vscode.commands.getCommands(true);
    const required = [
      "trust-lsp.debug.start",
      "trust-lsp.debug.attach",
      "trust-lsp.debug.stop",
      "trust-lsp.debug.reload",
      "trust-lsp.debug.ensureConfiguration",
      "trust-lsp.debug.openIoPanel",
      "trust-lsp.debug.openIoPanelSettings",
      "trust-lsp.debug.io.write",
      "trust-lsp.debug.io.force",
      "trust-lsp.debug.io.release",
      "trust-lsp.debug.expr.write",
      "trust-lsp.debug.expr.force",
      "trust-lsp.debug.expr.release",
    ];
    for (const command of required) {
      assert.ok(commands.includes(command), `Expected command '${command}' to be registered.`);
    }
  });

  test("integration: stop command returns false without active structured-text session", async () => {
    await ensureNoStructuredTextSession();
    const stopped = await vscode.commands.executeCommand<boolean>("trust-lsp.debug.stop");
    assert.strictEqual(stopped, false);
  });

  test("integration: io and expression commands are callable and reject without session", async () => {
    await ensureNoStructuredTextSession();

    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.io.write", {
            address: "%IX0.0",
            value: "TRUE",
          })
        ),
      /No active Structured Text debug session/
    );
    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.io.force", {
            address: "%IX0.0",
            value: "TRUE",
          })
        ),
      /No active Structured Text debug session/
    );
    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.io.release", {
            address: "%IX0.0",
          })
        ),
      /No active Structured Text debug session/
    );

    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.expr.write", {
            expression: "Main.run",
            value: "TRUE",
          })
        ),
      /No active Structured Text debug session/
    );
    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.expr.force", {
            expression: "Main.run",
            value: "TRUE",
          })
        ),
      /No active Structured Text debug session/
    );
    await assert.rejects(
      () =>
        Promise.resolve(
          vscode.commands.executeCommand("trust-lsp.debug.expr.release", {
            expression: "Main.run",
          })
        ),
      /No active Structured Text debug session/
    );
  });

  test("integration: VM debug session returns non-empty stackTrace at stopOnEntry", async () => {
    await ensureNoStructuredTextSession();
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    assert.ok(extension, "Expected trust-lsp extension to be installed for tests.");
    await extension!.activate();
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected workspace folder for debug session test.");

    const projectRoot = vscode.Uri.joinPath(fixturesRoot, "stacktrace-session-project");
    await vscode.workspace.fs.createDirectory(projectRoot);
    const sourceUri = vscode.Uri.joinPath(projectRoot, "main.st");
    const programName = "VmDebugStackTraceMain";
    const configurationName = "VmDebugStackTraceConf";
    const taskName = "VmDebugStackTraceTask";
    const programInstanceName = "VmDebugStackTraceP1";
    const source = [
      `PROGRAM ${programName}`,
      "VAR",
      "    Count : INT := 0;",
      "END_VAR",
      "    Count := Count + 1;",
      "END_PROGRAM",
      "",
      `CONFIGURATION ${configurationName}`,
      `TASK ${taskName} (INTERVAL := T#100ms, PRIORITY := 1);`,
      `PROGRAM ${programInstanceName} WITH ${taskName} : ${programName};`,
      "END_CONFIGURATION",
      "",
    ].join("\n");
    await vscode.workspace.fs.writeFile(sourceUri, Buffer.from(source, "utf8"));
    const sourceRelativePath = vscode.workspace
      .asRelativePath(sourceUri, false)
      .replace(/\\/g, "/");
    await __testApplySettingsUpdate({
      debugAdapterPath: "",
      runtimeIncludeGlobs: [sourceRelativePath],
      runtimeExcludeGlobs: [],
    } as any);
    const opened = await vscode.workspace.openTextDocument(sourceUri);
    await vscode.window.showTextDocument(opened, { preview: false });

    const breakpoint = new vscode.SourceBreakpoint(
      new vscode.Location(sourceUri, new vscode.Position(4, 0))
    );
    vscode.debug.addBreakpoints([breakpoint]);

    try {
      const started = await vscode.commands.executeCommand<boolean>(
        "trust-lsp.debug.start",
        sourceUri.fsPath
      );
      assert.strictEqual(started, true, "Expected debugger to start.");

      const session = await waitForStructuredTextSession();
      let frames: Array<unknown> = [];
      const deadline = Date.now() + 10000;
      while (Date.now() < deadline) {
        const threads = (await session.customRequest("threads")) as {
          threads?: Array<{ id: number }>;
        };
        const threadId = threads.threads?.[0]?.id;
        if (typeof threadId === "number") {
          const stack = (await session.customRequest("stackTrace", {
            threadId,
            startFrame: 0,
            levels: 20,
          })) as { stackFrames?: Array<unknown> };
          frames = stack.stackFrames ?? [];
          if (frames.length > 0) {
            break;
          }
        }
        await sleep(100);
      }
      assert.ok(frames.length > 0, "Expected non-empty stackTrace while debugger is stopped.");
      await vscode.debug.stopDebugging(session);
      await ensureNoStructuredTextSession();
    } finally {
      await __testApplySettingsUpdate(originalSettings as any);
      vscode.debug.removeBreakpoints([breakpoint]);
    }
  });
});
