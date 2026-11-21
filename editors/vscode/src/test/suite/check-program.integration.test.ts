import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { CHECK_PROGRAM_COMMAND, __testClearCompileDiagnostics } from "../../checkProgram";
import { getTrustConfiguration } from "../../configuration";

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

async function captureWarningMessages<T>(
  run: () => Thenable<T> | Promise<T>
): Promise<{ result: T; messages: string[] }> {
  const windowLike = vscode.window as unknown as {
    showWarningMessage: (...args: unknown[]) => Thenable<unknown>;
  };
  const original = windowLike.showWarningMessage;
  const messages: string[] = [];
  windowLike.showWarningMessage = (async (message: unknown) => {
    messages.push(String(message));
    return undefined;
  }) as (...args: unknown[]) => Thenable<unknown>;
  try {
    const result = await run();
    return { result, messages };
  } finally {
    windowLike.showWarningMessage = original;
  }
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
        .getConfiguration("trust")
        .update(
          "runtime.executablePath",
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
    // The invalid runtime.toml's errors would stay in Problems after the project is gone, and shut
    // the Start and Debug gates of every later suite (specification 25 §6.6).
    __testClearCompileDiagnostics();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
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

  test("Compile reports an actionable missing runtime binary", async () => {
    const config = getTrustConfiguration();
    const previous = config.get<string>("runtime.executablePath") ?? "";
    await config.update(
      "runtime.executablePath",
      "/tmp/trust-runtime-does-not-exist",
      vscode.ConfigurationTarget.Workspace
    );
    try {
      const { result, messages } = await captureWarningMessages(() =>
        vscode.commands.executeCommand(CHECK_PROGRAM_COMMAND)
      );
      assert.strictEqual(result, undefined);
      assert.ok(
        messages.some((message) =>
          message.includes("Missing trust-runtime")
        ),
        `Expected missing-runtime guidance, got: ${messages.join(" | ")}`
      );
      assert.ok(
        messages.some((message) => message.includes("Runtime path")),
        `Expected runtime setting hint, got: ${messages.join(" | ")}`
      );
    } finally {
      await config.update(
        "runtime.executablePath",
        previous || undefined,
        vscode.ConfigurationTarget.Workspace
      );
    }
  });

  test("Compile reports an actionable runtime report version mismatch", async () => {
    const config = getTrustConfiguration();
    const previous = config.get<string>("runtime.executablePath") ?? "";
    const fakeRuntime = path.join(
      os.tmpdir(),
      `trust-runtime-version-mismatch-${process.pid}.sh`
    );
    fs.writeFileSync(
      fakeRuntime,
      '#!/usr/bin/env sh\nprintf \'{"version":99,"command":"check","ok":true,"status":"ok","errors":0,"warnings":0,"issues":[],"source_count":1}\\n\'\n'
    );
    fs.chmodSync(fakeRuntime, 0o755);
    await config.update(
      "runtime.executablePath",
      fakeRuntime,
      vscode.ConfigurationTarget.Workspace
    );
    try {
      const { result, messages } = await captureWarningMessages(() =>
        vscode.commands.executeCommand(CHECK_PROGRAM_COMMAND)
      );
      assert.strictEqual(result, undefined);
      assert.ok(
        messages.some((message) => message.includes("Runtime mismatch v99")),
        `Expected version mismatch guidance, got: ${messages.join(" | ")}`
      );
      assert.ok(
        messages.some((message) => message.includes("Update truST")),
        `Expected update/reinstall guidance, got: ${messages.join(" | ")}`
      );
    } finally {
      await config.update(
        "runtime.executablePath",
        previous || undefined,
        vscode.ConfigurationTarget.Workspace
      );
      fs.rmSync(fakeRuntime, { force: true });
    }
  });

  /**
   * A stand-in trust-runtime in `folder`: `check` names the toolchain missing with the code `toolchain` until
   * `toolchain install` has run, and then passes; `toolchain status` answers whether it ran. Every call is logged.
   */
  function toolchainStandIn(folder: string, code = "toolchain"): string {
    const script = path.join(folder, "trust-runtime");
    const log = path.join(folder, "calls.log");
    const mark = path.join(folder, "installed");
    const failed = JSON.stringify({
      version: 1, command: "check", ok: false, status: "failed", errors: 1, warnings: 0, source_count: 1,
      issues: [{ severity: "error", code, message: "the truST toolchain 0.24.75 is not installed" }],
    });
    const passed = JSON.stringify({
      version: 1, command: "check", ok: true, status: "ok", errors: 0, warnings: 0, source_count: 1, issues: [],
    });
    fs.writeFileSync(
      script,
      [
        "#!/bin/sh",
        `echo "$1 $2" >> '${log}'`,
        'if [ "$1" = toolchain ] && [ "$2" = status ]; then',
        `  if [ -f '${mark}' ]; then echo '{"installed": true, "folder": "/t", "source": "installed", "trust": "0.24.75"}';`,
        `  else echo '{"installed": false, "folder": null, "source": null, "trust": null}'; fi`,
        'elif [ "$1" = toolchain ]; then',
        `  touch '${mark}'; echo '{"event": "installed", "folder": "/t"}'`,
        `elif [ -f '${mark}' ]; then echo '${passed}'`,
        `else echo '${failed}'; exit 11; fi`,
        "",
      ].join("\n"),
      { mode: 0o755 }
    );
    return script;
  }

  async function withStandIn<T>(script: string, run: () => Promise<T>): Promise<T> {
    const config = getTrustConfiguration();
    const previous = config.get<string>("runtime.executablePath") ?? "";
    await config.update("runtime.executablePath", script, vscode.ConfigurationTarget.Workspace);
    try {
      return await run();
    } finally {
      await config.update("runtime.executablePath", previous || undefined, vscode.ConfigurationTarget.Workspace);
    }
  }

  test("Compile installs a missing toolchain and checks again", async function () {
    // Specification 44 §4.6 and specification 11 §6.8.1c: a project's crate is checked with the toolchain's cargo,
    // and a check that names the toolchain missing (code `toolchain`) makes the extension install it, as a launch
    // does, and check again. Catches a Compile that fails on a computer that never ran truST and so holds back the
    // Start whose launch would install the toolchain.
    if (process.platform === "win32") this.skip();
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-compile-toolchain-"));
    try {
      const report = await withStandIn(toolchainStandIn(folder), () =>
        Promise.resolve(vscode.commands.executeCommand<{ ok: boolean } | undefined>(CHECK_PROGRAM_COMMAND))
      );
      assert.strictEqual(report?.ok, true, "the second check passes");
      const calls = fs.readFileSync(path.join(folder, "calls.log"), "utf8").trim().split("\n");
      assert.deepStrictEqual(calls, ["check --project", "toolchain status", "toolchain install", "check --project"]);
    } finally {
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });

  test("Compile installs nothing for a failure that is not the toolchain's", async function () {
    // Catches a Compile that installs the toolchain, or checks twice, for any failed check.
    if (process.platform === "win32") this.skip();
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-compile-no-toolchain-"));
    try {
      const { result } = await withStandIn(toolchainStandIn(folder, "compile"), () =>
        captureWarningMessages(() => vscode.commands.executeCommand<{ ok: boolean } | undefined>(CHECK_PROGRAM_COMMAND))
      );
      assert.strictEqual(result?.ok, false, "the check fails as it said");
      const calls = fs.readFileSync(path.join(folder, "calls.log"), "utf8").trim().split("\n");
      assert.deepStrictEqual(calls, ["check --project"]);
    } finally {
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });
});
