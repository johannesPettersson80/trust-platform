import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

suite("Control Studio command", () => {
  test("offers a configure command even when a project already has a loop", async () => {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("trust-lsp.controlStudio.configure"), "engineers need a persistent route to add another loop");
  });

  test("missing loop offers an existing examples route without writing configuration", async function () {
    this.timeout(10000); // The command asks the engine, which starts with the first command that needs it.
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const originalMessage = vscode.window.showErrorMessage;
    const originalCommand = vscode.commands.executeCommand;
    const actions: string[] = [];
    const messages: string[] = [];
    const commands: unknown[][] = [];
    (vscode.window as any).showErrorMessage = async (message: string, ...items: string[]) => {
      messages.push(message);
      actions.push(...items);
      return "Choose an example";
    };
    (vscode.commands as any).executeCommand = async (...args: unknown[]) => { commands.push(args); };
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { openControlStudioCommand } = require("../../controlStudio/openCommand");
      // The engine decides the workspace's loop, so the command starts the engine from the extension's own folder. The window keeps its one
      // engine from the first command that starts it, so this context must be the extension's own in what it finds the binary by: the
      // test mode looks in the cargo target folder, where a context without a mode looks for a bundled binary or a name on the PATH, and
      // an engine that did not start would answer every later command of the window with its start failure.
      const context = { extensionPath: vscode.extensions.getExtension("trust-platform.trust-lsp")?.extensionPath ?? process.cwd(),
        extensionMode: vscode.ExtensionMode.Test, subscriptions: [] };
      await openControlStudioCommand(context, undefined, () => assert.fail("must not open an unbound panel"));
      // The offer is the engine's answer that the project declares no loop, not the start failure of an engine that did not run.
      assert.match(messages[0] ?? "", /no loop definition/, "the engine answered that the project has no loop file");
      assert.ok(actions.includes("Configure control loop"), "an existing engineer project needs its own setup route");
      assert.ok(actions.includes("Choose an example"), "missing configuration requires an actionable setup route");
      assert.deepStrictEqual(commands, [["trust.examples.start"]]);
      assert.strictEqual(fs.existsSync(file), false, "discovery must never synthesize bindings");
    } finally {
      (vscode.window as any).showErrorMessage = originalMessage;
      (vscode.commands as any).executeCommand = originalCommand;
      if (previous) fs.writeFileSync(file, previous);
    }
  });
  test("opens the configured project loop when invoked without arguments", async function () {
    this.timeout(6000);
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder, "the Extension Host fixture must have a workspace");
    const file = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const { getSelectedRuntimeId } = require("../../selectedRuntime");
    const loop = { ...configuredLoop(), name: "Configured workspace loop",
      runtimeId: getSelectedRuntimeId() };
    try {
      fs.writeFileSync(file, JSON.stringify(loop));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open");
      const opened = () => vscode.window.tabGroups.all.flatMap(group => group.tabs)
        .some(tab => tab.label === "Control Studio · Configured workspace loop");
      const deadline = Date.now() + 3000;
      while (!opened() && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 25));
      assert.ok(opened(),
        "no-argument open must bind the configured loop rather than open an unbound editor");
    } finally {
      if (previous) fs.writeFileSync(file, previous);
      else fs.unlinkSync(file);
    }
  });

  test("reuses the active debug session control token for its own runtime endpoint", () => {
    const lifecycle = require("../../runtimeLifecycle") as {
      runtimeSessionControlAuthToken?: (session: unknown, endpoint: string) => string | undefined;
    };
    assert.strictEqual(lifecycle.runtimeSessionControlAuthToken?.({
      configuration: {
        request: "launch",
        controlEndpoint: "unix:///tmp/runtime.sock",
        controlAuthToken: "session-token",
      },
    }, "unix:///tmp/runtime.sock"), "session-token");
    assert.strictEqual(lifecycle.runtimeSessionControlAuthToken?.({
      configuration: {
        request: "launch",
        controlEndpoint: "unix:///tmp/other.sock",
        controlAuthToken: "session-token",
      },
    }, "unix:///tmp/runtime.sock"), undefined);
  });

  test("opens one reusable full editor from a Simulation handoff", async () => {
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("trust-lsp.controlStudio.open"));
    const handoff = {
      schemaVersion: 1,
      handoffId: "command-test-1",
      projectId: "project:fixture",
      runtimeId: "runtime:local",
      plantId: "tank",
      savedConfigurationRevision: "sha256:saved",
      source: "Simulation",
      requestedContext: { mode: "Monitor", selection: "plant_summary" },
    };
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", handoff);
    await new Promise((resolve) => setTimeout(resolve, 100));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", handoff);
    await new Promise((resolve) => setTimeout(resolve, 100));
    const tabs = vscode.window.tabGroups.all.flatMap((group) => group.tabs)
      .filter((tab) => tab.label.startsWith("Control Studio · tank"));
    assert.strictEqual(tabs.length, 1, "retrying one handoff must reveal, not duplicate, the editor");
  });

  test("opens and reuses a configured Control.PID loop", async function () {
    this.timeout(10000);
    const loopDefinition = configuredLoop();
    const request = {
      schemaVersion: 1,
      source: "ConfiguredLoop",
      scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
      loopDefinition,
    };
    await withDeclaredLoop(loopDefinition, async () => {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      await new Promise((resolve) => setTimeout(resolve, 100));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      await new Promise((resolve) => setTimeout(resolve, 100));
      const tabs = vscode.window.tabGroups.all.flatMap((group) => group.tabs)
        .filter((tab) => tab.label === "Control Studio · Tank level");
      assert.strictEqual(tabs.length, 1, "retrying one loop request must reveal, not duplicate, the editor");
    });
  });

  test("promotes an existing plant handoff to its configured loop in the same runtime", async function () {
    this.timeout(10000);
    const loopDefinition = configuredLoop();
    const handoff = {
      schemaVersion: 1,
      handoffId: "command-test-promote",
      projectId: loopDefinition.projectId,
      runtimeId: loopDefinition.runtimeId,
      plantId: "tank",
      savedConfigurationRevision: "sha256:saved",
      source: "Simulation",
      requestedContext: { mode: "Monitor", selection: "plant_summary" },
    };
    await withDeclaredLoop(loopDefinition, async () => {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", handoff);
      await new Promise((resolve) => setTimeout(resolve, 100));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1,
        source: "ConfiguredLoop",
        scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
        loopDefinition,
      });
      await new Promise((resolve) => setTimeout(resolve, 100));

      const configuredTabs = vscode.window.tabGroups.all.flatMap((group) => group.tabs)
        .filter((tab) => tab.label === "Control Studio · Tank level");
      assert.strictEqual(configuredTabs.length, 1,
        "a configured loop must replace the unbound handoff surface even when their scope is identical");
    });
  });

});

