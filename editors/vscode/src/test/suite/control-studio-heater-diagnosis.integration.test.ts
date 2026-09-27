import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, setWebviewValue, sleep } from "./plant-rendered-harness";
import { chooseHotHeaterSignal, clickHotHeaterControl } from "./control-studio-hot-heater-helpers";
import { revealControlForCapture } from "./control-studio-capture-visibility";

suite("Control Studio heater diagnosis", () => {
  test("records the published biased load repeat with separate native sensor and actuator evidence", async function () {
    if (process.env.TRUST_HEATER_DIAGNOSIS !== "1") this.skip();
    this.timeout(150000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-loop-diagnosis");
    assert.strictEqual(loop.controller.instance, "P1.Controller");
    assert.strictEqual(loop.controller.type, "Control.PID");
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      for (const [file, before, after] of [["src/Main.st", "TrialKind : INT := 0;", "TrialKind : INT := 1;"],
        ["simulation.toml", "value = 0.0", "value = 0.3"]]) {
        const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, file)));
        const editor = await vscode.window.showTextDocument(doc);
        const offset = doc.getText().indexOf(before);
        assert.ok(offset >= 0, `published setting ${before}`);
        const range = new vscode.Range(doc.positionAt(offset), doc.positionAt(offset + before.length));
        assert.strictEqual(await editor.edit(edit => edit.replace(range, after)), true);
        assert.strictEqual(await doc.save(), true);
        editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
        fs.writeFileSync(path.join(captures, file.endsWith(".st") ? "executed-Main.st" : "executed-simulation.toml"), doc.getText());
      }
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "01-native-bias-setting.png"));
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
        await vscode.commands.executeCommand("notifications.clearAll"); await sleep(350);
        await inner("await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));return 'painted';");
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      };
      await pollValue("hot measured sensor with native bias", async () => {
        await runtimeLifecycleService.requestIoState();
        return inner("return d.querySelector('[data-control-studio-pv]').textContent;");
      }, text => Math.abs(Number.parseFloat(text) - 35.3) < 1e-8, 15000);
      await stage("Monitor"); await click('[data-plot-tab="signals"]');
      for (const id of ["reference", "request", "paired-request", "thermal-load"]) {
        await chooseHotHeaterSignal(inner, loop.signals.additional.find((s: { id: string }) => s.id === id).symbol, true);
      }
      await capture("02-diagnostic-signals");
      await click('[data-plot-tab="response"]'); await stage("Test"); await click("[data-test-review]");
      await pollValue("bounded diagnostic review", () => inner("return String(d.querySelector('[data-test-approve]').hidden);"), text => text === "false", 10000);
      await capture("03-reviewed-repeat"); await click("[data-test-approve]");
      await pollValue("diagnostic trial running", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /Test running/.test(text), 10000);
      await stage("Monitor"); await sleep(1200); await capture("04-biased-load-response"); await stage("Test");
      await pollValue("native sixty-second diagnostic repeat", () => inner("return d.querySelector('[data-test-state]').textContent;"), text => /Completed/i.test(text), 75000);
      await stage("Monitor");
      await pollValue("observed actuator stop", async () => {
        await runtimeLifecycleService.requestIoState(); return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, text => Number.parseFloat(text) === 0, 5000);
      await pollValue("complete diagnostic recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), text => /Recording retained/.test(text), 10000);
      await capture("05-complete-diagnosis");
      await inner("d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.workflow-result'&&event.data.phase==='opened')d.querySelector('[data-control-studio]').dataset.diagnosisResult=JSON.stringify(event.data.payload.result);});return 'observe actual Open';");
      await stage("Validate"); await click("[data-result-save]");
      await pollValue("saved diagnostic evidence", () => inner("return d.querySelector('[data-result-state]').textContent;"), text => /Saved revision/.test(text), 10000);
      await capture("06-saved-diagnosis");
      await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
      await revealControlForCapture(inner, "[data-result-history]");
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
      await setWebviewValue(inner, "[data-result-history]", revision); await click("[data-result-open]");
      const result = JSON.parse(await pollValue("opened diagnostic record", () => inner("return d.querySelector('[data-control-studio]').dataset.diagnosisResult??'';"), text => text.length > 0, 10000)) as ControlStudioWorkflowResult;
      fs.writeFileSync(path.join(captures, "native-result.json"), JSON.stringify(result, null, 2));
      assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
      assert.strictEqual(result.recording.acquisition?.completeness, "complete");
      assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
      const active = result.recording.rawSamples.filter(sample => signal(sample, "experiment-authority") === 1);
      assert.ok(active.length >= 600);
      assert.ok(Math.abs(signal(active[0], "request") - 74.985) < 1e-8, "actual PID first biased load command");
      for (let i = 0; i < 600; i++) {
        const sample = active[i];
        assert.strictEqual(sample.sp, 35); assert.strictEqual(signal(sample, "thermal-load"), 30);
        assert.ok(Math.abs(sample.pv! - signal(sample, "reference") - 0.3) < 1e-8);
        assert.ok(Math.abs(signal(sample, "difference") - 0.3) < 1e-8);
        assert.ok(Math.abs(signal(sample, "unavailable")) < 1e-8, "I/O timing alone is not unavailable heater capacity");
        if (i > 0) {
          assert.strictEqual(sample.scan, active[i - 1].scan + 1);
          assert.ok(Math.abs(signal(sample, "paired-request") - signal(active[i - 1], "request")) < 1e-8);
        }
      }
      assert.strictEqual(result.validation, undefined, "a diagnostic recording is not a nominal validation pass");
      await stage("Monitor"); await click('[data-plot-tab="signals"]');
      await inner("for(const row of d.querySelectorAll('[data-control-signals] .signal-row')){const input=row.querySelector('input');const select=row.textContent.includes('P1.ReferenceDifferenceC');if(input.checked!==select)input.click();}return 'select recorded reference difference';");
      await click('[data-plot-tab="response"]'); await capture("07-recorded-reference-difference");
      await click('[data-chart-action="table"]'); await capture("08-reference-difference-samples");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});

function signal(sample: WorkflowResultSample, id: string): number {
  const found = sample.additional?.find(value => value.id === id);
  assert.ok(found && typeof found.value === "number" && Number.isFinite(found.value), `${id} at scan ${sample.scan}`);
  assert.strictEqual(found.quality, "good");
  return found.value;
}
