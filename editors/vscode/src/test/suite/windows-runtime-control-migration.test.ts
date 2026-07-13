import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { buildRuntimeTomlSource } from "../../newProject";
import { resolveLaunchMigrationRoot } from "../../debug/launchControl";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import {
  migrateRuntimeControlTomlSource,
  migrateWindowsRuntimeControlProject,
} from "../../windowsRuntimeControlMigration";

interface SourceMigrationResult {
  readonly changed: boolean;
  readonly content: string;
}

interface FileMigrationResult {
  readonly changed: boolean;
  readonly path?: string;
}

const TEST_TOKEN = "unit-test-generated-control-token-123456";

function migrateSource(
  source: string,
  platform: NodeJS.Platform = "win32",
  token = TEST_TOKEN
): SourceMigrationResult {
  return migrateRuntimeControlTomlSource(source, platform, () => token);
}

function migrateProject(
  projectRoot: string,
  platform: NodeJS.Platform = "win32"
): FileMigrationResult {
  return migrateWindowsRuntimeControlProject(projectRoot, platform);
}

function tokenFrom(source: string): string {
  return (
    /^\s*(?:runtime\.control\.)?auth_token\s*=\s*["']([^"']*)["']/m.exec(source)?.[1] ?? ""
  );
}

function legacyRuntimeToml(
  endpoint = "tcp://127.0.0.1:9902",
  eol = "\n"
): string {
  return [
    "# Existing project; keep this comment",
    "[bundle]",
    "version = 1",
    "",
    "[runtime.control]",
    `endpoint = "${endpoint}" # keep endpoint note`,
    'mode = "production"',
    "",
    "[runtime.web]",
    "enabled = false",
    "",
  ].join(eol);
}

function dottedRuntimeToml(
  endpoint = "tcp://127.0.0.1:9902",
  authTokenLine: string | undefined = undefined,
  eol = "\n"
): string {
  return [
    "# Existing dotted project; keep this comment",
    `runtime.control.endpoint = "${endpoint}" # keep dotted endpoint note`,
    ...(authTokenLine ? [authTokenLine] : []),
    'runtime.control.mode = "production"',
    "",
    "[runtime.web]",
    "enabled = false",
    "",
  ].join(eol);
}

function tokenlessDottedGeneratedRuntimeToml(): string {
  const generated = buildRuntimeTomlSource("win32");
  const controlBlock = /\[runtime\.control\]\r?\nendpoint = "([^"]+)"\r?\nauth_token = "[^"]+"\r?\nmode = "([^"]+)"\r?\ndebug_enabled = (true|false)\r?\n\r?\n/;
  const match = controlBlock.exec(generated);
  assert.ok(match, "Expected the generated project to contain runtime.control.");
  const withoutControlTable = generated.replace(controlBlock, "");
  return [
    `runtime.control.endpoint = "${match[1]}"`,
    `runtime.control.mode = "${match[2]}"`,
    `runtime.control.debug_enabled = ${match[3]}`,
    "",
    withoutControlTable,
  ].join("\n");
}

async function waitForStructuredTextSession(
  expectedName: string,
  timeoutMs = 15_000
): Promise<vscode.DebugSession> {
  const active = vscode.debug.activeDebugSession;
  if (active?.type === "structured-text" && active.name === expectedName) {
    return active;
  }
  return await new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      subscription.dispose();
      reject(new Error(`Timed out waiting for ${expectedName}.`));
    }, timeoutMs);
    const subscription = vscode.debug.onDidStartDebugSession((session) => {
      if (session.type !== "structured-text" || session.name !== expectedName) {
        return;
      }
      clearTimeout(timer);
      subscription.dispose();
      resolve(session);
    });
  });
}

async function waitForAuthenticatedControl(
  endpoint: string,
  authToken: string,
  timeoutMs = 15_000
): Promise<unknown> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown;
  while (Date.now() < deadline) {
    try {
      return await sendRuntimeControlRequest(
        endpoint,
        authToken,
        "comm.schema",
        undefined,
        { timeoutMs: 1_000 }
      );
    } catch (error) {
      lastError = error;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw new Error(
    `Authenticated simulator control did not become ready: ${
      lastError instanceof Error ? lastError.message : String(lastError)
    }`
  );
}

async function requestIoStateEvent(
  session: vscode.DebugSession,
  timeoutMs = 5_000
): Promise<unknown> {
  let subscription: vscode.Disposable | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const event = new Promise<unknown>((resolve) => {
    subscription = vscode.debug.onDidReceiveDebugSessionCustomEvent((candidate) => {
      if (candidate.session.id === session.id && candidate.event === "stIoState") {
        resolve(candidate.body);
      }
    });
  });
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => {
      reject(new Error("Timed out waiting for the stIoState DAP request and event."));
    }, timeoutMs);
  });

  try {
    // stIoState deliberately returns an empty DAP response; its payload is the
    // matching custom event so Live Values and other listeners receive one state.
    const [, body] = await Promise.race([
      Promise.all([session.customRequest("stIoState"), event]),
      timeout,
    ]);
    return body;
  } finally {
    if (timer) {
      clearTimeout(timer);
    }
    subscription?.dispose();
  }
}

