import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import {
  captureRenderedWorkbench,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
  type CdpConnection,
} from "./plant-rendered-harness";

suite("Rendered Control Studio", () => {
  test("calculates a provided shaft root locus without a live PID instance", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(60000);
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    const request = loopRequest();
    const loop = { ...request.loopDefinition, schemaVersion: 2,
      id: "loop:engineer-shaft", name: "Engineer shaft", plantId: undefined,
      signals: {
        setpoint: { ...request.loopDefinition.signals.setpoint, unit: "rad" },
        processVariable: { ...request.loopDefinition.signals.processVariable, unit: "rad" },
        manipulatedVariable: { ...request.loopDefinition.signals.manipulatedVariable, unit: "N·m", range: { minimum: -20, maximum: 20 } },
      },
      controller: { ...request.loopDefinition.controller, type: "EngineerShaftLaw", design: "project" },
      analysis: {
        plant: { kind: "transfer-function", numerator: [1], denominator: [1, 1, 0], delaySeconds: 0,
          inputUnit: "N·m", outputUnit: "rad", revision: "shaft-model-1", source: "provided", validation: "unvalidated" },
        controller: { numerator: [{ parameter: "Kd" }, { parameter: "Kp" }], denominator: [1],
          parameters: { Kp: { value: 12, minimum: 0, maximum: 100, unit: "N·m/rad" },
            Kd: { value: 7, minimum: 0, maximum: 20, unit: "N·m·s/rad" } } },
        rootLocus: { parameter: "Kp", values: [12, 16, 20], selectedValue: 16 },
      },
    };
    const decoded = require("../../controlStudio/loopDefinition").decodeControlLoopDefinition(loop);
    assert.strictEqual(decoded.kind, "accepted", "v2 supplied models must open without fabricated identification");
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { ...request, loopDefinition: loop });
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const version = await httpJson(port, "/json/version");
    const connection = await connectCdp(version.webSocketDebuggerUrl);
    try {
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("Tune pane", () => inner("var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune');b.click();var p=d.querySelector('[data-workflow-pane=Tune]');return String(p&&!p.hidden);"), value => value === "true", 10000);
      assert.strictEqual(await inner("var b=d.querySelector('[data-declared-root-run]');if(!b)return 'missing';b.click();return b.disabled?'disabled':'started';"), "started", "provided model calculation must be available offline");
      const result = await pollValue("calculated root locus", () => inner("var r=d.querySelector('[data-declared-root-result]');return r?r.textContent:'';"), text => text.includes("Kp = 20"), 10000);
      assert.strictEqual(await inner("var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune');return b.dataset.status;"), "complete", "a completed provided-model calculation must not still say Needs data");
      assert.match(result, /Kd = 7/);
      assert.match(result, /Kp = 12[\s\S]*-6[\s\S]*-2/);
      assert.match(result, /Kp = 16[\s\S]*-4/);
      assert.match(result, /Kp = 20[\s\S]*-4[\s\S]*2/);
      assert.match(result, /unvalidated/i);
      assert.strictEqual(await inner("var b=d.querySelector('[data-declared-root-select=\"20\"]');if(!b)return 'missing';b.click();return 'selected';"), "selected", "choose a candidate directly from the calculated locus");
      await pollValue("selected Kp candidate", () => inner("var r=d.querySelector('[data-declared-root-result] tr[data-selected=true]');return r?r.textContent:'';"), value => value.includes("Kp = 20"), 10000);
      assert.strictEqual(await inner("var b=d.querySelector('[data-declared-root-save]');if(!b)return 'missing';if(b.disabled)return 'disabled';b.click();return 'saving';"), "saving", "provided analysis has a visible save action");
      await pollValue("saved analysis", () => inner("var s=d.querySelector('[data-declared-root-save-status]');return s?s.textContent:'';"), value => value.includes("Saved revision"), 10000);
      const oldEditorSession = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { ...request, loopDefinition: loop });
      const restoredSession = await attachControlStudio(connection, port, oldEditorSession);
      await pollValue("restored selected gain", () => evaluateInWebview(connection, restoredSession,
        "var r=d.querySelector('[data-declared-root-result] tr[data-selected=true]');return r?r.textContent:'';"), value => value.includes("Kp = 20"), 10000);
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await sleep(300);
        captureRenderedWorkbench(screenshot);
      }
    } finally { connection.close(); }
  });

  test("project-owned loops can navigate every analysis stage before data is available", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(60000);
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const request = loopRequest();
    const loop = { ...request.loopDefinition,
      signals: { ...request.loopDefinition.signals, manipulatedVariable: { ...request.loopDefinition.signals.manipulatedVariable, direction: "read" } },
      experiment: { execution: "project-scripted", kind: "observation", amplitude: 0, durationSeconds: 10, seed: 1, activation: { symbol: "P1.Start", address: "%MX10.0", iecType: "BOOL", direction: "write" } },
    };
    assert.strictEqual(require("../../controlStudio/loopDefinition").validateControlLoopDefinition(loop, request.scope).kind, "accepted", "rendered fixture must be a valid binding");
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { ...request, loopDefinition: loop });
    await pollValue("Control Studio editor", async () => vscode.window.tabGroups.all.flatMap(g => g.tabs).some(t => t.label.startsWith("Control Studio")), value => value, 10000);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const version = await httpJson(port, "/json/version");
    const connection = await connectCdp(version.webSocketDebuggerUrl);
    try {
      const session = await attachControlStudio(connection, port);
      for (const stage of ["Identify", "Tune", "Validate"]) {
        const state = JSON.parse(await evaluateInWebview(connection, session, `
          var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()===${JSON.stringify(stage)});
          if(!b)return JSON.stringify({missing:true});
          b.click();
          var p=d.querySelector('[data-workflow-pane="${stage}"]');
          return JSON.stringify({disabled:b.disabled,visible:p&&!p.hidden,text:p&&p.innerText});`));
        assert.strictEqual(state.disabled, false, `${stage} must be reachable for a project-owned experiment`);
        await pollValue(`${stage} pane`, async () => evaluateInWebview(connection, session, `var b=Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()===${JSON.stringify(stage)});b.click();var p=d.querySelector('[data-workflow-pane="${stage}"]');return String(p&&!p.hidden);`), value => value === "true", 10000);
        assert.ok(state.text.length > 0, `${stage} must explain its workflow prerequisites`);
      }
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) captureRenderedWorkbench(screenshot);
    } finally { connection.close(); }
  });

  test("CPST-005 captures every runtime scan independently of editor refresh", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(120_000);

    const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
    const extension = vscode.extensions.getExtension("trust-platform.trust-lsp");
    if (extension) await extension.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) {
      await vscode.workspace.getConfiguration("workbench").update(
        "colorTheme", theme, vscode.ConfigurationTarget.Global
      );
      await sleep(300);
    }

    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    let connection: CdpConnection | undefined;
    try {
      const request = loopRequest();
      const configured = { ...request,
        loopDefinition: { ...request.loopDefinition, plantId: undefined,
          controller: { ...request.loopDefinition.controller, instance: "P1", type: "Main", design: "project" } } };
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", configured);
      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT === "1") {
        await vscode.commands.executeCommand("workbench.action.editorLayoutTwoColumns");
        const companion = await vscode.workspace.openTextDocument({
          content: "Narrow Control Studio recording evidence companion editor",
          language: "plaintext",
        });
        await vscode.window.showTextDocument(companion, { viewColumn: vscode.ViewColumn.Two, preview: false });
        await vscode.commands.executeCommand("workbench.action.focusFirstEditorGroup");
      }

      const userDataDir = requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR");
      const port = await devToolsPort(userDataDir, 15_000);
      const version = await pollValue("CDP endpoint", () => httpJson(port, "/json/version"),
        (value) => typeof value?.webSocketDebuggerUrl === "string", 15_000);
      connection = await connectCdp(version.webSocketDebuggerUrl);
      let sessionId = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection!, sessionId, body);

      await pollValue("first live Control Studio sample", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("var q=d.querySelector('[data-control-studio-quality]');return (q&&q.textContent||'').trim();");
      }, (value) => !/Unavailable|Waiting/i.test(value), 20_000);

      assert.strictEqual(
        await inner("return String(Boolean(d.querySelector('[data-control-studio-recording-evidence]')));"),
        "true",
        "Monitor must render a recording evidence status before recording starts",
      );

      assert.strictEqual(await inner("var b=d.querySelector('[data-control-studio-record-start]');if(!b)return 'NO_START';b.click();return 'STARTED';"), "STARTED");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), "Starting recording…",
        "The UI must not claim runtime recording before Start acknowledges");
      await pollValue("runtime recording acknowledged", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        value => value.includes("Recording at runtime"), 10000);
      for (let observation = 0; observation < 3; observation += 1) {
        await runtimeLifecycleService.requestIoState();
        await sleep(350);
      }
      const oldEditorSession = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { ...configured,
        loopDefinition: { ...configured.loopDefinition, revision: "sha256:changed-during-recording" } });
      sessionId = await attachControlStudio(connection, port, oldEditorSession);
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio]').dataset.loopRevision;"),
        configured.loopDefinition.revision, "an active recording must reopen its original binding revision");
      await pollValue("restored Stop recording action", () => inner("var b=d.querySelector('[data-control-studio-record-stop]');return String(Boolean(b&&!b.disabled));"), value => value === "true", 10000);
      assert.strictEqual(await inner("var b=d.querySelector('[data-control-studio-record-stop]');if(!b)return 'NO_STOP';b.click();return 'STOPPED';"), "STOPPED");

      const evidence = JSON.parse(await pollValue("rendered incomplete recording evidence", () => inner(
        "var s=d.querySelector('[data-control-studio-recording-evidence]');var identify=d.querySelector('[data-identify-run]');var text=(s&&s.textContent||'').trim();return JSON.stringify({text:text,identifyDisabled:Boolean(identify&&identify.disabled),identifyTitle:identify&&identify.title,body:(d.body&&d.body.innerText||'').slice(0,5000)});"
      ), (value) => {
        try { return /^(Incomplete|Complete) recording/.test(JSON.parse(value).text); } catch { return false; }
      }, 10_000));
      assert.match(evidence.text, /^Complete recording/, `runtime acquisition must retain scans that the editor did not poll: ${evidence.text}`);
      assert.strictEqual(evidence.identifyDisabled, false);
      assert.strictEqual(evidence.identifyTitle, "");

      if (process.env.TRUST_CONTROL_STUDIO_RENDERED_SPLIT === "1") {
        await inner("var s=d.querySelector('[data-control-studio-recording-evidence]');var studio=d.querySelector('.studio');if(s){s.scrollIntoView({block:'center'});if(studio)studio.scrollTop=studio.scrollHeight;}return 'SCROLLED';");
        await sleep(250);
        await inner("var studio=d.querySelector('.studio');if(studio)studio.scrollTop=studio.scrollHeight;return 'SCROLLED';");
        await sleep(250);
      }
      if (screenshot) captureRenderedWorkbench(screenshot);
    } finally {
      connection?.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });

  test("retains a manual runtime record and retries Stop after its Start acknowledgement is lost", async function () {
    const closePendingStart = process.env.TRUST_CONTROL_STUDIO_RECORDING_CLOSE_PENDING_START === "1";
    const hostReload = process.env.TRUST_CONTROL_STUDIO_MANUAL_RECORDING_ACK_HOST_RELOAD === "1";
    if (process.env.TRUST_CONTROL_STUDIO_MANUAL_RECORDING_ACK_LOSS !== "1" && !closePendingStart && !hostReload) this.skip();
    this.timeout(120_000);
    const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    const transport = require("../../runtimeControlClient");
    const originalSend = transport.sendRuntimeControlRequest;
    const recorder = require("../../controlStudio/runtimeRecording");
    const originalStop = recorder.RuntimeRecordingSession.prototype.stop;
    const originalRecover = recorder.RuntimeRecordingSession.recover;
    let connection: CdpConnection | undefined;
    let lostId: string | undefined; let recoveredId: string | undefined; let startCalls = 0; let blockFirstStop = true;
    let releaseStartReply: (() => void) | undefined;
    const stopIds: string[] = []; let retained: any;
    transport.sendRuntimeControlRequest = async (...args: any[]) => {
      const [, , operation, params] = args;
      if (operation === "control.recording.start") {
        startCalls += 1;
        const result = await originalSend(...args);
        if (!lostId) {
          lostId = params.recording_id;
          if (closePendingStart) await new Promise<void>(resolve => { releaseStartReply = resolve; });
          else throw new Error("controlled manual recording Start acknowledgement loss");
        }
        return result;
      }
      if (operation === "control.recording.stop") {
        stopIds.push(params.recording_id);
        if (blockFirstStop) throw new Error("controlled first manual recording Stop loss");
      }
      return originalSend(...args);
    };
    recorder.RuntimeRecordingSession.prototype.stop = async function () {
      const result = await originalStop.call(this); retained = result; return result;
    };
    recorder.RuntimeRecordingSession.recover = async function (loop: any, owner: any) {
      recoveredId = owner.recordingId;
      return originalRecover.call(this, loop, owner);
    };
    try {
      const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
      const loopDefinition = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      loopDefinition.controller.modeSignal = "Main.Manual";
      loopDefinition.controller.modeWhenTrue = "Manual";
      loopDefinition.controller.manualOutputType = "LREAL";
      const configured = { schemaVersion: 1, source: "ConfiguredLoop", scope: {
        projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId,
      }, loopDefinition };
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", configured);
      const runtime = await (require("../../controlStudio/runtimeConnection").bindRuntimeConnection(loopDefinition.runtimeId));
      let inspected: any;
      const bindings = [
        { id: "sp", address: loopDefinition.signals.setpoint.address, iec_type: loopDefinition.signals.setpoint.iecType },
        { id: "pv", address: loopDefinition.signals.processVariable.address, iec_type: loopDefinition.signals.processVariable.iecType },
        { id: "mv", address: loopDefinition.signals.manipulatedVariable.address, iec_type: loopDefinition.signals.manipulatedVariable.iecType },
        { id: "mode", symbol: "Main.Manual", iec_type: "BOOL" },
      ];
      for (let attempt = 0; attempt < 20; attempt += 1) {
        inspected = await runtime.request("control.signals.inspect", { loop_revision: loopDefinition.revision, bindings });
        if (inspected.available) break;
        await sleep(50);
      }
      assert.strictEqual(inspected.available, true, "Manual recording starts from a current coherent runtime scan");
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
      connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      let session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection!, session, body);
      if (hostReload) {
        const restored = JSON.parse(await inner("var text=d.querySelector('[data-control-studio-recording-evidence]').textContent;var start=d.querySelector('[data-control-studio-record-start]');var stop=d.querySelector('[data-control-studio-record-stop]');return JSON.stringify({text,startDisabled:start.disabled,stopDisabled:stop.disabled});"));
        if (/Recording recovery unconfirmed/.test(restored.text)) {
          assert.strictEqual(restored.startDisabled, true, "Reload must not offer another manual Start.");
          assert.strictEqual(restored.stopDisabled, false, "Reload must preserve Stop for the original owner.");
          assert.ok(recoveredId, "Reload must recover a persisted recording identity.");
          blockFirstStop = false;
          await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'stop recovered owner';");
          const terminal = JSON.parse(await pollValue("recovered lost-Start recording", () => inner("var text=d.querySelector('[data-control-studio-recording-evidence]').textContent;var start=d.querySelector('[data-control-studio-record-start]');var stop=d.querySelector('[data-control-studio-record-stop]');var identify=d.querySelector('[data-identify-run]');return JSON.stringify({text,startDisabled:start.disabled,stopDisabled:stop.disabled,identifyDisabled:identify.disabled});"), value => JSON.parse(value).text.includes("Start acknowledgement unconfirmed"), 15_000));
          assert.match(terminal.text, /Start acknowledgement unconfirmed \(Start acknowledgement was not confirmed before Extension Host reload\.\)/);
          assert.match(terminal.text, /Incomplete recording/i);
          assert.strictEqual(terminal.identifyDisabled, true);
          assert.strictEqual(startCalls, 0, "Recovery must not issue a replacement Start.");
          assert.deepStrictEqual(stopIds, [recoveredId], "Recovery Stop must use the persisted original ID.");
          assert.ok(retained?.rawSamples.length >= 2, "Recovered Stop must retain actual runtime samples.");
          if (screenshot) { await sleep(250); captureRenderedWorkbench(screenshot); }
          return;
        }
      }
      await pollValue("first live recording sample", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return (d.querySelector('[data-control-studio-quality]')?.textContent||'').trim();");
      }, value => !/Unavailable|Waiting/i.test(value), 20_000);

      assert.strictEqual(await inner("d.querySelector('[data-control-studio-record-start]').click();return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
        "Starting recording…", "Start is pending until its acknowledgement arrives");
      if (closePendingStart) {
        await pollValue("native Start accepted before panel close", async () => String(lostId ?? ""), value => value.length > 0, 10_000);
        const previousSession = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
        await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", configured);
        session = await attachControlStudio(connection, port, previousSession);
        const reopened = (body: string) => evaluateInWebview(connection!, session, body);
        const pending = JSON.parse(await pollValue("retained pending Start on reopen", () => reopened("var text=d.querySelector('[data-control-studio-recording-evidence]').textContent;var start=d.querySelector('[data-control-studio-record-start]');var stop=d.querySelector('[data-control-studio-record-stop]');return JSON.stringify({text,startDisabled:start.disabled,stopDisabled:stop.disabled});"),
          value => JSON.parse(value).text === "Starting recording…", 10_000));
        assert.strictEqual(pending.startDisabled, true);
        assert.strictEqual(pending.stopDisabled, false, "The reopened panel must retain Stop for the original owner");
        releaseStartReply!();
        await pollValue("reopened recording acknowledgement", () => reopened("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
          value => value.includes("Recording at runtime"), 10_000);
        blockFirstStop = false;
        await reopened("d.querySelector('[data-control-studio-record-stop]').click();return 'stop retained owner';");
        const terminal = await pollValue("reopened retained record", () => reopened("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"),
          value => value.includes("Complete recording"), 10_000);
        assert.match(terminal, /Complete recording/);
        assert.strictEqual(startCalls, 1, "Closing the panel must not replay Start");
        assert.deepStrictEqual(stopIds, [lostId]);
        assert.ok(retained?.rawSamples.length >= 2);
        if (screenshot) {
          await reopened("var evidence=d.querySelector('[data-control-studio-recording-evidence]');evidence?.scrollIntoView({block:'center'});return evidence?.textContent||'';");
          await sleep(1_250);
          captureRenderedWorkbench(screenshot);
        }
        return;
      }
      const uncertain = await pollValue("unconfirmed manual Start", () => inner("var text=d.querySelector('[data-control-studio-recording-evidence]').textContent;var start=d.querySelector('[data-control-studio-record-start]');var stop=d.querySelector('[data-control-studio-record-stop]');var identify=d.querySelector('[data-identify-run]');return JSON.stringify({text,startDisabled:start.disabled,stopDisabled:stop.disabled,identifyDisabled:identify.disabled});"),
        value => JSON.parse(value).text.includes("Start acknowledgement was not confirmed"), 15_000);
      const unresolved = JSON.parse(uncertain);
      assert.strictEqual(unresolved.startDisabled, true, "Do not offer another Start while the first owner is unresolved");
      assert.strictEqual(unresolved.stopDisabled, false, "Stop remains available to retry the original recording owner");
      assert.strictEqual(unresolved.identifyDisabled, true, "An unconfirmed Start cannot qualify a record");
      assert.ok(lostId);
      assert.deepStrictEqual(stopIds, [lostId], "The client attempted one Stop for the Start-owned ID");
      if (hostReload) {
        await vscode.commands.executeCommand("workbench.action.reloadWindow");
        await new Promise<void>(() => undefined);
      }
      if (screenshot) {
        await inner("var evidence=d.querySelector('[data-control-studio-recording-evidence]');evidence?.scrollIntoView({block:'center'});return evidence?.textContent||'';");
        await sleep(250);
        captureRenderedWorkbench(screenshot.replace(/\.png$/, "-unconfirmed.png"));
      }

      blockFirstStop = false;
      assert.strictEqual(await inner("d.querySelector('[data-control-studio-record-stop]').click();return 'retry';"), "retry");
      const terminal = JSON.parse(await pollValue("retained manual record", () => inner("var text=d.querySelector('[data-control-studio-recording-evidence]').textContent;var start=d.querySelector('[data-control-studio-record-start]');var stop=d.querySelector('[data-control-studio-record-stop]');var identify=d.querySelector('[data-identify-run]');return JSON.stringify({text,startDisabled:start.disabled,stopDisabled:stop.disabled,identifyDisabled:identify.disabled});"),
        value => JSON.parse(value).text.includes("stopped data retained"), 15_000));
      assert.match(terminal.text, /Start acknowledgement unconfirmed/i);
      assert.match(terminal.text, /Complete recording/i);
      assert.strictEqual(terminal.startDisabled, false);
      assert.strictEqual(terminal.stopDisabled, true);
      assert.strictEqual(terminal.identifyDisabled, true, "Actual samples are retained but the unconfirmed Start is not qualified evidence");
      assert.strictEqual(startCalls, 1);
      assert.deepStrictEqual(stopIds, [lostId, lostId], "Retry Stop retains the original runtime recording ID");
      assert.ok(retained?.rawSamples.length >= 2, "The actual runtime Stop result must survive the lost Start reply");
      assert.strictEqual(retained.evidence.observedSampleCount, retained.rawSamples.length);
      if (screenshot) {
        await inner("var evidence=d.querySelector('[data-control-studio-recording-evidence]');evidence?.scrollIntoView({block:'center'});return evidence?.textContent||'';");
        await sleep(250);
        captureRenderedWorkbench(screenshot);
      }
    } finally {
      recorder.RuntimeRecordingSession.prototype.stop = originalStop;
      recorder.RuntimeRecordingSession.recover = originalRecover;
      transport.sendRuntimeControlRequest = originalSend;
      connection?.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });
});

