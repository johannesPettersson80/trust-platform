import * as assert from "assert";
import * as vscode from "vscode";

import { compileGate } from "../../compileGate";
import {
  __testCreateDefaultConfigurationAuto,
  __testSendIoDebugRequest,
} from "../../debug";
import {
  __testApplySettingsUpdate,
  __testCollectSettingsSnapshot,
} from "../../ioPanel";
import { classifyIoStateFailure } from "../../networkCanvas/runtimeFailures";
import { runtimeLifecycleService } from "../../runtimeLifecycle";

/**
 * Null once the language server lets Debug start (specification 25 §6.6: no project error in
 * Problems), else its reason with each file that holds an error, after `timeoutMs`.
 */
async function debugGateOpen(timeoutMs: number): Promise<string | null> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const reason = (await compileGate()).reasons.debug;
    if (!reason) {
      return null;
    }
    if (Date.now() > deadline) {
      const files = vscode.languages
        .getDiagnostics()
        .map(([uri, diagnostics]) => [uri, diagnostics.find((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error)] as const)
        .filter(([, error]) => error !== undefined)
        .map(([uri, error]) => `${uri.fsPath}: ${error?.message}`);
      return `${reason} ${files.join("; ")}`;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
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

  // The folder rule of a default CONFIGURATION is the language server's (trust-lsp.runEntry; run_entry_tests.rs holds the
  // former interactive/auto cases).
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
    // The task runs every resource cycle: runtime.toml's, else 10 ms (specification 25 §11.6; run_entry_tests.rs).
    assert.match(text, /TASK MainTask \(INTERVAL := T#\d+ms, PRIORITY := 1\);/);
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

  test("integration: reload command returns failure without active structured-text session", async () => {
    await ensureNoStructuredTextSession();
    const result = await vscode.commands.executeCommand<{ ok?: boolean; message?: string }>(
      "trust-lsp.debug.reload"
    );
    assert.strictEqual(result?.ok, false);
    assert.ok(
      result?.message?.includes("No active Structured Text debug session"),
      `Expected no-session guidance, got ${JSON.stringify(result)}`
    );
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

  test("unit: Live Values I/O force/release use attach-safe custom requests", async () => {
    const calls: Array<{ command: string; args?: unknown }> = [];
    const session = {
      customRequest: async (command: string, args?: unknown) => {
        calls.push({ command, args });
        return undefined;
      },
    };

    await __testSendIoDebugRequest(session, "write", "%IX0.0", "TRUE");
    await __testSendIoDebugRequest(session, "force", "%QX0.0", "TRUE");
    await __testSendIoDebugRequest(session, "release", "%QX0.0");

    assert.deepStrictEqual(calls, [
      { command: "stIoWrite", args: { address: "%IX0.0", value: "TRUE" } },
      { command: "stIoForce", args: { address: "%QX0.0", value: "TRUE" } },
      { command: "stIoRelease", args: { address: "%QX0.0" } },
    ]);
    assert.ok(
      calls.every((call) => call.command !== "setExpression"),
      "Live Values I/O commands must not use setExpression in attach mode"
    );
  });

  test("unit: Live Values I/O shows the adapter's answer and sends nothing again", async () => {
    // Specification 13, "setVariable and setExpression": the adapter sends a write again while
    // the runtime is busy, so the extension sends each request once and shows the answer.
    const busy = "resource command queue is full; the request was not queued";
    const calls: Array<{ command: string; args?: unknown }> = [];
    const session = {
      customRequest: async (command: string, args?: unknown) => {
        calls.push({ command, args });
        throw new Error(busy);
      },
    };

    await assert.rejects(
      () => __testSendIoDebugRequest(session, "release", "%QX0.0"),
      new RegExp(busy)
    );

    assert.deepStrictEqual(calls, [
      { command: "stIoRelease", args: { address: "%QX0.0" } },
    ]);
  });

  // The language server reads a failed I/O state request (specification 25 §4.4); its kinds are proven by
  // crates/trust-lsp/src/handlers/commands/run_targets/connect_tests.rs. Catches the extension reading the failure's
  // text itself, or a failure that does not reach the server.
  test("integration: a failed I/O state request takes its kind from the language server", async () => {
    await ensureNoStructuredTextSession();
    const result = await runtimeLifecycleService.requestIoState();
    if (result.ok) {
      assert.fail("an I/O state request without a session fails");
    }
    assert.deepStrictEqual(result.failure, {
      kind: "no_session",
      message: "No active Structured Text debug session.",
    });
    assert.deepStrictEqual(await classifyIoStateFailure(new Error("I/O state request timed out.")), {
      kind: "connection_lost",
      message: "Live Values lost connection to the runtime. Restart or reconnect the runtime, then retry.",
      detail: "I/O state request timed out.",
    });
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
      // The suite's workspace holds the fixtures of earlier tests: Debug starts once Problems
      // holds no project error, and the test names any error that keeps the gate shut.
      assert.strictEqual(await debugGateOpen(20000), null, "Problems must hold no project error before Debug");
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
