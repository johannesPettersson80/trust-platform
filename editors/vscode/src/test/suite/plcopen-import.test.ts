import * as assert from "assert";
import * as vscode from "vscode";

const PLCOPEN_IMPORT_COMMAND = "trust-lsp.plcopen.import";

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

async function readText(uri: vscode.Uri): Promise<string> {
  const data = await vscode.workspace.fs.readFile(uri);
  return Buffer.from(data).toString("utf8");
}

async function captureErrorMessages<T>(
  run: () => Thenable<T> | Promise<T>
): Promise<{ result: T; messages: string[] }> {
  const windowLike = vscode.window as unknown as {
    showErrorMessage: (...args: unknown[]) => Thenable<unknown>;
  };
  const original = windowLike.showErrorMessage;
  const messages: string[] = [];
  windowLike.showErrorMessage = (async (message: unknown) => {
    messages.push(String(message));
    return undefined;
  }) as (...args: unknown[]) => Thenable<unknown>;
  try {
    const result = await run();
    return { result, messages };
  } finally {
    windowLike.showErrorMessage = original;
  }
}

async function resolveImportedMainSource(
  projectUri: vscode.Uri
): Promise<vscode.Uri | undefined> {
  const candidates = [
    vscode.Uri.joinPath(projectUri, "src", "Main.st"),
    vscode.Uri.joinPath(projectUri, "sources", "Main.st"),
  ];
  for (const candidate of candidates) {
    if (await pathExists(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

function openPlcFixtureXml(): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <fileHeader companyName="OpenPLC Project" productName="OpenPLC Editor v3" />
  <types>
    <pous>
      <pou name="Main" pouType="PROGRAM">
        <body>
          <ST><![CDATA[
PROGRAM Main
VAR
    Rising : R_EDGE;
END_VAR
Rising(CLK := TRUE);
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
</project>
`;
}

function malformedPlcFixtureXml(): string {
  return `<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="Main" pouType="PROGRAM">
        <body>
          <ST><![CDATA[
PROGRAM Main
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
`;
}

suite("PLCopen import command (VS Code)", function () {
  this.timeout(60000);
  let fixturesRoot: vscode.Uri;

  suiteSetup(async () => {
    const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(workspaceFolder, "Expected a workspace folder for tests.");
    fixturesRoot = vscode.Uri.joinPath(
      workspaceFolder.uri,
      "tmp",
      "vscode-plcopen-import"
    );
    await vscode.workspace.fs.createDirectory(fixturesRoot);
    await delay(200);
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

  test("imports OpenPLC XML into a target project folder", async () => {
    const fixtureUri = vscode.Uri.joinPath(fixturesRoot, "openplc.xml");
    await vscode.workspace.fs.writeFile(
      fixtureUri,
      Buffer.from(openPlcFixtureXml(), "utf8")
    );

    const targetUri = vscode.Uri.joinPath(fixturesRoot, "imported-openplc");
    const imported = await vscode.commands.executeCommand<boolean>(
      PLCOPEN_IMPORT_COMMAND,
      {
        inputUri: fixtureUri,
        projectUri: targetUri,
        overwrite: true,
        openProject: false,
        openReport: false,
      }
    );

    assert.strictEqual(imported, true, "Expected PLCopen import to succeed.");

    const mainSourceUri = await resolveImportedMainSource(targetUri);
    const migrationReportUri = vscode.Uri.joinPath(
      targetUri,
      "interop",
      "plcopen-migration-report.json"
    );

    assert.strictEqual(
      mainSourceUri !== undefined,
      true,
      "Expected imported Main.st source file."
    );
    assert.strictEqual(
      await pathExists(migrationReportUri),
      true,
      "Expected migration report JSON."
    );

    const migrationJson = JSON.parse(await readText(migrationReportUri)) as {
      detected_ecosystem?: string;
      imported_pous?: number;
      applied_library_shims?: Array<{
        vendor?: string;
        source_symbol?: string;
        replacement_symbol?: string;
      }>;
    };

    assert.strictEqual(migrationJson.detected_ecosystem, "openplc");
    assert.strictEqual(migrationJson.imported_pous, 1);
    assert.ok(
      (migrationJson.applied_library_shims ?? []).some(
        (entry) =>
          entry.vendor === "openplc" &&
          entry.source_symbol === "R_EDGE" &&
          entry.replacement_symbol === "R_TRIG"
      ),
      "Expected OpenPLC shim entry in migration report."
    );
  });

  test("cancel paths do not perform import", async () => {
    const cancelAtInput = await vscode.commands.executeCommand<boolean>(
      PLCOPEN_IMPORT_COMMAND,
      {
        simulateCancelAt: "input",
      }
    );
    assert.strictEqual(cancelAtInput, false);

    const fixtureUri = vscode.Uri.joinPath(fixturesRoot, "cancel-project.xml");
    await vscode.workspace.fs.writeFile(
      fixtureUri,
      Buffer.from(openPlcFixtureXml(), "utf8")
    );

    const cancelledProjectUri = vscode.Uri.joinPath(
      fixturesRoot,
      "cancelled-project"
    );
    const cancelAtProject = await vscode.commands.executeCommand<boolean>(
      PLCOPEN_IMPORT_COMMAND,
      {
        inputUri: fixtureUri,
        projectUri: cancelledProjectUri,
        simulateCancelAt: "project",
      }
    );
    assert.strictEqual(cancelAtProject, false);
    assert.strictEqual(await pathExists(cancelledProjectUri), false);
  });

  test("existing non-empty target requires explicit overwrite", async () => {
    const fixtureUri = vscode.Uri.joinPath(fixturesRoot, "overwrite-check.xml");
    await vscode.workspace.fs.writeFile(
      fixtureUri,
      Buffer.from(openPlcFixtureXml(), "utf8")
    );

    const targetUri = vscode.Uri.joinPath(fixturesRoot, "overwrite-target");
    await vscode.workspace.fs.createDirectory(targetUri);
    const sentinelUri = vscode.Uri.joinPath(targetUri, "keep.txt");
    await vscode.workspace.fs.writeFile(sentinelUri, Buffer.from("keep-me"));

    const imported = await vscode.commands.executeCommand<boolean>(
      PLCOPEN_IMPORT_COMMAND,
      {
        inputUri: fixtureUri,
        projectUri: targetUri,
        overwrite: false,
        openProject: false,
        openReport: false,
      }
    );

    assert.strictEqual(imported, false, "Expected overwrite=false to stop import.");
    assert.strictEqual(await pathExists(sentinelUri), true);
    assert.strictEqual(await resolveImportedMainSource(targetUri), undefined);
  });

  test("missing input file is rejected", async () => {
    const missingInputUri = vscode.Uri.joinPath(fixturesRoot, "missing.xml");
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "missing-input-target");

    const imported = await vscode.commands.executeCommand<boolean>(
      PLCOPEN_IMPORT_COMMAND,
      {
        inputUri: missingInputUri,
        projectUri: targetUri,
        overwrite: true,
        openProject: false,
        openReport: false,
      }
    );

    assert.strictEqual(imported, false, "Expected missing input to fail import.");
    assert.strictEqual(await pathExists(targetUri), false);
  });

  test("malformed XML reports actionable import error message", async () => {
    const fixtureUri = vscode.Uri.joinPath(fixturesRoot, "malformed.xml");
    await vscode.workspace.fs.writeFile(
      fixtureUri,
      Buffer.from(malformedPlcFixtureXml(), "utf8")
    );
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "malformed-target");

    const { result, messages } = await captureErrorMessages(() =>
      vscode.commands.executeCommand<boolean>(PLCOPEN_IMPORT_COMMAND, {
        inputUri: fixtureUri,
        projectUri: targetUri,
        overwrite: true,
        openProject: false,
        openReport: false,
      })
    );

    assert.strictEqual(result, false, "Expected malformed XML import to fail.");
    assert.ok(
      messages.some((message) => message.includes("input XML is malformed")),
      `Expected malformed XML guidance, got: ${messages.join(" | ")}`
    );
    assert.ok(
      messages.some((message) => message.includes("failed to parse PLCopen XML")),
      `Expected parser detail in error message, got: ${messages.join(" | ")}`
    );
  });

  test("missing runtime binary reports actionable import launch error", async () => {
    const fixtureUri = vscode.Uri.joinPath(fixturesRoot, "missing-runtime.xml");
    await vscode.workspace.fs.writeFile(
      fixtureUri,
      Buffer.from(openPlcFixtureXml(), "utf8")
    );
    const targetUri = vscode.Uri.joinPath(fixturesRoot, "missing-runtime-target");

    const config = vscode.workspace.getConfiguration("trust-lsp");
    const previousRuntimePath =
      config.get<string>("runtime.cli.path") ?? "";
    const previousEnvRuntime = process.env.ST_RUNTIME_TEST_BIN;
    const missingRuntimePath = "/tmp/trust-runtime-does-not-exist";

    process.env.ST_RUNTIME_TEST_BIN = missingRuntimePath;
    await config.update(
      "runtime.cli.path",
      missingRuntimePath,
      vscode.ConfigurationTarget.Workspace
    );

    try {
      const { result, messages } = await captureErrorMessages(() =>
        vscode.commands.executeCommand<boolean>(PLCOPEN_IMPORT_COMMAND, {
          inputUri: fixtureUri,
          projectUri: targetUri,
          overwrite: true,
          openProject: false,
          openReport: false,
        })
      );

      assert.strictEqual(
        result,
        false,
        "Expected import to fail when trust-runtime is missing."
      );
      assert.ok(
        messages.some((message) =>
          message.includes("trust-runtime binary was not found")
        ),
        `Expected missing-runtime guidance, got: ${messages.join(" | ")}`
      );
      assert.ok(
        messages.some((message) =>
          message.includes("trust-lsp.runtime.cli.path")
        ),
        `Expected runtime path setting hint, got: ${messages.join(" | ")}`
      );
    } finally {
      if (previousEnvRuntime === undefined) {
        delete process.env.ST_RUNTIME_TEST_BIN;
      } else {
        process.env.ST_RUNTIME_TEST_BIN = previousEnvRuntime;
      }
      await config.update(
        "runtime.cli.path",
        previousRuntimePath || undefined,
        vscode.ConfigurationTarget.Workspace
      );
    }
  });
});
