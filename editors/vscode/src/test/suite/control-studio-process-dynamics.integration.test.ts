import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import {
  captureRenderedWorkbenchCdp,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  setWebviewValue,
  sleep,
} from "./plant-rendered-harness";
import { clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

function channel(sample: WorkflowResultSample, id: string): number {
  const found = sample.additional?.find(value => value.id === id);
  assert.ok(found && typeof found.value === "number" && Number.isFinite(found.value), `${id} at scan ${sample.scan}`);
  assert.strictEqual(found.quality, "good");
  return found.value;
}

function nearest(samples: readonly WorkflowResultSample[], seconds: number): WorkflowResultSample {
  assert.ok(samples.length > 0, "the held-input recording must contain samples");
  return samples.reduce((best, sample) =>
    Math.abs(sample.elapsedSeconds - seconds) < Math.abs(best.elapsedSeconds - seconds) ? sample : best);
}

suite("Control Studio process dynamics", () => {
  for (const uiOnly of [false, true]) test(uiOnly
    ? "shows retained open-loop samples in Validate and current values again in Monitor"
    : "runs, saves, and reopens the published thirty-second held-input observation", async function () {
    if (process.env.TRUST_PROCESS_DYNAMICS !== "1") this.skip();
    if (uiOnly !== (process.env.TRUST_PROCESS_DYNAMICS_UI_ONLY === "1")) this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-lesson-13");
    assert.strictEqual(loop.topology, "open-loop");
    assert.strictEqual(loop.controller.type, "Main", "this is the project-owned open-loop process observation");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    assert.strictEqual(loop.signals.manipulatedVariable.address, "%QL0");
    assert.strictEqual(loop.experiment.durationSeconds, 30.1);

    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1,
        source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId },
        loopDefinition: loop,
      });
      opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const click = (selector: string) => clickHotHeaterControl(inner, selector);
      const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
      const capture = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll");
        await sleep(300);
        await inner("await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));return 'painted';");
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      };

      await pollValue("prepared hot process", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return JSON.stringify({pv:d.querySelector('[data-control-studio-pv]').textContent,mv:d.querySelector('[data-control-studio-mv]').textContent});");
      }, value => {
        const current = JSON.parse(value) as { pv: string; mv: string };
        return Number.parseFloat(current.pv) === 35 && Number.parseFloat(current.mv) === 75;
      }, 15_000);
      await stage("Monitor");
      await click('[data-plot-tab="response"]');
      await capture("01-prepared-response");
      if (uiOnly) {
        assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-sp]'));"), "null");
        assert.strictEqual(await inner("return String(d.querySelector('[data-control-studio-controller-mode]'));"), "null");
        assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-label');"), /Measured output.*Applied input/);
        await click('[data-plot-tab="signals"]');
        const labels = await inner("return d.querySelector('[data-control-signals]').textContent;");
        assert.match(labels, /Measured output/); assert.match(labels, /Applied input/);
        await capture("01a-live-signals");
        await click('[data-plot-tab="response"]');
        // Click the first visible legend entry in the real canvas. Hidden
        // diagnostics must not become selected through omitted event keys.
        const point = JSON.parse(await inner("const r=d.querySelector('[data-control-studio-chart] canvas').getBoundingClientRect();const frame=f.getBoundingClientRect();return JSON.stringify({x:frame.x+r.right-180,y:frame.y+r.top+20});"));
        await connection.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y }, session);
        await connection.send("Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button: "left", clickCount: 1 }, session);
        await connection.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button: "left", clickCount: 1 }, session);
        await pollValue("legend toggles measured output", () => inner("return d.querySelector('[data-control-studio]').dataset.controlChartSession;"), value => !JSON.parse(value).selectedSignals.includes("pv"), 5000);
        const selected: string[] = JSON.parse(await inner("return d.querySelector('[data-control-studio]').dataset.controlChartSession;")).selectedSignals;
        assert.ok(!selected.includes("experiment-authority") && !selected.includes("safety-abort"),
          "toggling a plotted legend item must not enable unselected diagnostics");
        await click('[data-plot-tab="signals"]');
        await click('[data-control-signals] .signal-row:first-child input');
        await click('[data-plot-tab="response"]');
      }
      await stage("Test");
      if (uiOnly) await setWebviewValue(inner, "#test-duration", "1.1");
      await click("[data-test-review]");
      await pollValue("reviewed held input", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 10_000);
      await capture("02-reviewed-held-input");
      await click("[data-test-approve]");
      await pollValue("held input running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Test running/.test(value), 10_000);
      await stage("Monitor");
      await pollValue("observed held input", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, value => Number.parseFloat(value) === 85, 5_000);
      await capture("03-held-input-running");
      await stage("Test");
      await pollValue("thirty-second held input", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 45_000);
      await stage("Monitor");
      await pollValue("automatic Test recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => /Recording retained/.test(value), 10_000);
      await pollValue("restored output", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, value => Number.parseFloat(value) === 0, 5_000);
      await capture("04-complete-held-input");

      await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',event=>{const m=event.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened'&&m.loopId===root.dataset.loopId)root.dataset.processDynamicsResult=JSON.stringify(m.payload?.result);});return 'observe actual Open';");
      await stage("Validate");
      if (uiOnly) {
        const tabs = await inner("return d.querySelector('[data-control-plot-tabs]').textContent;");
        assert.match(tabs, /Recorded Response/i, "Validate must show the retained Test recording before saving it");
        const recordingTabs = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-control-plot-tabs] [data-plot-tab]')].map(button=>({id:button.dataset.plotTab,selected:button.getAttribute('aria-selected')==='true',disabled:button.disabled})));")) as Array<{ id: string; selected: boolean; disabled: boolean }>;
        assert.deepStrictEqual(recordingTabs.map(tab => tab.id), ["recorded-response", "signals"],
          "recording-only Validate must expose the retained response and Signals views without a candidate detour");
        assert.strictEqual(recordingTabs.find(tab => tab.id === "signals")?.disabled, false,
          "recording-only Validate Signals must be selectable");
        await click('[data-plot-tab="signals"]');
        await pollValue("historical Validate signals", () => inner("const tab=d.querySelector('[data-plot-tab=signals]');const view=d.querySelector('[data-plot-view=signals]');return JSON.stringify({selected:tab?.getAttribute('aria-selected'),hidden:view?.hidden,text:d.querySelector('[data-control-signals]')?.textContent??'',freshness:d.querySelector('[data-control-studio-freshness]')?.textContent??''});"), value => {
          const state = JSON.parse(value) as { selected: string; hidden: boolean; text: string; freshness: string };
          return state.selected === "true" && state.hidden === false && /Measured output|Applied input/.test(state.text) && /Historical/.test(state.freshness);
        }, 5000);
        await click('[data-plot-tab="recorded-response"]');
        await pollValue("return to retained response", () => inner("const tab=d.querySelector('[data-plot-tab=recorded-response]');const view=d.querySelector('[data-plot-view=response]');return JSON.stringify({selected:tab?.getAttribute('aria-selected'),hidden:view?.hidden,freshness:d.querySelector('[data-control-studio-freshness]')?.textContent??'',historical:d.querySelector('[data-control-studio]')?.dataset.controlStudioHistoricalResult??''});"), value => {
          const state = JSON.parse(value) as { selected: string; hidden: boolean; freshness: string; historical: string };
          return state.selected === "true" && state.hidden === false && /Historical/.test(state.freshness);
        }, 5000);
        await click('[data-chart-action="table"]');
        const before = await inner("return d.querySelector('[data-chart-table] tbody').textContent;");
        await sleep(600);
        assert.strictEqual(await inner("return d.querySelector('[data-chart-table] tbody').textContent;"), before,
          "live cooling must not extend or replace the retained recording in Validate");
        await click('[data-chart-action="table"]');
        await capture("04a-retained-before-save");
        await stage("Monitor");
        await pollValue("return to live process", () => inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), value => value === "Live", 5_000);
        await capture("04b-live-after-record-review");
        await stage("Validate");
      }
      await click("[data-result-save]");
      await pollValue("saved process observation", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 10_000);
      await capture("05-saved-process-observation");
      if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);") !== "true") {
        await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
      }
      await revealControlForCapture(inner, "[data-result-history]");
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(option=>option.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
      await setWebviewValue(inner, "[data-result-history]", revision);
      await click("[data-result-open]");
      const encoded = await pollValue("opened saved process observation", () => inner("return d.querySelector('[data-control-studio]').dataset.processDynamicsResult??'';"), value => value.length > 0, 10_000);
      const result = JSON.parse(encoded) as ControlStudioWorkflowResult;
      fs.writeFileSync(path.join(captures, "process-dynamics-native-result.json"), JSON.stringify(result, null, 2));
      assert.strictEqual(result.loop.topology, "open-loop");
      assert.strictEqual(result.controller.instance, "P1");
      assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
      assert.strictEqual(result.recording.acquisition?.completeness, "complete");
      assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);

      if (uiOnly) {
        assert.ok(result.recording.rawSamples.length >= 10, "the short display scenario must retain actual native samples");
        await capture("06-reopened-process-observation");
        await stage("Monitor"); await click('[data-plot-tab="signals"]');
        const labels = await inner("return d.querySelector('[data-control-signals]').textContent;");
        assert.match(labels, /Measured output/); assert.match(labels, /Applied input/);
        assert.match(await inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), /Historical/);
        await capture("06a-recorded-signals");
        return;
      }

      const active = result.recording.rawSamples.filter(sample => channel(sample, "experiment-authority") === 1);
      assert.ok(active.length > 0, "the native record must include project activation");
      const onset = active[0].elapsedSeconds;
      const held = active.filter(sample => sample.elapsedSeconds - onset < 30 - 1e-9);
      assert.strictEqual(held.length, 300, "the project must retain all300 native100ms scans of its30s hold");
      assert.ok(held.every(sample => sample.appliedMv === 85), "the project must hold the declared 85 percent input");
      assert.ok(active.some(sample => Math.abs(sample.elapsedSeconds - onset - 30) < 1e-9 && sample.appliedMv === 0),
        "the program ends its30s hold before the host restores activation at30.1s");
      assert.ok(active.slice(0, 9).every(sample => Math.abs(sample.pv! - 35) < 1e-8), "the declared one-second process delay must be visible");
      assert.ok(Math.abs(nearest(active, 6).pv! - 36.264) < 0.04, "the six-second native process checkpoint must be retained");
      assert.ok(Math.abs(nearest(active, 26).pv! - 36.986) < 0.01, "the twenty-six-second native process checkpoint must be retained");
      const restored = result.recording.rawSamples.filter(sample => channel(sample, "experiment-authority") === 0);
      assert.ok(restored.some(sample => sample.appliedMv === 0), "the saved recording must include terminal zero output");
      await capture("06-reopened-process-observation");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