async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = ""): Promise<string> {
  return pollValue("Control Studio webview", async () => {
    const targets = await httpJson(port, "/json");
    for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      let sessionId: string | undefined;
      try {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
        if (!sessionId) continue;
        await connection.send("Runtime.enable", {}, sessionId);
        const marker = await evaluateInWebview(connection, sessionId,
          `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}));`);
        if (marker === "true") return sessionId;
        await connection.send("Target.detachFromTarget", { sessionId });
      } catch {
        if (sessionId) await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (value) => value.length > 0, 20_000);
}

function loopRequest() {
  const loopDefinition = {
    schemaVersion: 1,
    id: "loop:recording-gap",
    name: "20 ms recording gap",
    revision: "sha256:recording-gap-loop",
    projectId: "project:recording-gap",
    runtimeId: "simulator",
    plantId: "recording-gap-fopdt",
    controller: {
      instance: "P1.Controller",
      type: "Control.PID",
      revision: "sha256:recording-gap-controller",
      action: "direct",
      taskPeriodMs: 20,
      modeSignal: "%MX8.0",
    },
    signals: {
      setpoint: { symbol: "Setpoint", address: "%ML0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "ProcessValue", address: "%IL0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ManipulatedValue", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 20 },
  };
  return {
    schemaVersion: 1,
    source: "ConfiguredLoop",
    scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
    loopDefinition,
  };
}
