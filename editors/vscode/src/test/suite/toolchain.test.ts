import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import {
  ensureToolchain,
  INSTALL_TOOLCHAIN_COMMAND,
  INSTALL_TOOLCHAIN_FROM_FILE_COMMAND,
} from "../../toolchain";

// Specification 44 §4.6: before a debug launch the extension asks `trust-runtime toolchain status`
// and runs `trust-runtime toolchain install --json` only when no toolchain is installed. The
// runtime is a stand-in script that answers as trust-runtime does and logs how it was called.

/** A stand-in trust-runtime whose status answers `installed` and whose install prints `install`. */
function standIn(folder: string, installed: boolean, install: string[], exit = 0): string {
  const log = path.join(folder, "calls.log");
  const script = path.join(folder, "trust-runtime");
  const status = JSON.stringify({
    installed,
    folder: installed ? "/toolchains/0.24.75" : null,
    source: installed ? "installed" : null,
    trust: installed ? "0.24.75" : null,
  });
  fs.writeFileSync(
    script,
    [
      "#!/bin/sh",
      `echo "$*" >> '${log}'`,
      'if [ "$2" = status ]; then',
      `  echo '${status}'`,
      "else",
      ...install.map((line) => `  echo '${line}'`),
      `  exit ${exit}`,
      "fi",
      "",
    ].join("\n"),
    { mode: 0o755 }
  );
  return script;
}

function calls(folder: string): string[] {
  const log = path.join(folder, "calls.log");
  return fs.existsSync(log)
    ? fs.readFileSync(log, "utf8").trim().split("\n")
    : [];
}

suite("Toolchain install (VS Code)", function () {
  this.timeout(20000);
  let folder: string;
  const context = { extensionMode: vscode.ExtensionMode.Test } as vscode.ExtensionContext;
  const configuration = () => vscode.workspace.getConfiguration("trust");

  setup(function () {
    if (process.platform === "win32") {
      this.skip();
    }
    folder = fs.mkdtempSync(path.join(os.tmpdir(), "trust-toolchain-"));
  });

  teardown(async () => {
    await configuration().update(
      "runtime.executablePath",
      undefined,
      vscode.ConfigurationTarget.Global
    );
    fs.rmSync(folder, { recursive: true, force: true });
  });

  async function useRuntime(script: string): Promise<void> {
    await configuration().update(
      "runtime.executablePath",
      script,
      vscode.ConfigurationTarget.Global
    );
  }

  test("the install commands are registered", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes(INSTALL_TOOLCHAIN_COMMAND));
    assert.ok(commands.includes(INSTALL_TOOLCHAIN_FROM_FILE_COMMAND));
  });

  // Catches an extension that downloads 220 MB again before every launch.
  test("an installed toolchain is not installed again", async () => {
    await useRuntime(standIn(folder, true, []));
    assert.strictEqual(await ensureToolchain(context), true);
    assert.deepStrictEqual(calls(folder), ["toolchain status --json"]);
  });

  // Catches a first launch that never installs the toolchain, and an install whose JSON progress
  // is not read to its end.
  test("a missing toolchain is installed before the launch", async () => {
    await useRuntime(
      standIn(folder, false, [
        '{"event": "download", "bytes": 1048576, "total": 2097152}',
        '{"event": "download", "bytes": 2097152, "total": 2097152}',
        '{"event": "verify"}',
        '{"event": "installed", "folder": "/toolchains/0.24.75"}',
      ])
    );
    assert.strictEqual(await ensureToolchain(context), true);
    assert.deepStrictEqual(calls(folder), [
      "toolchain status --json",
      "toolchain install --json",
    ]);
  });

  // Catches a failed install reported as installed, and a warning without trust-runtime's reason.
  test("a failed install says why and offers the install from a file", async () => {
    await useRuntime(
      standIn(
        folder,
        false,
        ['{"event": "failed", "error": "the release v0.24.75 lists no trust-toolchain-linux-x64.tar.gz"}'],
        1
      )
    );
    const windowLike = vscode.window as unknown as {
      showWarningMessage: (...args: unknown[]) => Thenable<unknown>;
    };
    const original = windowLike.showWarningMessage;
    const warnings: unknown[][] = [];
    windowLike.showWarningMessage = (async (...args: unknown[]) => {
      warnings.push(args);
      return undefined;
    }) as (...args: unknown[]) => Thenable<unknown>;
    try {
      assert.strictEqual(await ensureToolchain(context), false);
    } finally {
      windowLike.showWarningMessage = original;
    }
    assert.deepStrictEqual(warnings, [
      [
        "The truST toolchain was not installed: the release v0.24.75 lists no trust-toolchain-linux-x64.tar.gz",
        "Install Toolchain from File",
      ],
    ]);
  });
});
