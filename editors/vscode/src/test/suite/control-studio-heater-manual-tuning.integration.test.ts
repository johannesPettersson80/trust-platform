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

const kp = "P1.Parameters.ProportionalGain";
const ki = "P1.Parameters.IntegralGain";

suite("Control Studio heater manual tuning", () => {
  test("reviews one actual PID gain, rejects invalid values, records the trial and restores the prior gains", async function () {
    if (process.env.TRUST_HEATER_MANUAL_TUNING !== "1") this.skip();
    this.timeout(150000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const trial = Number(process.env.TRUST_HEATER_RESPONSE_TRIAL ?? "0");
    assert.ok([0, 1].includes(trial));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      if (trial === 1) {
        const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, "src/Main.st")));
        const editor = await vscode.window.showTextDocument(doc);
        const before = "TrialKind : INT := 0;";
        const offset = doc.getText().indexOf(before);
        assert.ok(offset >= 0);
        assert.strictEqual(await editor.edit(edit => edit.replace(new vscode.Range(doc.positionAt(offset), doc.positionAt(offset + before.length)), "TrialKind : INT := 1;")), true);
        assert.strictEqual(await doc.save(), true);
      }
      fs.copyFileSync(path.join(project, "src/Main.st"), path.join(captures, "executed-Main.st"));
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
        if (process.env.TRUST_PARAMETER_EDIT_RETURN === "1" && name !== "03a-return-to-edit") return;
        await vscode.commands.executeCommand("notifications.clearAll"); await sleep(350);
        await inner("await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));return 'painted';");
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
      };
      await stage("Tune");
      await pollValue("parameter handlers ready", () => inner("return d.querySelector('[data-parameters]').dataset.ready;"), value => value === "true", 10000);
      await inner("d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.engineering.result'&&event.data.stage==='parameters')d.querySelector('[data-parameters]').dataset.nativeReply=JSON.stringify(event.data.payload);});return 'observe native parameter response';");
      await click("[data-parameters-inspect]");
      const inspected = JSON.parse(await pollValue("native inspection response", () => inner("return d.querySelector('[data-parameters]').dataset.nativeReply??'';"), value => value.length > 0, 10000));
      await capture("01-inspected-gains");
      assert.strictEqual(inspected.status, "inspected", JSON.stringify(inspected));
      const baseline = JSON.parse(await inner("return d.querySelector('[data-parameters-values]').value;"));
      assert.deepStrictEqual(baseline, { [kp]: 2, [ki]: 0.5 }, "public gain editing must bind the actual Control.PID record");
      if (process.env.TRUST_MANUAL_PARAMETERS_ONLY === "1") {
        assert.strictEqual(await inner("return String(d.querySelectorAll('[data-parameter-symbol]').length);"), "2", "manual gains need labelled numeric fields");
        assert.strictEqual(await inner("return String(d.querySelector('[data-parameters-values]').checkVisibility());"), "false", "raw JSON belongs in closed details");
        assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Tune,\"]').dataset.status;"), "ready", "manual parameters do not require a fitted model");
      }
      assert.strictEqual(loop.controller.instance, "P1.Controller");
      assert.strictEqual(loop.controller.type, "Control.PID");
      const client = (await connectRuntimeParameters(loop)).client;
      const observed = async () => (await client.status()).values;
      const editGain = async (value: number) => {
        if (process.env.TRUST_MANUAL_PARAMETERS_ONLY === "1") await setWebviewValue(inner, `[data-parameter-symbol="${kp}"]`, String(value));
        else await setWebviewValue(inner, "[data-parameters-values]", JSON.stringify({ [kp]: value, [ki]: 0.5 }, null, 2));
      };
      await editGain(4.1);
      await click("[data-parameters-review]");
      await pollValue("gain bound rejection", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => /maximum|range|bound|4/.test(value), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-parameters-apply]').disabled);"), "true");
      assert.deepStrictEqual(await observed(), baseline, "invalid review must not write either gain");
      await capture("02-invalid-gain");
      const candidate = { [kp]: 2.4, [ki]: 0.5 };
      await editGain(2.4);
      await click("[data-parameters-review]");
      await pollValue("actual record review", () => inner("return [...d.querySelectorAll('[data-parameters-review-table] tr')].map(row=>row.cells[0].title+row.textContent).join(' | ');"), value => value.includes(kp) && value.includes(ki) && value.includes("LREAL") && value.includes("2.4"), 10000);
      assert.deepStrictEqual(await observed(), baseline, "review is read-only");
      await revealControlForCapture(inner, "[data-parameters-review-area]");
      await capture("03-reviewed-gain");
      if (process.env.TRUST_PARAMETER_EDIT_RETURN === "1") {
        await click("[data-parameters-change]");
        assert.strictEqual(await inner("return String(d.querySelector('[data-parameters-editor]').checkVisibility());"), "true");
        assert.strictEqual(await inner("return String(d.querySelector('[data-parameters-apply]').disabled);"), "true");
        assert.deepStrictEqual(JSON.parse(await inner("return d.querySelector('[data-parameters-values]').value;")), candidate);
        assert.deepStrictEqual(await observed(), baseline, "returning to edit does not write");
        await capture("03a-return-to-edit");
        await click("[data-parameters-review]");
        await pollValue("review after returning to edit", () => inner("return String(d.querySelector('[data-parameters-apply]').disabled);"), value => value === "false", 10000);
      }
      await click("[data-parameters-apply]");
      await pollValue("ST observed confirmation", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("observed after ST"), 10000);
      assert.deepStrictEqual(await observed(), candidate);
      await capture("04-applied-gain");
      if (process.env.TRUST_MANUAL_PARAMETERS_ONLY === "1") {
        await click("[data-parameters-rollback]");
        await pollValue("review rollback", () => inner("return d.querySelector('[data-parameters-apply]').textContent;"), value => value.includes("rollback"), 10000);
        await capture("10-reviewed-rollback"); await click("[data-parameters-apply]");
        await pollValue("observed rollback", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("Previous values restored"), 10000);
        assert.deepStrictEqual(await observed(), baseline);
        assert.deepStrictEqual(JSON.parse(await inner("return d.querySelector('[data-parameters-values]').value;")), baseline, "the displayed candidate reflects confirmed rollback");
        await capture("11-restored-gains");
        return;
      }
      await stage("Monitor"); await click('[data-plot-tab="signals"]');
      for (const id of ["request", "thermal-load", "kp", "ki"]) {
        await chooseHotHeaterSignal(inner, loop.signals.additional.find((s: { id: string }) => s.id === id).symbol, !["kp", "ki"].includes(id));
      }
      await capture("05-observed-gains");
      await click('[data-plot-tab="response"]'); await stage("Test"); await click("[data-test-review]");
      await pollValue("reviewed bounded trial", () => inner("return String(d.querySelector('[data-test-approve]').hidden);"), value => value === "false", 10000);
      await capture("06-reviewed-trial"); await click("[data-test-approve]");
      await pollValue("trial running", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Test running/.test(value), 10000);
      await stage("Monitor"); await sleep(1200); await capture("07-candidate-response"); await stage("Test");
      await pollValue("complete native sixty-second candidate trial", () => inner("return d.querySelector('[data-test-state]').textContent;"), value => /Completed/i.test(value), 75000);
      await stage("Monitor");
      await pollValue("observed stopped heater", async () => {
        await runtimeLifecycleService.requestIoState(); return inner("return d.querySelector('[data-control-studio-mv]').textContent;");
      }, value => Number.parseFloat(value) === 0, 5000);
      await pollValue("complete candidate recording", () => inner("return d.querySelector('[data-control-studio-recording-evidence]').textContent;"), value => /Recording retained/.test(value), 10000);
      await capture("08-complete-trial");
      await inner("d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.workflow-result'&&event.data.phase==='opened')d.querySelector('[data-control-studio]').dataset.manualResult=JSON.stringify(event.data.payload.result);});return 'observe actual Open';");
      await stage("Validate"); await click("[data-result-save]");
      await pollValue("saved native candidate trial", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /Saved revision/.test(value), 10000);
      await capture("09-saved-trial");
      await click("[data-workflow-pane=Validate] details:has([data-result-history]) > summary");
      await revealControlForCapture(inner, "[data-result-history]");
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].filter(o=>o.value).sort((a,b)=>Number(b.value)-Number(a.value))[0].value;");
      await setWebviewValue(inner, "[data-result-history]", revision); await click("[data-result-open]");
      const result = JSON.parse(await pollValue("opened native trial", () => inner("return d.querySelector('[data-control-studio]').dataset.manualResult??'';"), value => value.length > 0, 10000)) as ControlStudioWorkflowResult;
      fs.writeFileSync(path.join(captures, "native-result.json"), JSON.stringify(result, null, 2));
      assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
      assert.strictEqual(result.recording.acquisition?.completeness, "complete");
      assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
      const active = result.recording.rawSamples.filter(sample => channel(sample, "experiment-authority") === 1);
      assert.ok(active.length >= 600);
      assert.ok(Math.abs(channel(active[0], "request") - (trial === 0 ? 82.35 : 75)) < 1e-8);
      for (let i = 0; i < 600; i++) {
        const sample = active[i];
        assert.strictEqual(sample.sp, trial === 0 ? 38 : 35);
        assert.strictEqual(channel(sample, "thermal-load"), trial === 0 ? 0 : 30);
        assert.strictEqual(channel(sample, "kp"), 2.4); assert.strictEqual(channel(sample, "ki"), 0.5);
        assert.ok(sample.appliedMv! >= 0 && sample.appliedMv! <= 100);
        if (i > 0) {
          assert.strictEqual(sample.scan, active[i - 1].scan + 1);
          assert.ok(Math.abs(sample.elapsedSeconds - active[0].elapsedSeconds - i * 0.1) < 1e-8);
        }
      }
      assert.strictEqual(result.validation, undefined, "a completed manual experiment does not manufacture a validation pass");
      await stage("Tune"); await click("[data-parameters-rollback]");
      await pollValue("review previous gains", () => inner("return d.querySelector('[data-parameters-apply]').textContent;"), value => value.includes("rollback"), 10000);
      await capture("10-reviewed-rollback"); await click("[data-parameters-apply]");
      await pollValue("observed previous gains", () => inner("return d.querySelector('[data-parameters-status]').textContent;"), value => value.includes("Previous values restored"), 10000);
      assert.deepStrictEqual(await observed(), baseline);
      await capture("11-restored-gains");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});

function channel(sample: WorkflowResultSample, id: string): number {
  const found = sample.additional?.find(value => value.id === id);
  assert.ok(found && typeof found.value === "number" && Number.isFinite(found.value), `${id} at scan ${sample.scan}`);
  assert.strictEqual(found.quality, "good");
  return found.value;
}
