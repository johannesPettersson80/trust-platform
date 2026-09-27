import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";
import type { ControlStudioWorkflowResult } from "../../controlStudio/workflowResult";

suite("Routh lesson actual Studio workflow", () => {
  test("analyzes applies and records positive zero and negative shaft gains", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_ROUTH !== "1") this.skip();
    this.timeout(150000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await new Promise(resolve => setTimeout(resolve, 300));
    }
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      for (const gain of [1, 0, -1]) {
        await vscode.commands.executeCommand("workbench.action.closeAllEditors");
        await runtimeLifecycleService.stopRuntime();
        const start = await runtimeLifecycleService.startLocalSimulator();
        assert.strictEqual(start.ok, true, JSON.stringify(start));
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        const session = await attachControlStudio(connection, port);
        const inner = (body: string) => evaluateInWebview(connection, session, body);
        const stage = async (name: string) => inner(`Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='${name}').click();return 'stage';`);
        await pollValue("Routh ready", () => inner("return d.querySelector('[data-routh]')?.dataset.ready;"), value => value === "true", 10000);
        await stage("Tune");
        await inner(`d.querySelector('[data-routh-value]').value='${gain}';d.querySelector('[data-routh-run]').click();return 'calculate';`);
        await pollValue("Routh calculation", () => inner("return d.querySelector('[data-routh-status]').textContent;"), value => value.includes(`Kp = ${gain}`), 10000);
        const captures = process.env.TRUST_CONTROL_STUDIO_RUNTIME_SCREENSHOT_DIR;
        if (captures) {
          fs.mkdirSync(captures, { recursive: true });
          await vscode.commands.executeCommand("notifications.clearAll");
          await inner("d.querySelector('[data-routh]').scrollIntoView({block:'start'});return 'visible';");
          await new Promise(resolve => setTimeout(resolve, 300));
          captureRenderedWorkbench(path.join(captures, `routh-actual-${gain}-analysis.png`));
        }
        await inner("d.querySelector('[data-parameters-selected]').click();return 'selected';");
        await pollValue("gain draft", () => inner("return d.querySelector('[data-parameters-values]').value;"), value => {
          try { return JSON.parse(value)["P1.Kp"] === gain; } catch { return false; }
        }, 10000);
        await inner("d.querySelector('[data-parameters-review]').click();return 'review';");
        await pollValue("gain review", () => inner("return String(!d.querySelector('[data-parameters-apply]').disabled);"), value => value === "true", 10000);
        await inner("d.querySelector('[data-parameters-apply]').click();return 'apply';");
        await pollValue("gain observed", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("observed after ST"), 10000);
        await stage("Test");
        await pollValue("fresh shaft", async () => { await runtimeLifecycleService.requestIoState(); return inner("return d.querySelector('[data-control-studio-quality]').textContent;"); }, value => !/Unavailable|Waiting/i.test(value), 15000);
        await inner("d.querySelector('[data-test-review]').click();return 'review';");
        const review = await pollValue("test review response", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Simulation test ready") || value.includes("Fresh synchronized"), 10000);
        assert.ok(review.includes("Simulation test ready"), `Review must refresh live samples after opening historical results: ${review}`);
        await inner("d.querySelector('[data-test-approve]').click();return 'start';");
        await pollValue("running test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Test running"), 10000);
        await pollValue("restored test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 16000);
        await stage("Monitor");
        await pollValue("complete Test record", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => value.includes("Recording retained"), 10000);
        await stage("Identify");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-run]').disabled);"), "false",
          "a complete runtime-owned Test record must unlock Identify without a separate Manual Record");
        await inner("d.querySelector('[data-identify-run]').click();return 'qualify';");
        const qualification = await pollValue("Test record qualification", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
          value => !/Qualifying the retained recording/.test(value), 10000);
        assert.doesNotMatch(qualification, /Incomplete recording/i,
          "Identify must consume the Test runtime buffer rather than editor-observed snapshots");
        assert.strictEqual(await inner("var b=d.querySelector('[data-historical-result-badge]');return String(!b||b.getClientRects().length===0);"), "true", "a fresh recorded trial must not be visibly labelled Saved result");
        if (captures) {
          await stage("Monitor");
          await inner("var e=d.querySelector('[data-control-studio-recording-evidence]');e?.scrollIntoView({block:'center'});return e?.textContent||'';");
          await vscode.commands.executeCommand("notifications.clearAll");
          await new Promise(resolve => setTimeout(resolve, 300));
          captureRenderedWorkbench(path.join(captures, `routh-actual-${gain}-response.png`));
        }
        await stage("Validate");
        assert.strictEqual(await inner("return String(d.querySelector('[data-result-save]').disabled);"), "false", "a complete actual ST recording must be savable without a PID candidate");
        await inner("d.querySelector('[data-result-save]').click();return 'save';");
        await pollValue("saved response", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => value.includes("Saved revision"), 10000);
        await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result'&&e.data?.payload?.result)d.defaultView.__routhRun=e.data.payload.result;});var h=d.querySelector('[data-result-history]');h.value=Array.from(h.options).filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'open';");
        const json = await pollValue("retained response", () => inner("return d.defaultView.__routhRun?JSON.stringify(d.defaultView.__routhRun):'';"), value => value.length > 0, 10000);
        const result = JSON.parse(json) as ControlStudioWorkflowResult;
        const samples = result.recording.rawSamples;
        assert.ok(samples.length >= 490, "ten-second scan record");
        assert.strictEqual(samples[samples.length - 1].controllerMode, "Manual");
        const active = samples.filter(sample => sample.controllerMode === "Auto");
        assert.ok(active.length > 100);
        assert.ok(active.every(sample => sample.appliedMv !== null && Math.abs(sample.appliedMv) <= 20));
        const end = active[active.length - 1].pv!;
        if (gain === 1) assert.ok(Math.abs(end - .01) < .001, `positive gain ends at ${end}`);
        if (gain === 0) assert.ok(active.every(sample => Math.abs(sample.pv!) < 1e-10 && sample.appliedMv === 0));
        if (gain === -1) assert.ok(Math.min(...active.map(sample => sample.pv!)) < -.1, "negative gain drives away before bounded stop");
        fs.writeFileSync(path.join(project, `routh-actual-gain-${gain}.json`), json);
      }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("retains an incomplete actual Test prefix, blocks Identify, and requires a fresh reviewed retry", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_TEST_INCOMPLETE !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) {
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
      await new Promise(resolve => setTimeout(resolve, 300));
    }
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const recorder = require("../../controlStudio/runtimeRecording");
    const originalStop = recorder.RuntimeRecordingSession.prototype.stop;
    let actualRecording: any;
    recorder.RuntimeRecordingSession.prototype.stop = async function () {
      const recording = await originalStop.call(this);
      actualRecording = recording;
      const firstScan = recording.rawSamples[0]?.scan ?? 0;
      const lastScan = recording.rawSamples.at(-1)?.scan ?? firstScan;
      return {
        ...recording,
        evidence: {
          ...recording.evidence,
          completeness: "incomplete",
          actualCycleTimingAvailable: false,
          missingScanRanges: [{ first: lastScan + 1, last: lastScan + 1 }],
          reason: "Test-only retained incomplete runtime prefix after an aborted Test.",
        },
      };
    };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const start = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(start.ok, true, JSON.stringify(start));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const captures = process.env.TRUST_CONTROL_STUDIO_RUNTIME_SCREENSHOT_DIR;
      const stage = async (name: string) => inner(`Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()===${JSON.stringify(name)}).click();return 'stage';`);
      await stage("Test");
      await pollValue("fresh incomplete-Test fixture", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-quality]').textContent;");
      }, value => !/Unavailable|Waiting/i.test(value), 15_000);
      await inner("d.querySelector('[data-test-review]').click();return 'review';");
      const failedPlanId = await pollValue("reviewed Test plan", () => inner("return d.querySelector('[data-control-studio]').dataset.experimentPlanId||'';"),
        value => value.length > 0, 10_000);
      await inner("d.querySelector('[data-test-approve]').click();return 'start';");
      await pollValue("running Test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => value.includes("Test running"), 10_000);
      await new Promise(resolve => setTimeout(resolve, 250));
      await inner("d.querySelector('[data-test-abort]').click();return 'abort';");
      await pollValue("aborted Test terminal", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Aborted|Failed/i.test(value), 15_000);
      await stage("Monitor");
      const retained = JSON.parse(await pollValue("retained incomplete Test evidence", () => inner(
        "var e=d.querySelector('[data-control-studio-recording-evidence]');var i=d.querySelector('[data-identify-run]');return JSON.stringify({evidence:e?.textContent||'',identifyDisabled:Boolean(i?.disabled)});"
      ), value => {
        try { return JSON.parse(value).evidence.includes("Incomplete recording"); } catch { return false; }
      }, 10_000));
      assert.ok(actualRecording?.rawSamples.length >= 2, "the Test must retain an actual runtime prefix before the test-only incomplete classification");
      assert.match(retained.evidence, /Test-only retained incomplete runtime prefix/);
      assert.strictEqual(retained.identifyDisabled, true, "incomplete Test evidence must not unlock Identify");
      await stage("Identify");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-run]').disabled);"), "true");
      if (captures) {
        fs.mkdirSync(captures, { recursive: true });
        await inner("d.querySelector('[data-identify-state]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 300));
        captureRenderedWorkbench(path.join(captures, "test-incomplete-identify-blocked.png"));
      }
      await stage("Test");
      const terminalText = await inner("return d.querySelector('[data-test-state]').textContent;");
      assert.match(terminalText, /not qualified for Identify; review a new Test plan to retry/i);
      if (captures) {
        await inner("d.querySelector('[data-test-state]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 300));
        captureRenderedWorkbench(path.join(captures, "test-incomplete-retry-required.png"));
      }
      await inner("d.querySelector('[data-test-review]').click();return 'retry review';");
      const retryPlanId = await pollValue("fresh reviewed retry plan", () => inner("return d.querySelector('[data-control-studio]').dataset.experimentPlanId||'';"),
        value => value.length > 0 && value !== failedPlanId, 10_000);
      assert.notStrictEqual(retryPlanId, failedPlanId, "retry must use a distinct reviewed Test plan identity");
      if (captures) {
        fs.mkdirSync(captures, { recursive: true });
        await stage("Monitor");
        await inner("d.querySelector('[data-control-studio-recording-evidence]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 300));
        captureRenderedWorkbench(path.join(captures, "test-incomplete-prefix.png"));
      }
    } finally {
      recorder.RuntimeRecordingSession.prototype.stop = originalStop;
      connection.close();
      await runtimeLifecycleService.stopRuntime();
    }
  });

});
