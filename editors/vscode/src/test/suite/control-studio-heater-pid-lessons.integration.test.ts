import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio actual PID heater lesson", () => {
  const layoutOnly = process.env.TRUST_HEATER_LAYOUT_ONLY === "1";
  test(layoutOnly ? "keeps the Test response, compact values and controls in separate visible areas" :
    "records the published P or PI warming and load experiment through ordinary I/O", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_HEATER_PID_LESSON !== "1") this.skip();
    this.timeout(200000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.type, "Control.PID");
    assert.strictEqual(loop.signals.processVariable.address, "%IL0");
    const mode = process.env.TRUST_HEATER_FAMILY ?? "P";
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(started.ok, true, JSON.stringify(started));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const click = async (selector: string) => {
        const result = await inner(`const b=d.querySelector(${JSON.stringify(selector)});b?.scrollIntoView({block:'nearest',inline:'nearest'});const r=b?.getBoundingClientRect();if(!b||b.disabled||!b.checkVisibility()||!r||!r.width||!r.height||r.top<0||r.bottom>d.defaultView.innerHeight)return 'unavailable';b.click();return 'clicked';`);
        assert.strictEqual(result, "clicked", `${selector} must be visible and enabled`);
      };
      const stage = async (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
      const capture = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll");
        await new Promise(resolve => setTimeout(resolve, 300));
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      };
      await pollValue("fresh heater measurement", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-pv]')?.textContent??'';");
      }, value => /20/.test(value), 15000);
      await capture("01-idle");
      await stage("Test");
      assert.strictEqual(await inner("return d.querySelector('#test-kind').value;"), "observation");
      assert.strictEqual(await inner("return d.querySelector('#test-amplitude').value;"), "0");
      await inner("const input=d.querySelector('#test-duration');input.value='120.1';input.dispatchEvent(new Event('input',{bubbles:true}));return input.value;");
      await click("[data-test-review]");
      await pollValue("review", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 15000);
      assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-workflow-pane=\"Test\"] button.primary')].filter(b=>!b.disabled&&b.getBoundingClientRect().width>0).map(b=>b.textContent.trim()));")), ["Confirm and start"]);
      await click("[data-test-change-review]");
      assert.strictEqual(await inner("return String(d.querySelector('#test-duration').disabled);"), "false");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio]').dataset.experimentPlanId??'';"), "");
      await click("[data-test-review]");
      await pollValue("changed test reviewed again", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 15000);
      await capture("02-review");
      if (layoutOnly) {
        const boxes = JSON.parse(await inner("const box=s=>{const r=d.querySelector(s).getBoundingClientRect();return {top:r.top,bottom:r.bottom,height:r.height};};return JSON.stringify({plot:box('.plot-column'),values:box('.values'),workspace:box('.workspace'),approve:box('[data-test-approve]'),viewport:d.defaultView.innerHeight});"));
        assert.ok(boxes.plot.bottom <= boxes.values.top + 1,
          `the complete response stays above the value strip: ${JSON.stringify(boxes)}`);
        assert.ok(boxes.values.bottom <= boxes.workspace.top + 1, "values do not cover the Test controls");
        assert.ok(boxes.values.height <= 60, `live values remain compact, got ${boxes.values.height}px`);
        assert.ok(boxes.approve.top >= boxes.workspace.top && boxes.approve.bottom <= boxes.viewport,
          "the reviewed Test action is visible without scrolling past the response");
        await stage("Validate");
        await capture("03-validation-controls");
        assert.strictEqual(await inner("return String(d.querySelector('#run-validation-current').checkVisibility());"), "false",
          "optional imported-run fields must not fill the saved-recording view");
        await click("[data-manual-run-validation] > summary");
        assert.strictEqual(await inner("return String(d.querySelector('#run-validation-current').checkVisibility());"), "true");
        await capture("04-imported-run-controls");
        return;
      }
      await click("[data-test-approve]");
      await pollValue("experiment running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Test running/.test(value), 10000);
      await pollValue("visible warming response", () => inner("return d.querySelector('[data-control-studio-pv]').textContent;"), value => Number.parseFloat(value) > 20.5, 15000);
      await capture("03-running");
      await pollValue("published 120-second experiment", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 145000);
      await capture("04-completed");
      await stage("Monitor");
      await pollValue("complete native recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => /Recording retained/.test(value), 15000);
      await capture("05-recorded-response");
      await stage("Validate");
      await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result')d.defaultView.__heaterLesson=e.data.payload?.result;});return 'observe saved result';");
      await click("[data-result-save]");
      await pollValue("saved result", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 15000);
      await inner("d.querySelector('[data-result-history]').closest('details').querySelector('summary').click();return 'opened saved results';");
      await inner("const h=d.querySelector('[data-result-history]');h.value=[...h.options].filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;h.dispatchEvent(new Event('change'));return h.value;");
      await click("[data-result-open]");
      const result = JSON.parse(await pollValue("reopened native result", () => inner("return JSON.stringify(d.defaultView.__heaterLesson??null);"), value => value !== "null", 15000));
      fs.writeFileSync(path.join(captures, "heater-result.json"), JSON.stringify(result, null, 2));
      const authority = loop.signals.additional.find((signal: any) => signal.address === loop.experiment.activation.address)?.id;
      const samples = result.recording.rawSamples.filter((sample: any) => sample.additional?.some((signal: any) => signal.id === authority && signal.value === 1));
      assert.strictEqual(result.recording.acquisition.completeness, "complete");
      assert.ok(samples.length >= 1190, "retain the actual 120-second experiment");
      const inWindow = (start: number, end: number) => samples.filter((sample: any) => sample.elapsedSeconds - samples[0].elapsedSeconds >= start && sample.elapsedSeconds - samples[0].elapsedSeconds < end);
      for (const [start, end, expected] of [[50, 60, mode === "P" ? 24.285714 : 35], [110, 120, mode === "P" ? 22.142857 : 35]]) {
        const window = inWindow(start, end);
        assert.ok(window.length >= 99, `retain the ${start}–${end}s assessment window at the 0.1s task interval`);
        assert.ok(window.every((sample: any) => Math.abs(sample.pv - expected) <= (mode === "P" ? 0.05 : 0.2)), `actual ${mode} temperatures satisfy the published comparison`);
      }
      await pollValue("reopened measured response", () => inner("return String(!d.querySelector('[data-plot-view=\"response\"]').hidden);"), value => value === "true", 5000);
      await capture("06-reopened-result");
      await stage("Test");
      await click("[data-test-review]");
      await pollValue("next experiment can be reviewed", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Simulation test ready/.test(value), 15000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-test-approve]').disabled);"), "false");
      await capture("07-next-review");
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });
});