/**
 * Runs `run` while the workspace's `control-loop.json` declares `loop`, as a project declares its loop. The engine decides which loop a
 * request opens from the workspace's files (`controlStudio/projectLoops`, specification 35, B1.2) and the panel checks no loop file, so a
 * request opens a loop only when a folder of the workspace declares it. The file the workspace had is put back afterwards.
 */
async function withDeclaredLoop(loop: ReturnType<typeof configuredLoop>, run: () => Promise<void>): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  assert.ok(folder, "the Extension Host fixture must have a workspace");
  const file = path.join(folder.uri.fsPath, "control-loop.json");
  const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
  try {
    fs.writeFileSync(file, JSON.stringify(loop));
    await run();
  } finally {
    if (previous) fs.writeFileSync(file, previous);
    else fs.rmSync(file, { force: true });
  }
}

function configuredLoop() {
  return {
    schemaVersion: 1,
    id: "loop:tank",
    name: "Tank level",
    revision: "sha256:loop-1",
    projectId: "project:sha256:abc",
    runtimeId: "runtime:local",
    controller: {
      instance: "TankController",
      type: "Control.PID",
      revision: "sha256:controller-1",
      action: "direct",
      taskPeriodMs: 10,
      modeSignal: "%MX0.0",
    },
    signals: {
      setpoint: { symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read", unit: "%", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 10 },
  };
}
