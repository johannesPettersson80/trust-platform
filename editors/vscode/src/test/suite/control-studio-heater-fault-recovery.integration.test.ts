import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, setWebviewValue, sleep } from "./plant-rendered-harness";
import { clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

suite("Control Studio heater fault recovery", () => {
  test("retains aborted and forced-input records, observes zero output, and restarts from the declared state", async function () {
    if (process.env.TRUST_HEATER_FAULT_RECOVERY !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME ?? "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.type, "Control.PID");
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let forced = false;
    let inspectFailure: (() => Promise<string>) | undefined;
    try {
      const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, "src/Main.st")));
      const editor = await vscode.window.showTextDocument(doc);
      const before = "TrialKind : INT := 0;", offset = doc.getText().indexOf(before);
      assert.ok(offset >= 0);
      assert.strictEqual(await editor.edit(edit => edit.replace(new vscode.Range(doc.positionAt(offset), doc.positionAt(offset + before.length)), "TrialKind : INT := 1;")), true);
      assert.strictEqual(await doc.save(), true);
      fs.writeFileSync(path.join(captures, "executed-Main.st"), doc.getText());
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      inspectFailure = () => inner("return JSON.stringify({test:d.querySelector('[data-test-state]').textContent,values:d.querySelector('.values').textContent,recording:d.querySelector('[data-control-studio-recording-evidence]').textContent});");
      const click = (selector: string) => clickHotHeaterControl(inner, selector);
      const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
      const capture = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll"); await sleep(300);
        await inner("await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));return 'painted';");
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      };
      const output = async (value: number) => pollValue(`native applied output ${value}`, async () => {
        await runtimeLifecycleService.requestIoState(); return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, text => Number.parseFloat(text) === value, 10000);
      const startTrial = async (reviewed = false) => {
        await stage("Test");
        if (!reviewed) {
          await click("[data-test-review]");
          await pollValue("reviewed trial", () => inner("return String(d.querySelector('[data-test-approve]').hidden);"), text => text === "false", 10000);
        }
        await click("[data-test-approve]");
        await pollValue("running native trial", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /Test running/.test(text), 10000);
        await stage("Monitor"); await sleep(600);
      };
      await inner("d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.workflow-result'&&event.data.phase==='opened')d.querySelector('[data-control-studio]').dataset.faultResult=JSON.stringify(event.data.payload.result);});return 'observe actual opened results';");
      const saveRecord = async (name: string): Promise<ControlStudioWorkflowResult> => {
        await pollValue("retained native recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), text => /Recording retained/.test(text), 10000);
        await stage("Validate"); await click("[data-result-save]");
        await pollValue("saved result", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => /Saved revision/.test(text), 10000);
        if (await inner("return String(d.querySelector('[data-workflow-pane=Validate] details:has([data-result-history])').open);") !== "true") {
          await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
        }
        await revealControlForCapture(inner, "[data-result-history]");
        const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
        await inner("delete d.querySelector('[data-control-studio]').dataset.faultResult;return 'clear previous observation';");
        await setWebviewValue(inner, "[data-result-history]", revision); await click("[data-result-open]");
        const result = JSON.parse(await pollValue("opened result", () => inner("return d.querySelector('[data-control-studio]').dataset.faultResult??'';"), text => text.length > 0, 10000)) as ControlStudioWorkflowResult;
        fs.writeFileSync(path.join(captures, `${name}.json`), JSON.stringify(result, null, 2));
        assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
        assert.strictEqual(result.recording.acquisition?.completeness, "complete");
        assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
        assert.ok(result.recording.rawSamples.length > 2);
        assert.strictEqual(result.validation, undefined, "aborted data cannot manufacture validation success");
        await capture(name);
        return result;
      };
      await output(75); await startTrial(); await capture("01-running-before-abort");
      await click("[data-test-abort]");
      await pollValue("explicit abort restored", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /Aborted/.test(text) && /confirmed restored/.test(text), 15000);
      await output(0); await capture("02-observed-abort");
      const aborted = await saveRecord("03-saved-aborted-record");
      assert.strictEqual(aborted.experiment?.status, "aborted");
      assert.strictEqual(aborted.experiment?.restoration.status, "confirmed");

      assert.strictEqual((await runtimeLifecycleService.stopRuntime()).ok, true);
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      // Saved results stay historical until an explicit live operation. Reviewing
      // the next Test observes the new session without starting an experiment.
      await stage("Test"); await click("[data-test-review]");
      await pollValue("fresh reviewed hot state", () => inner("return String(d.querySelector('[data-test-approve]').hidden);"), text => text === "false", 10000);
      await stage("Monitor");
      // Observe the new native session rather than editing a saved controller state.
      await pollValue("fresh runtime observation", async () => {
        await runtimeLifecycleService.requestIoState(); return inner("return d.querySelector('[data-control-studio-freshness]').textContent;");
      }, text => text === "Live", 10000);
      await output(75);
      await click('[data-chart-action="table"]');
      const freshRows: string[][] = JSON.parse(await inner("return JSON.stringify([...d.querySelectorAll('[data-chart-table] tbody tr')].map(row=>[...row.cells].map(cell=>cell.textContent.trim())));"));
      assert.ok(freshRows.length > 0, "the restarted live chart must contain current samples");
      assert.ok(freshRows.every(row => Number.parseFloat(row[1]) === 35 && Number.parseFloat(row[2]) === 35 && Number.parseFloat(row[3]) === 75),
        `The restarted live response must not join the earlier aborted session: ${JSON.stringify(freshRows)}`);
      await click('[data-chart-action="table"]');
      await capture("04-fresh-hot-start");
      assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify({lastTest:d.querySelector('[data-control-studio-recording-evidence]').textContent.startsWith('Last test: Aborted.'),testStatus:d.querySelector('[data-control-studio-mode][aria-label^=\"Test,\"]').dataset.status});")),
        { lastTest: true, testStatus: "ready" }, "the fresh live runtime must distinguish its newly reviewed plan from the prior aborted record");
      await startTrial(true);
      await vscode.commands.executeCommand("trust-lsp.debug.io.force", { address: "%IL0", value: "81" }); forced = true;
      await output(0); // The application guard stops heating before operator action.
      await click("[data-test-abort]");
      const terminal = await pollValue("operator abort after the input fault", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /Aborted/.test(text) && /confirmed restored/.test(text), 15000);
      fs.writeFileSync(path.join(captures, "forced-input-terminal.txt"), terminal);
      await output(0); await capture("05-forced-input-stop");
      await stage("Test"); await click("[data-test-review]");
      await pollValue("forced primary input blocks a new Test", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /forcing|forced/i.test(text), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-test-approve]').hidden);"), "true");
      await capture("05a-forced-input-blocks-new-test"); await stage("Monitor");
      await vscode.commands.executeCommand("trust-lsp.debug.io.release", { address: "%IL0" }); forced = false;
      await sleep(600); await output(0);
      await click('[data-plot-tab="signals"]');
      const signals = await inner("return d.querySelector('[data-control-signals]').textContent;");
      fs.writeFileSync(path.join(captures, "released-input-signals.txt"), signals);
      assert.strictEqual(await inner("const row=[...d.querySelectorAll('[data-control-signals] .signal-row')].find(row=>row.querySelector('.signal-secondary')?.textContent.endsWith('P1.SafetyAbort'));return row?.children[3].textContent.trim();"), "1", "a fresh valid input does not clear the ST fault latch");
      await capture("06-released-input-stays-stopped");
      await click('[data-plot-tab="response"]');
      const faulted = await saveRecord("07-saved-forced-input-record");
      assert.notStrictEqual(faulted.resultRevision, aborted.resultRevision);
      assert.strictEqual(faulted.experiment?.status, "aborted");
      assert.ok(faulted.recording.rawSamples.some(sample => sample.pv === 81 && sample.quality === "forced"), "retain the forced primary input and its actual quality");
      await revealControlForCapture(inner, "[data-result-history]");
      const revisions: string[] = JSON.parse(await inner("return JSON.stringify([...d.querySelector('[data-result-history]').options].map(o=>o.value));"));
      assert.ok(revisions.includes(String(aborted.resultRevision)), "the earlier aborted recording survives restart and the new trial");
    } catch (error) {
      if (inspectFailure) {
        const state = await inspectFailure();
        fs.writeFileSync(path.join(captures, "failure-state.json"), state);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "failure-state.png"));
        throw new Error(`${error instanceof Error ? error.message : error}; rendered state: ${state}`);
      }
      throw error;
    } finally {
      if (forced) await vscode.commands.executeCommand("trust-lsp.debug.io.release", { address: "%IL0" });
      await runtimeLifecycleService.stopRuntime();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      connection.close();
    }
  });
});
