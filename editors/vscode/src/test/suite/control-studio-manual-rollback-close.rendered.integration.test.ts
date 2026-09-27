import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio Manual rollback close recovery", () => {
  test("reopens a real ST rollback from fresh observed status without replaying it", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_MANUAL_ROLLBACK_CLOSE !== "1") this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.controller.modeSignal = "Main.Manual";
    loop.controller.modeWhenTrue = "Manual";
    loop.controller.manualOutputType = "LREAL";
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    const connections = require("../../controlStudio/runtimeConnection");
    const originalBind = connections.bindRuntimeConnection;
    let releaseRollback!: () => void;
    const acknowledgement = new Promise<void>(resolve => { releaseRollback = resolve; });
    let rollbackStarted!: () => void;
    const startedRollback = new Promise<void>(resolve => { rollbackStarted = resolve; });
    let rollbackCalls = 0;
    connections.bindRuntimeConnection = async (...args: unknown[]) => {
      const bound = await originalBind(...args);
      return { ...bound, request: async (operation: string, params: Record<string, unknown>) => {
        const response = await bound.request(operation, params);
        if (operation === "control.parameters.rollback") {
          rollbackCalls += 1;
          rollbackStarted();
          await acknowledgement;
        }
        return response;
      } };
    };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const parameters = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      const original = await parameters.client.status();
      assert.deepStrictEqual(original.values, { "Main.Kp": 1 });
      const stage = await parameters.client.review({ "Main.Kp": 20 });
      await parameters.client.apply(stage, false);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 20 }, "The actual ST task must observe the staged gain before rollback.");

      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10000);
      await inner("[...d.querySelectorAll('[data-control-studio-mode]')].find(b=>b.textContent.trim()==='Tune').click();return 'show engineer controls';");
      await inner("d.querySelector('[data-engineer-rollback]').click();return 'review actual rollback';");
      await pollValue("actual rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm actual rollback';");
      await startedRollback;
      assert.strictEqual(rollbackCalls, 1, "The first panel sends exactly one real rollback request.");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      releaseRollback();
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, session);
      const reopened = (body: string) => evaluateInWebview(connection, session, body);
      const status = await pollValue("fresh reopened Manual rollback status", () => reopened("return d.querySelector('[data-engineer-status]').textContent;"),
        value => value.includes("Closed-panel rollback is confirmed by the current runtime status."), 10000);
      assert.match(status, /confirmed by the current runtime status/);
      assert.match(await reopened("return d.querySelector('[data-engineer-evidence]').textContent;"), /Main\.Kp.*1/,
        "The reopened panel must show the actual observed restored gain.");
      assert.strictEqual(rollbackCalls, 1, "Reopening must inspect status rather than replaying the rollback.");
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 1 });
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT) {
        await reopened("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'show recovered status';");
        await new Promise(resolve => setTimeout(resolve, 1_250));
        captureRenderedWorkbench(process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT);
      }
    } finally {
      releaseRollback?.();
      connections.bindRuntimeConnection = originalBind;
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });

  test("requires recovery after an actual ST session restart", async function () {
    const sourceChanged = process.env.TRUST_CONTROL_STUDIO_MANUAL_ROLLBACK_PROGRAM_CHANGE === "1";
    if (!sourceChanged && process.env.TRUST_CONTROL_STUDIO_MANUAL_ROLLBACK_DEBUG_SESSION_CHANGE !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const sourcePath = path.join(project, "src", "Main.st");
    const originalSource = fs.readFileSync(sourcePath, "utf8");
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.controller.modeSignal = "Main.Manual";
    loop.controller.modeWhenTrue = "Manual";
    loop.controller.manualOutputType = "LREAL";
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    const connections = require("../../controlStudio/runtimeConnection");
    const originalBind = connections.bindRuntimeConnection;
    let releaseRollback!: () => void;
    const acknowledgement = new Promise<void>(resolve => { releaseRollback = resolve; });
    let rollbackStarted!: () => void;
    const startedRollback = new Promise<void>(resolve => { rollbackStarted = resolve; });
    let rollbackCalls = 0;
    connections.bindRuntimeConnection = async (...args: unknown[]) => {
      const bound = await originalBind(...args);
      return { ...bound, request: async (operation: string, params: Record<string, unknown>) => {
        const response = await bound.request(operation, params);
        if (operation === "control.parameters.rollback") {
          rollbackCalls += 1;
          rollbackStarted();
          await acknowledgement;
        }
        return response;
      } };
    };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const parameterConnection = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      const originalStatus = await parameterConnection.client.status();
      const stage = await parameterConnection.client.review({ "Main.Kp": 20 });
      await parameterConnection.client.apply(stage, false);
      assert.deepStrictEqual((await parameterConnection.client.status()).values, { "Main.Kp": 20 });

      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-rollback]').click();return 'review actual rollback';");
      await pollValue("actual rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'confirm actual rollback';");
      await startedRollback;
      assert.strictEqual(rollbackCalls, 1, "The closed panel dispatches only its original rollback request.");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");

      if (sourceChanged) {
        fs.writeFileSync(sourcePath, originalSource.replace("    Manual : BOOL := TRUE;", "    Manual : BOOL := TRUE;\n    ProgramRevisionMarker : DINT := 1;"));
      }
      assert.strictEqual((await runtimeLifecycleService.stopRuntime()).ok, true, "Stop the old ST program before reloading it.");
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true, "Start the changed ST program.");
      const reloadedConnection = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      const reloadedStatus = await reloadedConnection.client.status();
      assert.notStrictEqual(reloadedStatus.program_revision, originalStatus.program_revision,
        "The reopened panel must inspect the real new program/session identity, not a stubbed status field.");
      releaseRollback();

      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      session = await attachControlStudio(connection, port, session);
      const reopened = (body: string) => evaluateInWebview(connection, session, body);
      const status = await pollValue("changed-program Manual recovery", () => reopened("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /program or parameter binding changed.*unresolved.*fresh review/i.test(value), 15_000);
      assert.match(status, /program or parameter binding changed.*fresh review/i);
      assert.strictEqual(await reopened("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true",
        "A program change must not grant Apply authority from the closed panel.");
      assert.strictEqual(await reopened("return String(d.querySelector('[data-engineer-confirm]').hidden);"), "true",
        "The old confirmation token must remain unavailable after a program change.");
      assert.strictEqual(rollbackCalls, 1, "Reopen must not replay the old rollback after a changed program.");
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT) {
        await reopened("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'show changed program recovery';");
        await new Promise(resolve => setTimeout(resolve, 750));
        captureRenderedWorkbench(process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT);
      }
    } finally {
      releaseRollback?.();
      connections.bindRuntimeConnection = originalBind;
      fs.writeFileSync(sourcePath, originalSource);
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
