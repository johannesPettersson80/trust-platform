import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, setWebviewValue, sleep } from "./plant-rendered-harness";
import { chooseHotHeaterSignal, clickHotHeaterControl, hotHeaterSelectedSignals } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

function value(sample: WorkflowResultSample, id: string): number {
  const signal = sample.additional?.find(signal => signal.id === id);
  assert.ok(signal && typeof signal.value === "number" && Number.isFinite(signal.value), `native ${id} at scan ${sample.scan}`);
  assert.strictEqual(signal.quality, "good");
  return signal.value;
}

suite("Control Studio heater response trials", () => {
  for (const trial of [0, 1]) {
    test(`trial ${trial} records and reopens the published sixty-second experiment`, async function () {
      if (process.env.TRUST_HEATER_RESPONSE_TRIAL !== String(trial)) this.skip();
      this.timeout(150000);
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
      const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
      const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
      assert.strictEqual(loop.controller.instance, "P1.Controller");
      assert.strictEqual(loop.controller.type, "Control.PID");
      assert.strictEqual(loop.signals.processVariable.address, "%IL0");
      assert.strictEqual(loop.signals.manipulatedVariable.address, "%IL8");
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      let opened = false;
      try {
        // Follow the published choice through a real editor document before the
        // fresh simulator starts; never patch a running program or its result.
        const document = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, "src/Main.st")));
        const editor = await vscode.window.showTextDocument(document);
        const text = document.getText();
        const match = /TrialKind : INT := [01];/.exec(text);
        assert.ok(match, "published trial initializer");
        const range = new vscode.Range(document.positionAt(match.index), document.positionAt(match.index + match[0].length));
        assert.strictEqual(await editor.edit(edit => edit.replace(range, `TrialKind : INT := ${trial};`)), true);
        assert.strictEqual(await document.save(), true);
        fs.writeFileSync(path.join(captures, "executed-Main.st"), document.getText());
        editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "01-trial-source.png"));
        assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        opened = true;
        await vscode.commands.executeCommand("workbench.action.closeSidebar");
        await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
        const session = await attachControlStudio(connection, port, "", loop.id);
        const inner = (body: string) => evaluateInWebview(connection, session, body);
        const click = (selector: string) => clickHotHeaterControl(inner, selector);
        const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
        const capture = async (name: string) => {
          await vscode.commands.executeCommand("notifications.clearAll"); await sleep(250);
          assert.strictEqual(await inner("await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));return String(d.querySelector('[data-control-studio-mode][aria-selected=true]').checkVisibility());"), "true",
            "capture must follow visible, painted stage navigation");
          await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
        };
        await pollValue("complete hot reset", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return JSON.stringify({pv:d.querySelector('[data-control-studio-pv]').textContent,mv:d.querySelector('[data-control-studio-mv]').textContent});");
        }, result => { const current = JSON.parse(result); return Number.parseFloat(current.pv) === 35 && Number.parseFloat(current.mv) === 75; }, 15000);
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        await pollValue("lesson diagnostics", () => inner("return String(d.querySelectorAll('[data-control-signals] .signal-row').length);"), count => Number(count) >= 10, 10000);
        const diagnostics = ["integral", "proportional", "request", ...(trial === 1 ? ["thermal-load"] : [])];
        for (const id of diagnostics) {
          const signal = loop.signals.additional.find((signal: { id: string }) => signal.id === id);
          await chooseHotHeaterSignal(inner, signal.symbol, true);
        }
        const selected = ["sp", "pv", "mv", ...diagnostics];
        await capture("02-hot-signals");
        await click('[data-plot-tab="response"]'); await stage("Test");
        await click("[data-test-review]");
        await pollValue("reviewed trial", () => inner("return String(d.querySelector('[data-test-approve]').hidden);"), hidden => hidden === "false", 10000);
        await capture("03-reviewed-trial");
        await click("[data-test-approve]");
        await pollValue("native trial running", () => inner("return d.querySelector('[data-test-state]').textContent;"), state => /Test running/.test(state), 10000);
        await stage("Monitor"); await sleep(1200); await capture("04-event-response");
        await stage("Test");
        await pollValue("complete sixty-second native trial", () => inner("return d.querySelector('[data-test-state]').textContent;"), state => /Completed/i.test(state), 75000);
        await stage("Monitor");
        await pollValue("complete native recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), state => /Recording retained/.test(state), 10000);
        await pollValue("applied heater stop", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
        }, current => Number.parseFloat(current) === 0, 5000);
        await capture("05-complete-response");
        await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',event=>{const m=event.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened'&&m.loopId===root.dataset.loopId)root.dataset.responseTrialOpenedResult=JSON.stringify(m.payload?.result);});return 'observe actual Open result';");
        await stage("Validate"); await click("[data-result-save]");
        await pollValue("saved result", () => inner("return d.querySelector('[data-result-state]').textContent;"), state => /Saved revision/.test(state), 10000);
        await capture("06-saved-result");
        assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Validate,\"]').dataset.status;"), "ready");
        assert.strictEqual(await inner("return String(d.querySelector('[data-plot-view=response]').checkVisibility());"), "true",
          "Validate must retain an explicitly live response before opening the saved native recording");
        const validationText = await inner("return d.querySelector('[data-workflow-pane=Validate]').innerText;");
        assert.ok(!/Control\.PID model comparison|Generate a candidate first|Every requirement reports/.test(validationText),
          "a saved recording without a candidate must show recording actions, not empty candidate-comparison content");
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        await chooseHotHeaterSignal(inner, "P1.RequestedPowerPct", false);
        await stage("Validate");
        if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);") !== "true") {
          await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
        }
        await revealControlForCapture(inner, "[data-result-history]");
        const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(option=>option.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
        await setWebviewValue(inner, "[data-result-history]", revision); await click("[data-result-open]");
        const result = JSON.parse(await pollValue("opened recorded trial", () => inner("return d.querySelector('[data-control-studio]').dataset.responseTrialOpenedResult??'';"), result => result.length > 0, 10000)) as ControlStudioWorkflowResult;
        fs.writeFileSync(path.join(captures, "native-result.json"), JSON.stringify(result, null, 2));
        assert.strictEqual(result.controller.instance, "P1.Controller");
        assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
        assert.strictEqual(result.recording.acquisition?.completeness, "complete");
        assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
        const active = result.recording.rawSamples.filter(sample => value(sample, "experiment-authority") === 1);
        assert.ok(active.length >= 600, `need the complete sixty-second trial, got ${active.length} scans`);
        assert.ok(Math.abs(value(active[0], "request") - (trial === 0 ? 81.15 : 75)) < 1e-8);
        let lateError = 0;
        for (let index = 0; index < 600; index++) {
          const sample = active[index];
          assert.strictEqual(sample.quality, "good");
          assert.strictEqual(sample.sp, trial === 0 ? 38 : 35);
          assert.strictEqual(value(sample, "thermal-load"), trial === 0 ? 0 : 30);
          assert.ok(Math.abs(value(sample, "error") - (sample.sp! - sample.pv!)) < 1e-8);
          assert.ok(sample.appliedMv! >= 0 && sample.appliedMv! <= 100);
          assert.ok(trial === 0 ? sample.pv! <= 39 : sample.pv! >= 33);
          if (index > 0) assert.strictEqual(sample.scan, active[index - 1].scan + 1);
          if (index >= 300) lateError = Math.max(lateError, Math.abs(sample.pv! - sample.sp!));
        }
        assert.ok(Math.abs(lateError - (trial === 0 ? 0.07512128 : 0.21201012)) < 1e-6, `preserved native physical result: ${lateError}`);
        assert.strictEqual(lateError <= 0.2, trial === 0, "retain the failed disturbance requirement for diagnosis");
        fs.writeFileSync(path.join(captures, "trial-measurements.json"), JSON.stringify({ trial, activeSamples: active.length, lateError, meetsRequirement: lateError <= 0.2 }, null, 2));
        await capture("07-opened-validation");
        assert.strictEqual(await inner("return d.querySelector('[data-control-plot-tabs] [aria-selected=true]')?.dataset.plotTab??'';"), "recorded-response");
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        for (const id of selected) assert.ok((await hotHeaterSelectedSignals(inner)).includes(id), `Open restores ${id}`);
        await capture("07-reopened-signals");
        await click('[data-plot-tab="response"]'); await capture("08-reopened-response");
        await click('[data-plot-tab="signals"]');
        for (const symbol of [loop.signals.setpoint.symbol, loop.signals.processVariable.symbol,
          loop.signals.manipulatedVariable.symbol, ...diagnostics.map(id => loop.signals.additional.find((signal: { id: string }) => signal.id === id).symbol)]) {
          await chooseHotHeaterSignal(inner, symbol, false);
        }
        await chooseHotHeaterSignal(inner, "P1.ErrorC", true);
        await click('[data-plot-tab="response"]'); await capture("09-error-response");
        await click('[data-chart-action="table"]'); await capture("10-error-samples");
      } finally {
        await runtimeLifecycleService.stopRuntime();
        if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
        connection.close();
      }
    });
  }
});
