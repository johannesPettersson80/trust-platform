import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";
import { chooseHotHeaterSignal, clickHotHeaterControl, hotHeaterResponseRaster } from "./control-studio-hot-heater-helpers";

function additional(sample: WorkflowResultSample, id: string): number {
  const signal = sample.additional?.find(value => value.id === id);
  assert.ok(signal && typeof signal.value === "number" && Number.isFinite(signal.value), `missing ${id} at scan ${sample.scan}`);
  assert.strictEqual(signal.quality, "good");
  return signal.value;
}

function nearest(samples: readonly WorkflowResultSample[], seconds: number): WorkflowResultSample {
  assert.ok(samples.length > 0);
  return samples.reduce((best, sample) => Math.abs(sample.elapsedSeconds - seconds) < Math.abs(best.elapsedSeconds - seconds) ? sample : best);
}

suite("Control Studio energy-balance process", () => {
  test("runs the public energy-balance Test, observes native model/PV evidence, and reopens the saved result", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ENERGY_BALANCE !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-water-heater-energy-balance");
    assert.strictEqual(loop.topology, "open-loop");
    assert.strictEqual(loop.controller.type, "Main");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");

    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      opened = true;
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = (name: string) => clickHotHeaterControl(inner, `[data-control-studio-mode][aria-label^="${name},"]`);
      if (process.env.TRUST_HOT_HEATER_REOPEN_ONLY !== "1") {
        await stage("Monitor");
        await pollValue("initial applied power", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
        }, value => Number.parseFloat(value) === 75, 10_000);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-initial.png"));
  
        await stage("Test");
        await clickHotHeaterControl(inner, "[data-test-review]");
        await pollValue("reviewed energy-balance test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 10_000);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-reviewed.png"));
        await clickHotHeaterControl(inner, "[data-test-approve]");
        await pollValue("energy-balance test running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Test running/.test(value), 10_000);
  
        await stage("Monitor");
        await pollValue("observed applied 85 percent", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
        }, value => Number.parseFloat(value) === 85, 10_000);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-running-85.png"));
  
        await stage("Test");
        await pollValue("completed energy-balance test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 45_000);
        await stage("Monitor");
        await pollValue("restored zero output", async () => {
          await runtimeLifecycleService.requestIoState();
          return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
        }, value => Number.parseFloat(value) === 0, 10_000);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-completed.png"));
  
        await stage("Validate");
        await clickHotHeaterControl(inner, '[data-plot-tab="response"], [data-plot-tab="recorded-response"]');
        assert.match(await inner("return d.querySelector('[data-control-plot-tabs]').textContent;"), /Recorded Response/i);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-validate-before-save.png"));
        await clickHotHeaterControl(inner, "[data-result-save]");
        await pollValue("saved energy-balance result", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 10_000);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-saved.png"));
      }
      await stage("Validate");
      await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',event=>{const m=event.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened'&&m.loopId===root.dataset.loopId)root.dataset.energyBalanceResult=JSON.stringify(m.payload?.result);});return 'observe actual Open';");
      if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);" ) !== "true") await clickHotHeaterControl(inner, "[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(option=>option.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
      await inner(`const select=d.querySelector('[data-result-history]');select.value=${JSON.stringify(revision)};select.dispatchEvent(new Event('change',{bubbles:true}));return 'selected';`);
      await clickHotHeaterControl(inner, "[data-result-open]");
      const encoded = await pollValue("opened energy-balance result", () => inner("return d.querySelector('[data-control-studio]').dataset.energyBalanceResult??'';"), value => value.length > 0, 10_000);
      const result = JSON.parse(encoded) as ControlStudioWorkflowResult;
      fs.writeFileSync(path.join(captures, "energy-balance-native-result.json"), JSON.stringify(result, null, 2));
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-opened.png"));
      assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
      assert.strictEqual(result.recording.acquisition?.completeness, "complete");
      assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
      const active = result.recording.rawSamples.filter(sample => additional(sample, "experiment-authority") === 1);
      assert.ok(active.length >= 300);
      assert.ok(active.every((sample, index) => index === 0 || sample.scan === active[index - 1].scan + 1));
      assert.ok(active.every((sample, index) => index === 0 || Math.abs(sample.elapsedSeconds - active[index - 1].elapsedSeconds - 0.1) < 1e-8));
      assert.ok(active.every(sample => sample.sp == null), "the open-loop recording must not invent a setpoint channel");
      assert.ok(active.every(sample => Math.abs(sample.appliedMv! - 85) < 1e-8));
      assert.ok(result.recording.rawSamples.some(sample => additional(sample, "experiment-authority") === 0 && sample.appliedMv === 0), "the terminal record confirms restored zero output");
      assert.ok(active.every(sample => Math.abs(sample.pv! - additional(sample, "model")) < 0.05));
      assert.ok(Math.abs(nearest(active, active[0].elapsedSeconds + 6).pv! - 36.264) < 0.04);
      assert.ok(Math.abs(nearest(active, active[0].elapsedSeconds + 26).pv! - 36.986) < 0.01);
      assert.ok(Math.abs(additional(active[0], "delayed") - 75) < 1e-6);
      assert.ok(active.some(sample => Math.abs(additional(sample, "delayed") - 85) < 1e-6));
      assert.ok(active.some(sample => Math.abs(additional(sample, "rate") - 0.4) < 0.03));
      for (const sample of active) {
        const model = additional(sample, "model");
        const previousModel = model - 0.1 * additional(sample, "rate");
        const delayed = additional(sample, "delayed");
        const heater = additional(sample, "heat");
        const loss = additional(sample, "loss");
        const rate = additional(sample, "rate");
        assert.ok(Math.abs(heater - 2 * delayed) < 0.05);
        assert.ok(Math.abs(loss - 10 * (previousModel - 20)) < 0.1);
        assert.ok(Math.abs(rate - (heater - loss) / 50) < 0.02);
      }
      await clickHotHeaterControl(inner, "[data-plot-tab=signals]");
      const symbols = ["P1.ModelTemperatureC", "P1.TemperatureC", "P1.AppliedPowerPct", "P1.HeaterPowerKw", "P1.LossPowerKw", "P1.ModelRateCPerS", "P1.DelayedCommandPct"];
      const showGroup = async (name: string, group: readonly string[]) => {
        for (const symbol of symbols) await chooseHotHeaterSignal(inner, symbol, false);
        for (const symbol of group) await chooseHotHeaterSignal(inner, symbol, true);
        await clickHotHeaterControl(inner, '[data-plot-tab="response"], [data-plot-tab="recorded-response"]');
        assert.strictEqual(await inner("return String(!!d.querySelector('[data-control-studio-chart] canvas'));"), "true");
        assert.ok((await hotHeaterResponseRaster(inner)).length > 0, `${name} response canvas must contain readable pixels`);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `energy-balance-${name}.png`));
        await clickHotHeaterControl(inner, "[data-plot-tab=signals]");
      };
      await showGroup("model-pv-input", ["P1.ModelTemperatureC", "P1.TemperatureC", "P1.AppliedPowerPct"]);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-model-signals.png"));
      await clickHotHeaterControl(inner, '[data-plot-tab="response"], [data-plot-tab="recorded-response"]');
      await clickHotHeaterControl(inner, '[data-chart-action="table"]');
      assert.match(await inner("return d.querySelector('[data-chart-table] thead').textContent;"), /Model/i);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "energy-balance-model-samples.png"));
      await clickHotHeaterControl(inner, '[data-chart-action="table"]');
      await clickHotHeaterControl(inner, "[data-plot-tab=signals]");
      await showGroup("heat-loss", ["P1.HeaterPowerKw", "P1.LossPowerKw"]);
      await showGroup("rate", ["P1.ModelRateCPerS"]);
      await showGroup("delayed-input", ["P1.DelayedCommandPct", "P1.AppliedPowerPct"]);

      await clickHotHeaterControl(inner, '[data-plot-tab="response"], [data-plot-tab="recorded-response"]');
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
