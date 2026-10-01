import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { setupTestPrompts } from "./control-studio-setup-test-prompts";

suite("Control Studio command", () => {
  test("rejects an incompatible typed manual output binding", () => {
    const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
    const loop = configuredLoop();
    const candidate = { ...loop, controller: { ...loop.controller, manualOutputSignal: "Main.ManualCommand", manualOutputType: "BOOL" } };
    assert.strictEqual(validateControlLoopDefinition(candidate, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "rejected");
  });
  test("controller transactions retain the runtime endpoint and credential selected at creation", async () => {
    const lifecycle = require("../../runtimeLifecycle").runtimeLifecycleService;
    const transport = require("../../runtimeControlClient");
    const originalSnapshot = lifecycle.snapshot;
    const originalToken = lifecycle.controlAuthToken;
    const originalSend = transport.sendRuntimeControlRequest;
    let endpoint = "tcp://first:1";
    const calls: unknown[][] = [];
    lifecycle.snapshot = async () => ({ status: { running: true, endpoint } });
    lifecycle.controlAuthToken = async (target: string) => `credential:${target}`;
    transport.sendRuntimeControlRequest = async (...args: unknown[]) => {
      calls.push(args);
      return { active_revision: "r1", confirmed: true, parameters: {} };
    };
    try {
      const client = await require("../../controlStudio/panel").runtimeControllerClient();
      await client.status("P1.Controller");
      endpoint = "tcp://second:2";
      await client.status("P1.Controller");
      assert.deepStrictEqual(calls.map(call => call.slice(0, 2)), [
        ["tcp://first:1", "credential:tcp://first:1"],
        ["tcp://first:1", "credential:tcp://first:1"],
      ], "retargeting the editor must not redirect a transaction's confirmation or rollback");
    } finally {
      lifecycle.snapshot = originalSnapshot;
      lifecycle.controlAuthToken = originalToken;
      transport.sendRuntimeControlRequest = originalSend;
    }
  });

  test("offers a configure command even when a project already has a loop", async () => {
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("trust-lsp.controlStudio.configure"), "engineers need a persistent route to add another loop");
  });

  test("accepts observation-only project loops without an activation signal", () => {
    const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
    const loop = configuredLoop();
    const definition = { ...loop,
      signals: { ...loop.signals, manipulatedVariable: { ...loop.signals.manipulatedVariable, direction: "read" } },
      experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 10, seed: 1 },
    };
    assert.strictEqual(validateControlLoopDefinition(definition, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted");
  });

  test("missing loop offers an existing examples route without writing configuration", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const originalMessage = vscode.window.showErrorMessage;
    const originalCommand = vscode.commands.executeCommand;
    const actions: string[] = [];
    const commands: unknown[][] = [];
    (vscode.window as any).showErrorMessage = async (_message: string, ...items: string[]) => {
      actions.push(...items);
      return "Choose an example";
    };
    (vscode.commands as any).executeCommand = async (...args: unknown[]) => { commands.push(args); };
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { openControlStudioCommand } = require("../../controlStudio/openCommand");
      await openControlStudioCommand({}, undefined, () => assert.fail("must not open an unbound panel"));
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
  test("discovers additional loop files and rejects duplicate IDs within a project", async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "trust-multiple-loops-"));
    const folder = { uri: vscode.Uri.file(root), name: "Engineer project", index: 0 };
    const { controlStudioRequestForWorkspace } = require("../../controlStudio/projectLoop");
    const original = vscode.window.showQuickPick;
    const loop = configuredLoop();
    let offered: vscode.QuickPickItem[] = [];
    (vscode.window as any).showQuickPick = async (items: vscode.QuickPickItem[]) => {
      offered = items;
      return items.find((item) => item.label === "Second loop");
    };
    try {
      fs.mkdirSync(path.join(root, "control-loops"));
      const additional = path.join(root, "control-loops", "second.json");
      fs.writeFileSync(additional, JSON.stringify({ ...loop, id: "loop:second", name: "Second loop" }));
      const single = await controlStudioRequestForWorkspace(loop.runtimeId, [folder]);
      assert.strictEqual(single.kind, "accepted", "additional definitions work without a legacy root file");
      assert.strictEqual(single.request.loopDefinition.id, "loop:second");
      fs.writeFileSync(path.join(root, "control-loop.json"), JSON.stringify(loop));
      const selected = await controlStudioRequestForWorkspace(loop.runtimeId, [folder]);
      assert.strictEqual(offered.length, 2, "one project can offer multiple loops");
      assert.strictEqual(selected.request.loopDefinition.id, "loop:second");
      (vscode.window as any).showQuickPick = async () => undefined;
      assert.strictEqual((await controlStudioRequestForWorkspace(loop.runtimeId, [folder])).kind, "cancelled");
      fs.writeFileSync(additional, JSON.stringify({ ...loop, name: "Duplicate identity" }));
      const duplicate = await controlStudioRequestForWorkspace(loop.runtimeId, [folder]);
      assert.strictEqual(duplicate.kind, "invalid", "duplicate identity cannot silently select a loop");
      assert.match(duplicate.reason, /duplicate.*loop:tank/i);
    } finally {
      (vscode.window as any).showQuickPick = original;
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
  test("configures a compiled custom open loop step by step and preserves its ST program", async function () {
    this.timeout(10000);
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const rootFile = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(rootFile) ? fs.readFileSync(rootFile) : undefined;
    const file = path.join(folder.uri.fsPath, "control-loops", "custom-pump.json");
    const sourceFile = path.join(folder.uri.fsPath, "CustomPumpTest.st");
    const sourceText = "PROGRAM CustomPumpTest\nVAR pressure : LREAL; power : LREAL; END_VAR\npower := 2.0 * (5.0 - pressure);\nEND_PROGRAM\n";
    fs.writeFileSync(sourceFile, sourceText, { flag: "wx" });
    const originalError = vscode.window.showErrorMessage;
    const originalInfo = vscode.window.showInformationMessage;
    const originalPick = vscode.window.showQuickPick;
    (vscode.window as any).showErrorMessage = async () => "Configure control loop";
    const values: Record<string, string> = {
      "Loop name": "Custom pump", "Controller instance": "PumpStation.Regulator",
      "Controller type": "MyPumpLaw", "Task period (ms)": "20",
      "Process unit": "bar", "Process minimum": "0", "Process maximum": "12",
      "Output unit": "%", "Output minimum": "0", "Output maximum": "100",
      "Observation duration (seconds)": "30",
      "Maximum rising output rate (output units/s)": "",
      "Maximum falling output rate (output units/s)": "",
    };
    const inputTitles: string[] = [];
    const input = async (options: vscode.InputBoxOptions) => {
      inputTitles.push(options.title ?? "");
      return values[options.title ?? ""];
    };
    const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
      if (options.title === "Setpoint") return items.find(item => item.label === "No setpoint (open loop)");
      if (options.title === "Process variable") return items.find(item => item.label === "PumpStation.Pressure");
      if (options.title === "Controller output") return items.find(item => item.label === "PumpStation.Power");
      return items[0];
    };
    const presenter = setupTestPrompts(input, pick);
    (vscode.window as any).showInformationMessage = async () => "Save loop";
    let opened: any;
    try {
      if (fs.existsSync(rootFile)) fs.unlinkSync(rootFile);
      // Configure control loop opens the setup form; controllers it does not set up use this step-by-step setup.
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const configured = await configureControlLoop("simulator", async () => ({
        ok: true,
        controlSymbols: { controllers: [{ instance: "PumpStation.Regulator", type: "MyPumpLaw",
          tasks: [{ name: "PumpTask", intervalMs: 20 }] }] },
        processImage: {
          commands: [{ symbol: "PumpStation.Power", address: "%QL0", type: "LREAL" }],
          feedbacks: [{ symbol: "PumpStation.Pressure", address: "%IL0", type: "LREAL" }],
        },
      }), presenter);
      if (configured.kind === "accepted") opened = configured.request;
      assert.ok(fs.existsSync(file), "guided setup saves the explicitly selected loop");
      const saved = JSON.parse(fs.readFileSync(file, "utf8"));
      assert.strictEqual(saved.schemaVersion, 2, "new loop saves use v2 without rewriting legacy files");
      assert.strictEqual(saved.controller.type, "MyPumpLaw");
      assert.strictEqual(saved.controller.taskPeriodMs, 20);
      assert.ok(!inputTitles.includes("Controller instance") && !inputTitles.includes("Controller type") && !inputTitles.includes("Task period (ms)"),
        "compiled controller identity and its sole periodic task should be selectable without retyping");
      assert.strictEqual(saved.signals.processVariable.symbol, "PumpStation.Pressure");
      assert.strictEqual(saved.topology, "open-loop");
      assert.strictEqual(saved.signals.setpoint, undefined);
      assert.strictEqual(saved.signals.manipulatedVariable.direction, "read");
      assert.strictEqual(opened.loopDefinition.id, saved.id);
      assert.strictEqual(fs.readFileSync(sourceFile, "utf8"), sourceText, "setup must not rewrite the custom control program");
      fs.unlinkSync(file);
      const existing = JSON.stringify({ ...saved, name: "Existing loop with this ID" });
      fs.writeFileSync(rootFile, existing);
      const { configureControlStudioCommand } = require("../../controlStudio/openCommand");
      // Other suites may leave a second workspace root; the command then asks for the project, answered here.
      (vscode.window as any).showQuickPick = async (items: readonly { folder?: vscode.WorkspaceFolder }[]) => items.find(item => item.folder?.uri.fsPath === folder.uri.fsPath);
      await configureControlStudioCommand({}, () => assert.fail("a duplicate ID must not open"), async () => ({
        ok: true, processImage: {
          commands: [{ symbol: "PumpStation.Power", address: "%QL0", type: "LREAL" }],
          feedbacks: [{ symbol: "PumpStation.Pressure", address: "%IL0", type: "LREAL" }],
        },
      }), presenter);
      assert.strictEqual(fs.existsSync(file), false, "setup must reject a duplicate ID before writing a second file");
      assert.strictEqual(fs.readFileSync(rootFile, "utf8"), existing);

    } finally {
      (vscode.window as any).showErrorMessage = originalError;
      (vscode.window as any).showInformationMessage = originalInfo;
      (vscode.window as any).showQuickPick = originalPick;
      if (previous) fs.writeFileSync(rootFile, previous);
      else if (fs.existsSync(rootFile)) fs.unlinkSync(rootFile);
      if (fs.existsSync(file)) fs.unlinkSync(file);
      fs.unlinkSync(sourceFile);
    }
  });
  test("keeps compiled controller picker entries legible when instance and type share a name", async () => {
    const { configureControlLoop } = require("../../controlStudio/configureLoop");
    let controllerItems: readonly any[] = [];
    const presenter = setupTestPrompts(
      async (options: vscode.InputBoxOptions) => options.title === "Loop name" ? "Controller labels" : options.title === "Task period (ms)" ? "100" : undefined,
      async (items: readonly any[], options: vscode.QuickPickOptions) => {
        if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") { controllerItems = items; return undefined; }
        return undefined;
      },
    );
    const result = await configureControlLoop("runtime:controller-labels", async () => ({
      ok: true,
      controlSymbols: {
        controllers: [
          { instance: "P1", type: "P1", tasks: [] },
          { instance: "P1.Controller", type: "Control.PID", tasks: [{ name: "MainTask", intervalMs: 100 }] },
        ],
      },
      processImage: {
        feedbacks: [{ symbol: "P1.ProcessValue", address: "%IL0", type: "LREAL" }],
        commands: [{ symbol: "P1.ManipulatedValue", address: "%QL0", type: "LREAL" }],
      },
    }), presenter);
    assert.strictEqual(result.kind, "cancelled");
    const container = controllerItems.find(item => item.label === "P1" && item.description === undefined);
    assert.ok(container, "the compiled container identity remains available");
    assert.strictEqual(container.description, undefined, "an equal instance/type must omit redundant picker metadata");
    const pid = controllerItems.find(item => item.label === "P1.Controller");
    assert.strictEqual(pid?.description, "Control.PID", "the actual PID identity and type remain visible");
  });
  test("multi-root discovery offers named loops and respects selection and cancellation", async function () {
    this.timeout(20000);
    const first = vscode.workspace.workspaceFolders?.[0];
    assert.ok(first);
    const firstFile = path.join(first.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(firstFile) ? fs.readFileSync(firstFile) : undefined;
    const secondPath = fs.mkdtempSync(path.join(os.tmpdir(), "trust-loop-choice-"));
    const { getSelectedRuntimeId } = require("../../selectedRuntime");
    const { controlStudioRequestForWorkspace } = require("../../controlStudio/projectLoop");
    const runtimeId = getSelectedRuntimeId();
    const original = vscode.window.showQuickPick;
    let offered: vscode.QuickPickItem[] = [];
    let cancel = false;
    (vscode.window as any).showQuickPick = async (items: vscode.QuickPickItem[]) => {
      offered = items;
      return cancel ? undefined : items.find((item) => item.label.includes("Second loop"));
    };
    const folders = [first, { uri: vscode.Uri.file(secondPath), name: "Second project", index: 1 }];
    try {
      fs.writeFileSync(firstFile, JSON.stringify({ ...configuredLoop(), runtimeId, name: "First loop" }));
      fs.writeFileSync(path.join(secondPath, "control-loop.json"), JSON.stringify({
        ...configuredLoop(), runtimeId, name: "Second loop", projectId: "project:second",
      }));
      const result = await controlStudioRequestForWorkspace(runtimeId, folders);
      assert.strictEqual(offered.length, 2, "multiple configured projects require a chooser");
      assert.ok(offered.some((item) => item.label.includes("First loop")));
      assert.ok(offered.some((item) => item.description?.includes("Second project")));
      assert.strictEqual(result.kind, "accepted", JSON.stringify(result));
      assert.strictEqual(result.request.scope.projectId, "project:second");
      cancel = true;
      const cancelled = await controlStudioRequestForWorkspace(runtimeId, folders);
      assert.strictEqual(cancelled.kind, "cancelled", "cancellation must not fall back to the first project");
    } finally {
      (vscode.window as any).showQuickPick = original;
      if (previous) fs.writeFileSync(firstFile, previous);
      else if (fs.existsSync(firstFile)) fs.unlinkSync(firstFile);
      fs.rmSync(secondPath, { recursive: true, force: true });
    }
  });
  test("offers the invalid loop configuration as a repair destination", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const original = vscode.window.showErrorMessage;
    const actions: unknown[] = [];
    (vscode.window as any).showErrorMessage = async (_message: string, ...items: unknown[]) => {
      actions.push(...items);
      return "Open configuration";
    };
    try {
      fs.writeFileSync(file, "{ invalid JSON");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open");
      assert.ok(actions.includes("Open configuration"), "an invalid loop must offer its repair destination");
      assert.strictEqual(vscode.window.activeTextEditor?.document.uri.fsPath, file);
    } finally {
      (vscode.window as any).showErrorMessage = original;
      if (previous) fs.writeFileSync(file, previous);
      else fs.unlinkSync(file);
    }
  });

  test("workspace discovery rejects incompatible units with the failing signal names", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loop.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const { getSelectedRuntimeId } = require("../../selectedRuntime");
    const { controlStudioRequestForWorkspace } = require("../../controlStudio/projectLoop");
    const loop = { ...configuredLoop(), runtimeId: getSelectedRuntimeId() };
    loop.signals.setpoint.unit = "degC";
    try {
      fs.writeFileSync(file, JSON.stringify(loop));
      const result = await controlStudioRequestForWorkspace(loop.runtimeId);
      assert.strictEqual(result.kind, "invalid", "discovery must validate signal relationships before opening");
      assert.match(result.reason, /setpoint.*processVariable/);
    } finally {
      if (previous) fs.writeFileSync(file, previous);
      else fs.unlinkSync(file);
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

  test("promotes an existing plant handoff to its configured loop in the same runtime", async () => {
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

  test("guided proportional setup saves a declared existing law without inventing a plant", async function () {
    this.timeout(10000);
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loops", "existing-proportional.json");
    const previous = fs.existsSync(file) ? fs.readFileSync(file) : undefined;
    const originalInfo = vscode.window.showInformationMessage;
    const values: Record<string, string> = {
      "Loop name": "Existing proportional",
      "Process unit": "bar",
      "Process minimum": "0",
      "Process maximum": "12",
      "Output unit": "%",
      "Output minimum": "0",
      "Output maximum": "100",
      "Gain minimum": "0",
      "Gain maximum": "25",
      "Observation duration (seconds)": "45",
      "Maximum rising output rate (output units/s)": "",
      "Maximum falling output rate (output units/s)": "",
    };
    const input = async (options: vscode.InputBoxOptions) => values[options.title ?? ""];
    const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
      if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") return items.find(item => item.label === "Main.Controller");
      if (options.title === "Process variable") return items.find(item => item.label === "Main.PV");
      if (options.title === "Controller output") return items.find(item => item.label === "Main.Output");
      if (options.title === "Setpoint") return items.find(item => item.label === "Main.SP");
      if (options.title === "Control workflow") return items.find(item => item.label === "Tune existing proportional controller");
      if (options.title === "Proportional gain") return items.find(item => item.label === "Main.Kp");
      if (options.title === "Manual command") return items.find(item => item.label === "Main.ManualCommand");
      if (options.title === "Manual mode") return items.find(item => item.label === "Main.Manual");
      if (options.title === "Manual mode polarity") return items.find(item => /TRUE means Manual/.test(item.label));
      if (options.title === "Control channels") return items.find(item => item.label === "Single channel");
      if (options.title === "Actuator capability") return items.find(item => item.label === "Continuous output");
      if (options.title?.endsWith(": applied actuator feedback")) return items.find(item => item.label === "Use the controller output");
      return undefined;
    };
    (vscode.window as any).showInformationMessage = async () => "Save loop";
    const presenter = setupTestPrompts(input, pick);
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const result = await configureControlLoop("runtime:guided-proportional", async () => ({
        ok: true,
        controlSymbols: {
          controllers: [{ instance: "Main.Controller", type: "ExistingProportional",
            tasks: [{ name: "MainTask", intervalMs: 20 }] }],
          symbols: [
            { symbol: "Main.Kp", type: "LREAL" },
            { symbol: "Main.ManualCommand", type: "LREAL" },
            { symbol: "Main.Manual", type: "BOOL" },
          ],
        },
        processImage: {
          feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }],
          commands: [{ symbol: "Main.Output", address: "%QL0", type: "LREAL" }],
          memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" },
            { symbol: "Main.ManualCommand", address: "%ML8", type: "LREAL" }],
        },
      }), presenter);
      assert.strictEqual(result.kind, "accepted", JSON.stringify(result));
      const saved: any = JSON.parse(fs.readFileSync(file, "utf8"));
      assert.deepStrictEqual(saved.controller.law, { kind: "proportional", gainSymbol: "Main.Kp" });
      assert.strictEqual(saved.controller.manualOutputSignal, "Main.ManualCommand");
      assert.strictEqual(saved.controller.manualOutputAddress, "%ML8", "retain the compiled Manual-command address for bounded Test execution");
      assert.strictEqual(saved.controller.manualOutputType, "LREAL", "retain the compiled manual-command type for guarded transfer");
      assert.strictEqual(saved.controller.modeSignal, "Main.Manual");
      assert.strictEqual(saved.controller.modeWhenTrue, "Manual");
      assert.deepStrictEqual(saved.parameterBinding, {
        loop_id: saved.id,
        task_name: "MainTask",
        parameters: [{ symbol: "Main.Kp", iec_type: "LREAL", unit: "%/bar", convention: "proportional gain", minimum: 0, maximum: 25 }],
        mode: { symbol: "Main.Manual", manual_value: true },
      });
      assert.strictEqual(saved.experiment.durationSeconds, 45, "setup records the engineer's observation duration");
      assert.ok(!("analysis" in saved) && !("analysisParameterSymbols" in saved) && !("plant" in saved) && !("plantId" in saved),
        "declaring an existing proportional controller must not fabricate analysis or a plant");
      const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
      assert.strictEqual(validateControlLoopDefinition(saved, { projectId: saved.projectId, runtimeId: saved.runtimeId }).kind, "accepted");
      const { controlStudioRequestForWorkspace } = require("../../controlStudio/projectLoop");
      const reopened = await controlStudioRequestForWorkspace(saved.runtimeId, [folder]);
      assert.strictEqual(reopened.kind, "accepted", "the saved definition must reopen through the project validator");
    } finally {
      (vscode.window as any).showInformationMessage = originalInfo;
      if (previous) fs.writeFileSync(file, previous);
      else if (fs.existsSync(file)) fs.unlinkSync(file);
    }
  });

  test("guided proportional setup rejects a duration that exceeds the automatic-identification sample budget", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loops", "sample-budget-proportional.json");
    let durationValidation: string | undefined;
    const values: Record<string, string> = {
      "Loop name": "Sample budget proportional",
      "Process unit": "bar", "Process minimum": "0", "Process maximum": "12",
      "Output unit": "%", "Output minimum": "0", "Output maximum": "100",
      "Gain minimum": "0", "Gain maximum": "25",
      "Maximum rising output rate (output units/s)": "",
      "Maximum falling output rate (output units/s)": "",
    };
    const input = async (options: vscode.InputBoxOptions) => {
      if (options.title === "Observation duration (seconds)") {
        durationValidation = options.validateInput?.("600") as string | undefined;
        return undefined;
      }
      return values[options.title ?? ""];
    };
    const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
      if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") return items.find(item => item.label === "Main.Controller");
      if (options.title === "Process variable") return items.find(item => item.label === "Main.PV");
      if (options.title === "Controller output") return items.find(item => item.label === "Main.Output");
      if (options.title === "Setpoint") return items.find(item => item.label === "Main.SP");
      if (options.title === "Control workflow") return items.find(item => item.label === "Tune existing proportional controller");
      if (options.title === "Proportional gain") return items.find(item => item.label === "Main.Kp");
      if (options.title === "Manual command") return items.find(item => item.label === "Main.ManualCommand");
      if (options.title === "Manual mode") return items.find(item => item.label === "Main.Manual");
      if (options.title === "Manual mode polarity") return items.find(item => /TRUE means Manual/.test(item.label));
      if (options.title === "Control channels") return items.find(item => item.label === "Single channel");
      if (options.title === "Actuator capability") return items.find(item => item.label === "Continuous output");
      if (options.title?.endsWith(": applied actuator feedback")) return items.find(item => item.label === "Use the controller output");
      return undefined;
    };
    const presenter = setupTestPrompts(input, pick);
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const result = await configureControlLoop("runtime:guided-proportional", async () => ({
        ok: true,
        controlSymbols: {
          controllers: [{ instance: "Main.Controller", type: "ExistingProportional",
            tasks: [{ name: "MainTask", intervalMs: 50 }] }],
          symbols: [
            { symbol: "Main.Kp", type: "LREAL" },
            { symbol: "Main.ManualCommand", type: "LREAL" },
            { symbol: "Main.Manual", type: "BOOL" },
          ],
        },
        processImage: {
          feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }],
          commands: [{ symbol: "Main.Output", address: "%QL0", type: "LREAL" }],
          memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }],
        },
      }), presenter);
      assert.match(durationValidation ?? "", /at most 599\.95 seconds at this task period \(12,000 samples\)/i,
        "600 seconds at 50 ms creates 12,001 scan samples and must explain the supported limit before save");
      assert.strictEqual(result.kind, "cancelled");
      assert.strictEqual(fs.existsSync(file), false, "an adapter-ineligible duration must not write a loop definition");
    } finally {
      if (fs.existsSync(file)) fs.unlinkSync(file);
    }
  });

  test("guided proportional setup reports the scan-rounded duration maximum", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loops", "scan-rounded-proportional.json");
    let durationValidation: string | undefined;
    const values: Record<string, string> = {
      "Loop name": "Scan rounded proportional",
      "Process unit": "bar", "Process minimum": "0", "Process maximum": "12",
      "Output unit": "%", "Output minimum": "0", "Output maximum": "100",
      "Gain minimum": "0", "Gain maximum": "25",
      "Maximum rising output rate (output units/s)": "",
      "Maximum falling output rate (output units/s)": "",
    };
    const input = async (options: vscode.InputBoxOptions) => {
      if (options.title === "Observation duration (seconds)") {
        durationValidation = options.validateInput?.("601") as string | undefined;
        return undefined;
      }
      return values[options.title ?? ""];
    };
    const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
      if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") return items.find(item => item.label === "Main.Controller");
      if (options.title === "Process variable") return items.find(item => item.label === "Main.PV");
      if (options.title === "Controller output") return items.find(item => item.label === "Main.Output");
      if (options.title === "Setpoint") return items.find(item => item.label === "Main.SP");
      if (options.title === "Control workflow") return items.find(item => item.label === "Tune existing proportional controller");
      if (options.title === "Proportional gain") return items.find(item => item.label === "Main.Kp");
      if (options.title === "Manual command") return items.find(item => item.label === "Main.ManualCommand");
      if (options.title === "Manual mode") return items.find(item => item.label === "Main.Manual");
      if (options.title === "Manual mode polarity") return items.find(item => /TRUE means Manual/.test(item.label));
      if (options.title === "Control channels") return items.find(item => item.label === "Single channel");
      if (options.title === "Actuator capability") return items.find(item => item.label === "Continuous output");
      if (options.title?.endsWith(": applied actuator feedback")) return items.find(item => item.label === "Use the controller output");
      return undefined;
    };
    const presenter = setupTestPrompts(input, pick);
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const result = await configureControlLoop("runtime:guided-proportional", async () => ({
        ok: true,
        controlSymbols: {
          controllers: [{ instance: "Main.Controller", type: "ExistingProportional",
            tasks: [{ name: "MainTask", intervalMs: 700 }] }],
          symbols: [
            { symbol: "Main.Kp", type: "LREAL" },
            { symbol: "Main.ManualCommand", type: "LREAL" },
            { symbol: "Main.Manual", type: "BOOL" },
          ],
        },
        processImage: {
          feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }],
          commands: [{ symbol: "Main.Output", address: "%QL0", type: "LREAL" }],
          memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }],
        },
      }), presenter);
      assert.match(durationValidation ?? "", /at most 599\.9 seconds at this task period \(12,000 samples\)/i,
        "the reported maximum must be a scan-rounded duration that the runtime adapter accepts");
      assert.strictEqual(result.kind, "cancelled");
      assert.strictEqual(fs.existsSync(file), false);
    } finally {
      if (fs.existsSync(file)) fs.unlinkSync(file);
    }
  });

  test("guided observation setup retains the one-hour observation limit", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loops", "one-hour-observation.json");
    const originalInfo = vscode.window.showInformationMessage;
    let durationValidation: string | undefined;
    const values: Record<string, string> = {
      "Loop name": "One hour observation",
      "Process unit": "bar", "Process minimum": "0", "Process maximum": "12",
      "Output unit": "%", "Output minimum": "0", "Output maximum": "100",
      "Maximum rising output rate (output units/s)": "",
      "Maximum falling output rate (output units/s)": "",
    };
    const input = async (options: vscode.InputBoxOptions) => {
      if (options.title === "Observation duration (seconds)") {
        durationValidation = options.validateInput?.("3600") as string | undefined;
        return durationValidation ? undefined : "3600";
      }
      return values[options.title ?? ""];
    };
    const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
      if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") return items.find(item => item.label === "Main.Controller");
      if (options.title === "Process variable") return items.find(item => item.label === "Main.PV");
      if (options.title === "Controller output") return items.find(item => item.label === "Main.Output");
      if (options.title === "Setpoint") return items.find(item => item.label === "Main.SP");
      if (options.title === "Control workflow") return items.find(item => item.label === "Observation");
      if (options.title === "Control channels") return items.find(item => item.label === "Single channel");
      if (options.title === "Actuator capability") return items.find(item => item.label === "Continuous output");
      if (options.title?.endsWith(": applied actuator feedback")) return items.find(item => item.label === "Use the controller output");
      return undefined;
    };
    (vscode.window as any).showInformationMessage = async () => "Save loop";
    const presenter = setupTestPrompts(input, pick);
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const result = await configureControlLoop("runtime:guided-observation", async () => ({
        ok: true,
        controlSymbols: { controllers: [{ instance: "Main.Controller", type: "ExistingProportional",
          tasks: [{ name: "MainTask", intervalMs: 50 }] }] },
        processImage: {
          feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }],
          commands: [{ symbol: "Main.Output", address: "%QL0", type: "LREAL" }],
          memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }],
        },
      }), presenter);
      assert.strictEqual(durationValidation, undefined, "ordinary observation keeps its documented one-hour limit");
      assert.strictEqual(result.kind, "accepted", JSON.stringify(result));
      assert.strictEqual(JSON.parse(fs.readFileSync(file, "utf8")).experiment.durationSeconds, 3600);
    } finally {
      (vscode.window as any).showInformationMessage = originalInfo;
      if (fs.existsSync(file)) fs.unlinkSync(file);
    }
  });

  test("cancelling the proportional workflow writes no loop file", async () => {
    const folder = vscode.workspace.workspaceFolders?.[0];
    assert.ok(folder);
    const file = path.join(folder.uri.fsPath, "control-loops", "cancelled-proportional.json");
    try {
      if (fs.existsSync(file)) fs.unlinkSync(file);
      const input = async (options: vscode.InputBoxOptions) => ({
        "Loop name": "Cancelled proportional",
        "Process unit": "bar", "Process minimum": "0", "Process maximum": "12",
        "Output unit": "%", "Output minimum": "0", "Output maximum": "100",
      } as Record<string, string>)[options.title ?? ""];
      const pick = async (items: readonly any[], options: vscode.QuickPickOptions) => {
        if (options.title === "Configure control loop") return items.find(item => item.folder?.uri.toString() === vscode.workspace.workspaceFolders?.[0]?.uri.toString());
      if (options.title === "Controller") return items.find(item => item.label === "Main.Controller");
        if (options.title === "Process variable") return items.find(item => item.label === "Main.PV");
        if (options.title === "Controller output") return items.find(item => item.label === "Main.Output");
        if (options.title === "Setpoint") return items.find(item => item.label === "Main.SP");
        if (options.title === "Control workflow") return undefined;
        return undefined;
      };
      const presenter = setupTestPrompts(input, pick);
      const { configureControlLoop } = require("../../controlStudio/configureLoop");
      const result = await configureControlLoop("runtime:guided-proportional", async () => ({
        ok: true,
        controlSymbols: { controllers: [{ instance: "Main.Controller", type: "ExistingProportional",
          tasks: [{ name: "MainTask", intervalMs: 20 }] }] },
        processImage: {
          feedbacks: [{ symbol: "Main.PV", address: "%IL0", type: "LREAL" }],
          commands: [{ symbol: "Main.Output", address: "%QL0", type: "LREAL" }],
          memory: [{ symbol: "Main.SP", address: "%ML0", type: "LREAL" }],
        },
      }), presenter);
      assert.strictEqual(result.kind, "cancelled");
      assert.strictEqual(fs.existsSync(file), false, "cancellation must not leave a partial loop definition");
    } finally {
      if (fs.existsSync(file)) fs.unlinkSync(file);
    }
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
