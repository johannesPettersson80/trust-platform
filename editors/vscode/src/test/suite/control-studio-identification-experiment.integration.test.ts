import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { connectRuntimeParameters } from "../../controlStudio/runtimeParameterConnection";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, setWebviewValue, sleep } from "./plant-rendered-harness";
import { chooseHotHeaterSignal, clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

function signal(sample: WorkflowResultSample, id: string): number {
  const value = sample.additional?.find(candidate => candidate.id === id);
  assert.ok(value && typeof value.value === "number" && Number.isFinite(value.value), `missing ${id} at scan ${sample.scan}`);
  assert.strictEqual(value.quality, "good");
  return value.value;
}

function nearest(samples: readonly WorkflowResultSample[], seconds: number): WorkflowResultSample {
  assert.ok(samples.length > 0);
  return samples.reduce((best, sample) => Math.abs(sample.elapsedSeconds - seconds) < Math.abs(best.elapsedSeconds - seconds) ? sample : best);
}

suite("Control Studio identification experiment", () => {
  test("applies bounded symbolic experiment parameters, records the native step, and reopens it", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_IDENTIFICATION_EXPERIMENT !== "1") this.skip();
    this.timeout(120_000);
    const uiOnly = process.env.TRUST_IDENTIFICATION_UI_ONLY === "1";
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-lesson-21");
    assert.strictEqual(loop.controller.instance, "P1");
    assert.strictEqual(loop.controller.type, "Main");
    assert.strictEqual(loop.topology, "open-loop");

    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const click = (selector: string) => clickHotHeaterControl(inner, selector);
      const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
      const capture = async (name: string) => {
        await sleep(250);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `identification-${name}.png`));
      };
      const step = "P1.SelectedStepPoints";
      const hold = "P1.HoldSeconds";
      const parameters = (await connectRuntimeParameters(loop)).client;
      const observed = async () => (await parameters.status()).values as Record<string, number>;

      await stage("Test");
      await click("[data-experiment-settings] > summary");
      assert.strictEqual(await inner("return String(d.querySelector('#test-duration').checkVisibility());"), "false");
      await pollValue("parameter editor ready", () => inner("return d.querySelector('[data-parameters]').dataset.ready;"), value => value === "true", 10_000);
      await click("[data-parameters-inspect]");
      await pollValue("visible inspected parameter fields", () => inner("return String(d.querySelectorAll('[data-parameter-symbol]').length);"), value => value === "2", 10_000);
      const baseline = await pollValue("native parameter inspection", async () => JSON.stringify(await observed()), value => value.length > 2, 10_000).then(JSON.parse) as Record<string, number>;
      assert.strictEqual(baseline[step], 10);
      assert.strictEqual(baseline[hold], 30);
      await capture("prepared-parameters");

      await setWebviewValue(inner, `[data-parameter-symbol="${step}"]`, "4");
      await click("[data-parameters-review]");
      await pollValue("invalid step rejected", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => /range|bound|minimum|5/.test(value), 10_000);
      assert.deepStrictEqual(await observed(), baseline, "invalid step review must not write symbolic parameters");
      assert.strictEqual(await inner("return String(d.querySelector('[data-parameters-apply]').disabled);"), "true");
      await capture("invalid-step");

      await setWebviewValue(inner, `[data-parameter-symbol="${step}"]`, "12");
      await setWebviewValue(inner, `[data-parameter-symbol="${hold}"]`, "25");
      await click("[data-parameters-review]");
      await pollValue("reviewed experiment parameters", () => inner("return d.querySelector('[data-parameters-review-table]').textContent;"), value => value.includes("12") && value.includes("25"), 10_000);
      await capture("reviewed-parameters");
      await click("[data-parameters-apply]");
      await pollValue("parameters observed after ST", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => /observed after ST/.test(value), 10_000);
      assert.deepStrictEqual(await observed(), { ...baseline, [step]: 12, [hold]: 25 });
      await capture("applied-parameters");
      await click("[data-experiment-settings] > summary");

      await stage("Monitor");
      await click('[data-plot-tab="signals"]');
      for (const symbol of ["P1.BaselineQualified", "P1.Active", "P1.ProposedCommandPct", "P1.PredictedFinalC"]) await chooseHotHeaterSignal(inner, symbol, true);
      await pollValue("real baseline qualified", () => inner("const row=[...d.querySelectorAll('[data-control-signals] .signal-row')].find(row=>row.children[2].textContent.endsWith('P1.BaselineQualified'));return row?.children[3].textContent??'';"), value => Number.parseFloat(value) === 1, 15_000);
      await capture("qualified-signals");
      for (const symbol of ["P1.BaselineQualified", "P1.Active", "P1.ProposedCommandPct", "P1.PredictedFinalC"]) await chooseHotHeaterSignal(inner, symbol, false);
      await click('[data-plot-tab="response"]');

      await stage("Test");
      await setWebviewValue(inner, "#test-duration", uiOnly ? "6.1" : "30.1");
      await click("[data-test-review]");
      await pollValue("reviewed test", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 10_000);
      await capture("test-review");
      assert.match(await inner("return d.querySelector('[data-test-timeout]').textContent;"), /5 s baseline/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-experiment-settings]').checkVisibility());"), "false",
        "the reviewed Test must lock its experiment parameters until Change test or termination");
      if (process.env.TRUST_IDENTIFICATION_REVIEW_ONLY === "1") return;
      await click("[data-test-change-review]");
      assert.strictEqual(await inner("return String(d.querySelector('[data-experiment-settings]').checkVisibility());"), "true");
      assert.deepStrictEqual(await observed(), { ...baseline, [step]: 12, [hold]: 25 }, "changing Test review does not change applied parameters");
      await capture("change-test");
      await click("[data-test-review]");
      await pollValue("reviewed test again", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 10_000);
      await click("[data-test-approve]");
      await sleep(400);
      await runtimeLifecycleService.requestIoState();
      assert.strictEqual(Number.parseFloat(await inner("return d.querySelector('[data-control-studio-mv]').textContent;")), 75,
        "recording starts at the baseline before project activation");
      await capture("baseline-recording");
      await pollValue("test running at87 percent", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, value => Number.parseFloat(value) === 87, 10_000);
      await stage("Monitor");
      await capture("running-87");
      await stage("Test");
      await pollValue("experiment complete", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 40_000);
      await stage("Monitor");
      await pollValue("normal completion restores75", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, value => Number.parseFloat(value) === 75, 10_000);
      await capture("complete");

      await stage("Validate");
      await click('[data-plot-tab="recorded-response"]');
      await capture("validate-before-save");
      await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',event=>{const m=event.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened')root.dataset.identificationResult=JSON.stringify(m.payload?.result);});return 'observe Open';");
      await click("[data-result-save]");
      await pollValue("saved experiment", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 10_000);
      await capture("saved");
      if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);") !== "true") await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
      await revealControlForCapture(inner, "[data-result-history]");
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(option=>option.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
      await setWebviewValue(inner, "[data-result-history]", revision);
      await click("[data-result-open]");
      const result = JSON.parse(await pollValue("opened saved experiment", () => inner("return d.querySelector('[data-control-studio]').dataset.identificationResult??'';"), value => value.length > 0, 10_000)) as ControlStudioWorkflowResult;
      fs.writeFileSync(path.join(captures, "identification-native-result.json"), JSON.stringify(result, null, 2));
      await capture("open");
      assert.strictEqual(result.recording.acquisition?.completeness, "complete");
      assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
      const active = result.recording.rawSamples.filter(sample => signal(sample, "active") === 1);
      const beforeStep = result.recording.rawSamples.filter(sample => sample.scan < active[0].scan);
      assert.ok(beforeStep.length >= 50 && active[0].elapsedSeconds - beforeStep[0].elapsedSeconds >= 5,
        "the saved native record must include the full five-second pre-step baseline");
      for (const sample of beforeStep) {
        assert.strictEqual(sample.appliedMv, 75);
        assert.ok(Math.abs(sample.pv! - 35) <= 0.05);
        assert.strictEqual(signal(sample, "quality"), 1);
      }
      if (uiOnly) assert.ok(active.length >= 10 && active.length <= 12, "the short UI scenario contains its declared active interval");
      else assert.strictEqual(active.length, 250, "25 second hold must retain250 active native scans");
      for (let index = 0; index < active.length; index += 1) {
        const sample = active[index];
        assert.strictEqual(sample.appliedMv, 87, "the selected12-point step must be applied; default85 must never run");
        if (index > 0) {
          assert.strictEqual(sample.scan, active[index - 1].scan + 1);
          assert.ok(Math.abs(sample.elapsedSeconds - active[0].elapsedSeconds - index * 0.1) < 1e-8);
        }
        assert.strictEqual(signal(sample, "proposal"), 87);
        assert.ok(Math.abs(signal(sample, "prediction") - 37.4) < 0.00001, "planning prediction uses the actual REAL representation");
      }
      const at6 = nearest(active, active[0].elapsedSeconds + 6).pv!;
      const at24 = nearest(active, active[0].elapsedSeconds + 24).pv!;
      if (!uiOnly) {
        assert.ok(Math.abs(at6 - (35 + 2.4 * (1 - Math.exp(-5 / 5)))) < 0.04);
        assert.ok(Math.abs(at24 - (35 + 2.4 * (1 - Math.exp(-23 / 5)))) < 0.02);
      }
      const terminal = result.recording.rawSamples.filter(sample => signal(sample, "active") === 0);
      assert.ok(terminal.some(sample => sample.appliedMv === 75), "normal completion must restore75 in the retained record");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
