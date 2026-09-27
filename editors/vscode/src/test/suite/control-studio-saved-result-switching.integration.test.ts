import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { candidateFromControlPid } from "../../controlStudio/controllerCandidate";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Control Studio saved-result switching", () => {
  test("replaces candidate plots with the opened recording and restores candidate review on return", async function () {
    if (process.env.TRUST_CONTROL_RESULT_SWITCHING !== "1") this.skip();
    this.timeout(45000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const saved = JSON.parse(fs.readFileSync(requiredEnvironment("TRUST_CONTROL_RESULT_FIXTURE"), "utf8"));
    // Deliberate renderer fixture: real retained recording plus a synthetic,
    // unapplied candidate. This proves result switching, not tuning or ST verification.
    const candidate = candidateFromControlPid({ proportionalGain: 2, integralGain: 0.5, derivativeGain: 0,
      proportionalSetpointWeight: 1, derivativeSetpointWeight: 0, bias: 0, outputMinimum: 0, outputMaximum: 100,
      rateMinimum: -10000, rateMaximum: 10000, trackingTime: 1, antiWindup: "BackCalculation" }, "Manual");
    const candidateResult = { ...saved, resultRevision: 10, candidate, validation: { requirements: [] } };
    const recordingResult = { ...saved, resultRevision: 11, candidate: undefined, model: undefined, validation: undefined };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("Validate handlers ready", () => inner("d.querySelector('[data-control-studio-mode][aria-label^=\"Validate,\"]').click();return String(d.querySelector('[data-workflow-pane=Validate]').hidden);"), value => value === "false", 5000);
      await sleep(250);
      const capture = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll");
        await sleep(350);
        await captureRenderedWorkbenchCdp(connection,
          path.join(path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT")), `${name}.png`));
      };
      const restore = async (result: unknown) => {
        await inner(`const root=d.querySelector('[data-control-studio]');d.defaultView.dispatchEvent(new d.defaultView.MessageEvent('message',{data:{
          kind:'control-studio.workflow-result',phase:'opened',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,
          loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,payload:{result:${JSON.stringify(result)}}}}));
          d.querySelector('[data-control-studio-mode][aria-label^="Validate,"]').click();return 'fixture restored';`);
        await pollValue("historical result shown", () => inner("return String(d.querySelector('[data-control-studio]').dataset.controlStudioHistoricalResult);"), value => value === "true", 5000);
      };
      const tabs = async () => JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-control-plot-tabs] [data-plot-tab]')].map(b=>({id:b.dataset.plotTab,selected:b.getAttribute('aria-selected')==='true',disabled:b.disabled})));")) as Array<{ id: string; selected: boolean; disabled: boolean }>;
      await restore(candidateResult);
      assert.ok((await tabs()).some(tab => tab.id === "requirements" && !tab.disabled));
      assert.strictEqual(await inner("return d.querySelector('[data-application-validation]').textContent;"), "Not validated",
        "an empty saved requirement list cannot claim required validation passed");
      assert.match(await inner("return d.querySelector('[data-engineering-plot-summary]').textContent;"), /No comparison is stored/,
        "a restored candidate without saved prediction must explain the empty chart");
      await sleep(250);
      const explanationPixels = Number(await inner("const host=d.querySelector('[data-engineering-requirements]');if(!host.checkVisibility())return '0';let count=0;for(const canvas of host.querySelectorAll('canvas')){const width=Math.min(360,canvas.width),height=Math.min(80,canvas.height);const data=canvas.getContext('2d').getImageData(Math.floor((canvas.width-width)/2),Math.floor((canvas.height-height)/2),width,height).data;for(let i=3;i<data.length;i+=4)if(data[i]>128)count++;}return String(count);"));
      assert.ok(explanationPixels > 40,
        "the empty comparison explanation must be painted inside the actual canvas, beyond its faint grid lines");
      await capture("01-candidate-result");
      await restore(recordingResult);
      await capture("02-recording-result");
      assert.deepStrictEqual((await tabs()).map(tab => tab.id), ["recorded-response", "signals"],
        "opening recording-only evidence must remove the previously opened candidate's plots");
      assert.strictEqual((await tabs())[0].selected, true);
      assert.strictEqual(await inner("return String(d.querySelector('[data-validation-candidate-only]').hidden);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-plot-view=response]').checkVisibility());"), "true");
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Validate,\"]').dataset.status;"), "ready");
      const click = (selector: string) => clickHotHeaterControl(inner, selector);
      await click('[data-control-studio-mode][aria-label^="Monitor,"]');
      await click('[data-plot-tab="signals"]');
      assert.strictEqual(await inner("const rows=[...d.querySelectorAll('[data-control-signals] .signal-row')];if(!rows.some(row=>row.querySelector('span').textContent==='Error'))return 'missing';for(const row of rows){const input=row.querySelector('input');const selected=row.querySelector('span').textContent==='Error';if(input.checked!==selected)input.click();}return String(d.querySelectorAll('[data-control-signals] input:checked').length);"), "1",
        "select the saved Error row by its displayed identity without requiring a live symbol binding");
      await click('[data-plot-tab="response"]');
      await capture("02b-error-axis");
      await restore(candidateResult);
      await capture("03-candidate-return");
      assert.ok((await tabs()).some(tab => tab.id === "requirements" && !tab.disabled));
      assert.strictEqual(await inner("return String(d.querySelector('[data-validation-candidate-only]').hidden);"), "false");
      if (process.env.TRUST_CONSTANT_DIFFERENCE_FIXTURE === "1") {
        await restore(recordingResult);
        await click('[data-control-studio-mode][aria-label^="Monitor,"]'); await click('[data-plot-tab="signals"]');
        assert.strictEqual(await inner("const rows=[...d.querySelectorAll('[data-control-signals] .signal-row')];if(!rows.some(row=>row.querySelector('span').textContent==='Difference'))return 'missing';for(const row of rows){const input=row.querySelector('input');const select=row.querySelector('span').textContent==='Difference';if(input.checked!==select)input.click();}return String(d.querySelectorAll('[data-control-signals] input:checked').length);"), "1");
        await click('[data-plot-tab="response"]'); await capture("04-constant-difference");
        const trace = JSON.parse(await inner("const host=d.querySelector('[data-control-studio-chart]');let low=Infinity,high=-Infinity,count=0;for(const canvas of host.querySelectorAll('canvas')){const pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;for(let y=64;y<canvas.height-44;y++)for(let x=58;x<canvas.width-50;x++){const i=(y*canvas.width+x)*4;if(pixels[i]>150&&pixels[i+1]<190&&pixels[i+2]<90&&pixels[i+3]>128){low=Math.min(low,y);high=Math.max(high,y);count++;}}}return JSON.stringify({count,span:high-low});"));
        assert.ok(trace.count > 40 && trace.span <= 4,
          `constant native reference difference must paint a level trace, not amplified roundoff: ${JSON.stringify(trace)}`);
        await click('[data-chart-action="table"]'); await capture("05-constant-difference-samples");
      }
    } finally {
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