suite("Windows runtime control migration", () => {
  test("falls back to the workspace when launch variables are not substituted yet", () => {
    const folder = {
      uri: vscode.Uri.file(path.join(os.tmpdir(), "trust-workspace-root")),
    } as vscode.WorkspaceFolder;

    assert.strictEqual(
      resolveLaunchMigrationRoot(
        {
          type: "structured-text",
          request: "launch",
          name: "Simulator",
          runtimeRoot: "${workspaceFolder}",
          cwd: "${workspaceFolder}",
        },
        folder
      ),
      folder.uri.fsPath
    );
    assert.strictEqual(
      resolveLaunchMigrationRoot(
        {
          type: "structured-text",
          request: "launch",
          name: "Simulator",
          runtimeRoot: "C:\\projects\\explicit",
        },
        folder
      ),
      "C:\\projects\\explicit"
    );
  });

  test("adds a strong token to a legacy local Windows runtime.toml", () => {
    const source = legacyRuntimeToml();
    const result = migrateSource(source);

    assert.strictEqual(result.changed, true);
    assert.ok(tokenFrom(result.content).length >= 24);
    assert.match(result.content, /endpoint = "tcp:\/\/127\.0\.0\.1:9902" # keep endpoint note/);
    assert.ok(result.content.includes("# Existing project; keep this comment"));
    assert.ok(result.content.includes('[runtime.web]\nenabled = false'));
  });

  test("adds a strong token to the supported top-level dotted runtime.control form", () => {
    const source = dottedRuntimeToml();
    const result = migrateSource(source);

    assert.strictEqual(result.changed, true);
    assert.strictEqual(tokenFrom(result.content), TEST_TOKEN);
    assert.ok(
      result.content.includes(
        `runtime.control.endpoint = "tcp://127.0.0.1:9902" # keep dotted endpoint note\n` +
          `runtime.control.auth_token = "${TEST_TOKEN}"\n` +
          'runtime.control.mode = "production"'
      )
    );
    assert.ok(result.content.includes("# Existing dotted project; keep this comment"));
    assert.ok(result.content.includes('[runtime.web]\nenabled = false'));
  });

  test("replaces a blank token while preserving indentation and its inline comment", () => {
    const source = legacyRuntimeToml().replace(
      'mode = "production"',
      '  auth_token   =   "   "   # keep credential note\nmode = "production"'
    );
    const result = migrateSource(source);

    assert.strictEqual(result.changed, true);
    assert.ok(
      result.content.includes(
        `  auth_token   =   "${TEST_TOKEN}"   # keep credential note`
      )
    );
  });

  test("rotates known placeholder tokens without logging or preserving the placeholder", () => {
    for (const placeholder of [
      "some-secret-value",
      "change-me",
      "changeme",
      "replace-me",
      "placeholder",
      "your-secret-here",
    ]) {
      const source = legacyRuntimeToml().replace(
        'mode = "production"',
        `  auth_token = "${placeholder}" # keep credential note\nmode = "production"`
      );
      const result = migrateSource(source);

      assert.strictEqual(result.changed, true, placeholder);
      assert.strictEqual(result.content.includes(placeholder), false, placeholder);
      assert.ok(
        result.content.includes(
          `  auth_token = "${TEST_TOKEN}" # keep credential note`
        )
      );
    }
  });

  test("replaces dotted blank and placeholder tokens while preserving formatting and comments", () => {
    for (const existing of ['"   "', '"some-secret-value"']) {
      const source = dottedRuntimeToml(
        "tcp://localhost:9902",
        `  runtime.control.auth_token   =   ${existing}   # keep dotted credential note`
      );
      const result = migrateSource(source);

      assert.strictEqual(result.changed, true, existing);
      assert.ok(
        result.content.includes(
          `  runtime.control.auth_token   =   "${TEST_TOKEN}"   # keep dotted credential note`
        )
      );
      assert.strictEqual(result.content.includes("some-secret-value"), false);
    }
  });

  test("accepts localhost, IPv6 loopback, and every literal in 127/8", () => {
    for (const endpoint of [
      "tcp://localhost:9902",
      "tcp://[::1]:9902",
      "tcp://127.0.0.1:9902",
      "tcp://127.42.3.9:9902",
      "tcp://127.255.255.254:9902",
    ]) {
      assert.strictEqual(
        migrateSource(legacyRuntimeToml(endpoint)).changed,
        true,
        endpoint
      );
    }
  });

  test("leaves existing credentials byte-for-byte unchanged", () => {
    const source = legacyRuntimeToml().replace(
      'mode = "production"',
      "  auth_token  =  'already-configured-value' # keep formatting\nmode = \"production\""
    );
    const result = migrateSource(source);

    assert.strictEqual(result.changed, false);
    assert.strictEqual(result.content, source);
  });

  test("leaves an existing strong dotted credential byte-for-byte unchanged", () => {
    const source = dottedRuntimeToml(
      "tcp://127.0.0.1:9902",
      "runtime.control.auth_token = 'already-configured-dotted-value' # keep formatting"
    );
    const result = migrateSource(source);

    assert.strictEqual(result.changed, false);
    assert.strictEqual(result.content, source);
  });

  test("does not alter non-Windows, Unix, remote TCP, missing-section, or malformed inputs", () => {
    const cases: ReadonlyArray<readonly [string, NodeJS.Platform]> = [
      [legacyRuntimeToml(), "linux"],
      [legacyRuntimeToml("unix:///tmp/trust-runtime.sock"), "win32"],
      [legacyRuntimeToml("tcp://192.168.50.42:9902"), "win32"],
      ["[runtime]\nmode = \"production\"\n", "win32"],
      [legacyRuntimeToml("tcp://127.0.0.1"), "win32"],
    ];

    for (const [source, platform] of cases) {
      const result = migrateSource(source, platform);
      assert.strictEqual(result.changed, false);
      assert.strictEqual(result.content, source);
    }
  });

  test("rejects unsafe, nested, mixed, duplicate, and malformed dotted control inputs", () => {
    const cases = [
      dottedRuntimeToml("tcp://192.168.50.42:9902"),
      `[mesh]\nruntime.control.endpoint = "tcp://127.0.0.1:9902"\n`,
      `runtime.control.endpoint = tcp://127.0.0.1:9902\n`,
      `runtime.control.endpoint = "tcp://127.0.0.1:9902"\n` +
        `runtime.control.endpoint = "tcp://localhost:9902"\n`,
      `runtime.control.endpoint = "tcp://127.0.0.1:9902"\n` +
        `runtime.control.auth_token = not-a-toml-string\n`,
      `runtime.control.auth_token = ""\n`,
      `runtime.control.endpoint = "tcp://127.0.0.1:9902"\n` +
        `[runtime.control]\nendpoint = "tcp://127.0.0.1:9902"\n`,
    ];

    for (const source of cases) {
      const result = migrateSource(source);
      assert.strictEqual(result.changed, false, source);
      assert.strictEqual(result.content, source);
    }
  });

  test("preserves CRLF, comments, unrelated TOML, and is idempotent", () => {
    const source = legacyRuntimeToml("tcp://localhost:9902", "\r\n");
    const first = migrateSource(source);
    const second = migrateSource(first.content, "win32", "must-not-be-used-123456789012345");

    assert.strictEqual(first.changed, true);
    assert.strictEqual(second.changed, false);
    assert.strictEqual(second.content, first.content);
    assert.strictEqual(first.content.replace(/\r\n/g, "").includes("\n"), false);
    assert.ok(first.content.includes("# Existing project; keep this comment\r\n"));
    assert.ok(first.content.includes("# keep endpoint note\r\n"));
  });

  test("migrates the root runtime.toml atomically and preserves permissions", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-win-runtime-root-"));
    const runtimeToml = path.join(root, "runtime.toml");
    try {
      fs.writeFileSync(runtimeToml, legacyRuntimeToml(), { mode: 0o640 });
      fs.chmodSync(runtimeToml, 0o640);
      const modeBefore = fs.statSync(runtimeToml).mode & 0o777;

      const first = migrateProject(root);
      const afterFirst = fs.readFileSync(runtimeToml, "utf8");
      const second = migrateProject(root);

      assert.strictEqual(first.changed, true);
      assert.strictEqual(first.path, runtimeToml);
      assert.ok(tokenFrom(afterFirst).length >= 24);
      assert.strictEqual(second.changed, false);
      assert.strictEqual(fs.readFileSync(runtimeToml, "utf8"), afterFirst);
      assert.strictEqual(fs.statSync(runtimeToml).mode & 0o777, modeBefore);
      assert.deepStrictEqual(
        fs.readdirSync(root).filter((entry) => entry.includes(".trust-migrate-")),
        []
      );
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("migrates a dotted root runtime.toml atomically and preserves permissions", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-win-runtime-dotted-"));
    const runtimeToml = path.join(root, "runtime.toml");
    try {
      const source = dottedRuntimeToml("tcp://localhost:9902");
      fs.writeFileSync(runtimeToml, source, { mode: 0o640 });
      fs.chmodSync(runtimeToml, 0o640);
      const modeBefore = fs.statSync(runtimeToml).mode & 0o777;

      const first = migrateProject(root);
      const afterFirst = fs.readFileSync(runtimeToml, "utf8");
      const second = migrateProject(root);

      assert.strictEqual(first.changed, true);
      assert.strictEqual(first.path, runtimeToml);
      assert.strictEqual(tokenFrom(afterFirst).length >= 24, true);
      assert.ok(afterFirst.includes("# keep dotted endpoint note"));
      assert.strictEqual(second.changed, false);
      assert.strictEqual(fs.readFileSync(runtimeToml, "utf8"), afterFirst);
      assert.strictEqual(fs.statSync(runtimeToml).mode & 0o777, modeBefore);
      assert.deepStrictEqual(
        fs.readdirSync(root).filter((entry) => entry.includes(".trust-migrate-")),
        []
      );
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("falls back to bundle/runtime.toml and skips missing or non-Windows projects", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-win-runtime-bundle-"));
    const bundle = path.join(root, "bundle");
    const runtimeToml = path.join(bundle, "runtime.toml");
    try {
      fs.mkdirSync(bundle);
      fs.writeFileSync(runtimeToml, legacyRuntimeToml(), "utf8");

      const linuxResult = migrateProject(root, "linux");
      assert.strictEqual(linuxResult.changed, false);
      assert.strictEqual(fs.readFileSync(runtimeToml, "utf8"), legacyRuntimeToml());

      const windowsResult = migrateProject(root, "win32");
      assert.strictEqual(windowsResult.changed, true);
      assert.strictEqual(windowsResult.path, runtimeToml);
      assert.ok(tokenFrom(fs.readFileSync(runtimeToml, "utf8")).length >= 24);

      assert.strictEqual(migrateProject(path.join(root, "missing")).changed, false);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test("runs before launch control injection and logs no credential or popup", () => {
    const launchControlSource = fs.readFileSync(
      path.resolve(
        __dirname,
        "..",
        "..",
        "..",
        "src",
        "debug",
        "launchControl.ts"
      ),
      "utf8"
    );
    const debugSource = fs.readFileSync(
      path.resolve(__dirname, "..", "..", "..", "src", "debug.ts"),
      "utf8"
    );
    const migrationCall = launchControlSource.indexOf(
      "migrateWindowsRuntimeControlProject(migrationRoot, platform)"
    );
    const launchInjection = launchControlSource.indexOf(
      "applyLaunchControlEndpoint(",
      migrationCall
    );
    const seam = launchControlSource.slice(migrationCall, launchInjection);

    assert.ok(migrationCall >= 0, "Launch resolution must invoke the migration.");
    assert.ok(
      launchInjection > migrationCall,
      "Migration must run before per-workspace launch control injection."
    );
    assert.ok(debugSource.includes("const launchControl = prepareLaunchControl("));
    assert.ok(
      debugSource.includes(
        'debugChannel().appendLine("Secured Windows local runtime control authentication in runtime.toml.")'
      ),
      "The only user-facing migration text must be the safe success log."
    );
    assert.ok(!launchControlSource.includes("showInformationMessage"));
    assert.ok(!launchControlSource.includes("showErrorMessage"));
    assert.ok(!seam.includes("authToken"));
  });

  test("Windows Extension Host migrates a tokenless dotted project and starts the real simulator", async function () {
    if (process.platform !== "win32") {
      this.skip();
      return;
    }
    this.timeout(60_000);

    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    assert.ok(extension, "Expected the truST extension in the Windows test host.");
    await extension!.activate();
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder, "Expected a Windows extension-test workspace.");

    const files = new Map<string, Uint8Array | undefined>();
    const remember = async (relative: string): Promise<vscode.Uri> => {
      const uri = vscode.Uri.joinPath(folder!.uri, ...relative.split("/"));
      try {
        files.set(relative, await vscode.workspace.fs.readFile(uri));
      } catch {
        files.set(relative, undefined);
      }
      return uri;
    };
    const runtimeToml = await remember("runtime.toml");
    const ioToml = await remember("io.toml");
    const projectToml = await remember("trust-lsp.toml");
    const mainSource = await remember("src/Main.st");
    const configSource = await remember("src/config.st");
    await vscode.workspace.fs.createDirectory(vscode.Uri.joinPath(folder.uri, "src"));
    const debugConfig = vscode.workspace.getConfiguration("trust", folder.uri);
    const previousAdapterPath = debugConfig.inspect<string>(
      "debugAdapter.executablePath"
    )?.workspaceValue;
    const adapterPath = process.env.ST_DEBUG_TEST_BIN?.trim();
    let adapterPathOverridden = false;

    const tokenless = tokenlessDottedGeneratedRuntimeToml();
    assert.strictEqual(tokenFrom(tokenless), "", "The E2E fixture must begin tokenless.");
    assert.ok(
      tokenless.includes('runtime.control.endpoint = "tcp://127.0.0.1:9902"'),
      "The Windows E2E regression must exercise the top-level dotted form."
    );
    await vscode.workspace.fs.writeFile(runtimeToml, Buffer.from(tokenless, "utf8"));
    await vscode.workspace.fs.writeFile(
      ioToml,
      Buffer.from('[io]\ndriver = "simulated"\nparams = {}\n', "utf8")
    );
    await vscode.workspace.fs.writeFile(
      projectToml,
      Buffer.from('include_paths = ["src"]\n', "utf8")
    );
    await vscode.workspace.fs.writeFile(
      mainSource,
      Buffer.from("PROGRAM Main\nEND_PROGRAM\n", "utf8")
    );
    await vscode.workspace.fs.writeFile(
      configSource,
      Buffer.from(
        [
          "CONFIGURATION Config",
          "RESOURCE MainRes ON PLC",
          "    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);",
          "    PROGRAM Main WITH MainTask : Main;",
          "END_RESOURCE",
          "END_CONFIGURATION",
          "",
        ].join("\n"),
        "utf8"
      )
    );

    const sessionName = "Windows token migration simulator E2E";
    let session: vscode.DebugSession | undefined;
    try {
      assert.ok(adapterPath, "ST_DEBUG_TEST_BIN must identify the adapter under test.");
      assert.ok(
        fs.existsSync(adapterPath),
        `The adapter under test does not exist: ${adapterPath}`
      );
      await debugConfig.update(
        "debugAdapter.executablePath",
        adapterPath,
        vscode.ConfigurationTarget.Workspace
      );
      adapterPathOverridden = true;
      const sessionStarted = waitForStructuredTextSession(sessionName);
      const started = await vscode.debug.startDebugging(folder, {
        type: "structured-text",
        request: "launch",
        name: sessionName,
        program: configSource.fsPath,
        runtimeRoot: folder.uri.fsPath,
        cwd: folder.uri.fsPath,
        stopOnEntry: false,
        internalConsoleOptions: "neverOpen",
      });
      assert.strictEqual(started, true, "VS Code must accept the real simulator launch.");
      session = await sessionStarted;

      const migrated = Buffer.from(
        await vscode.workspace.fs.readFile(runtimeToml)
      ).toString("utf8");
      assert.ok(tokenFrom(migrated).length >= 24, "Launch must migrate runtime.toml first.");
      const endpoint = session.configuration.controlEndpoint;
      const authToken = session.configuration.controlAuthToken;
      assert.match(endpoint, /^tcp:\/\/127\.0\.0\.1:\d+$/);
      assert.ok(typeof authToken === "string" && authToken.length >= 24);
      const schema = await waitForAuthenticatedControl(endpoint, authToken);
      assert.ok(schema && typeof schema === "object", "Authenticated comm.schema must respond.");
      const ioState = await requestIoStateEvent(session);
      assert.ok(
        ioState && typeof ioState === "object",
        "The launched adapter must serve DAP I/O through the stIoState event."
      );
    } finally {
      if (session) {
        await vscode.debug.stopDebugging(session);
      }
      if (adapterPathOverridden) {
        await debugConfig.update(
          "debugAdapter.executablePath",
          previousAdapterPath,
          vscode.ConfigurationTarget.Workspace
        );
      }
      for (const [relative, previous] of files) {
        const uri = vscode.Uri.joinPath(folder.uri, ...relative.split("/"));
        if (previous) {
          await vscode.workspace.fs.writeFile(uri, previous);
        } else {
          try {
            await vscode.workspace.fs.delete(uri, { useTrash: false });
          } catch {
            // The fixture may already have been removed by a failed debug launch.
          }
        }
      }
    }
  });
});
