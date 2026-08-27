import * as assert from "assert";
import * as vscode from "vscode";

suite("Control Studio command", () => {
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

  test("opens and reuses a configured Control.PID loop", async () => {
    const loopDefinition = configuredLoop();
    const request = {
      schemaVersion: 1,
      source: "ConfiguredLoop",
      scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
      loopDefinition,
    };
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
    await new Promise((resolve) => setTimeout(resolve, 100));
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
    await new Promise((resolve) => setTimeout(resolve, 100));
    const tabs = vscode.window.tabGroups.all.flatMap((group) => group.tabs)
      .filter((tab) => tab.label === "Control Studio · Tank level");
    assert.strictEqual(tabs.length, 1, "retrying one loop request must reveal, not duplicate, the editor");
  });
});

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
