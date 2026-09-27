import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { configureLibraryPidThroughDialogs } from "./control-studio-library-pid-setup";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment,
  captureRenderedWorkbenchCdp, type CdpConnection } from "./plant-rendered-harness";

suite("Control Studio real Control.PID commissioning journey", () => {
  test("identifies tunes verifies adopts and rolls back the seven-state library controller", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_LIBRARY_PID_JOURNEY !== "1") this.skip();
    this.timeout(180000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME ?? "Default Dark Modern";
    const family = process.env.TRUST_CONTROL_STUDIO_FAMILY ?? "PI";
    assert.ok(["P", "PI", "PD", "PID"].includes(family));
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    let loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.controller.pidInitialization.kind, "control-pid-v1");
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const events: Record<string, any> = {};
    let connection: CdpConnection | undefined;
    try {
      if (process.env.TRUST_CONTROL_STUDIO_LIBRARY_PID_SETUP === "1") fs.unlinkSync(path.join(project, "control-loop.json"));
      const started = await runtimeLifecycleService.startLocalSimulator();
      assert.strictEqual(started.ok, true, JSON.stringify(started));
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      if (process.env.TRUST_CONTROL_STUDIO_LIBRARY_PID_SETUP === "1") {
        loop = await configureLibraryPidThroughDialogs(connection, port, project, loop, captures);
      }
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      let session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection!, session, body);
      const capture = async (name: string) => {
        await vscode.commands.executeCommand("notifications.clearAll");
        await new Promise(resolve => setTimeout(resolve, 250));
        events.captureFacts ??= {};
        events.captureFacts[name] = { theme: vscode.workspace.getConfiguration("workbench").get("colorTheme"),
          kind: vscode.window.activeColorTheme.kind, bodyClass: await inner("return d.body.className;") };
        await captureRenderedWorkbenchCdp(connection!, path.join(captures, `${name}.png`));
      };
      const click = async (selector: string) => {
        const actual = await inner(`const b=d.querySelector(${JSON.stringify(selector)});const r=b?.getBoundingClientRect();
          if(!b||b.disabled||b.hidden||!r||r.width===0||r.height===0)return 'unavailable';b.click();return 'clicked';`);
        assert.strictEqual(actual, "clicked", `${selector} must be visible and available`);
      };
      const set = async (selector: string, value: string) => {
        const actual = await inner(`const b=d.querySelector(${JSON.stringify(selector)});const r=b?.getBoundingClientRect();
          if(!b||b.disabled||!r||r.width===0||r.height===0)return 'unavailable';b.value=${JSON.stringify(value)};
          b.dispatchEvent(new Event('input',{bubbles:true}));b.dispatchEvent(new Event('change',{bubbles:true}));return b.value;`);
        assert.strictEqual(actual, value);
      };
      const run = async (name: string, selector: string, success: string, timeout = 40000) => {
        await inner("d.defaultView.__libraryPidEvent=undefined;return 'observe operation';");
        await click(selector);
        const result = JSON.parse(await pollValue(name, () => inner("return JSON.stringify(d.defaultView.__libraryPidEvent??{});"),
          value => [success, "failed", "verification-failed", "recovery-required"].includes(JSON.parse(value).status), timeout));
        events[name] = result;
        await capture(name);
        assert.strictEqual(result.status, success, JSON.stringify(result));
        return result;
      };
      await pollValue("workflow handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
      await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>{d.defaultView.__libraryPidEvent=e.detail;});return 'listen';");
      await capture("01-start");
      const identified = await run("02-identified", "[data-engineer-find]", "model-ready", 60000);
      assert.ok(identified.identificationPresentation?.predictedResponse?.length > 2,
        "Identify must retain measured-versus-predicted evidence for the selected general model");
      await pollValue("visible model fit", () => inner("const r=d.querySelector('[data-plot-view=fit]')?.getBoundingClientRect();return String(!!r&&r.width>0&&r.height>0);"), value => value === "true", 5000);
      await capture("02-model-fit");
      await click("[data-engineer-use-model]");
      await set("[data-engineer-family]", family);
      if (process.env.TRUST_CONTROL_STUDIO_MAXIMUM_ERROR) await set("[data-engineer-error]", process.env.TRUST_CONTROL_STUDIO_MAXIMUM_ERROR);
      await set("[data-engineer-settling]", process.env.TRUST_CONTROL_STUDIO_VERIFY_RECOVERY === "1" ? "0.5" : "12");
      await click("[data-engineer-options] > summary");
      await click("[data-engineer-settings-stage=tune] details:has([data-engineer-duration]) > summary");
      await set("[data-engineer-duration]", "24");
      if (family === "P") {
        const manualGainHidden = () => inner("const r=d.querySelector('[data-engineer-gain-value]').closest('label').getBoundingClientRect();return String(r.height===0); ");
        assert.strictEqual(await manualGainHidden(), "true", "Automatic gain must not show an unrelated fixed value");
        await set("[data-engineer-gain-mode]", "manual");
        assert.strictEqual(await manualGainHidden(), "false");
        await set("[data-engineer-gain-value]", "2");
        await set("[data-engineer-gain-mode]", "automatic");
        assert.strictEqual(await manualGainHidden(), "true");
      }
      await capture("03-requirements");
      await click("[data-engineer-inspector-close=settings]");
      const tuned = await run("04-tuned", "[data-engineer-tune]", "candidate");
      assert.strictEqual(tuned.family, family, "preserve the engineer's accepted controller choice");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-workflow]').dataset.selectedStage;"), "tune",
        "keep the tuned result visible until the engineer runs verification");
      await pollValue("tuned preview visible", () => inner("const r=d.querySelector('[data-plot-view=engineering-response]')?.getBoundingClientRect();return String(!!r&&r.width>0&&r.height>0);"), value => value === "true", 5000);
      await capture("04-prediction");
      if (process.env.TRUST_CONTROL_STUDIO_REOPEN === "1") {
        const previousEditor = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
        const tab = vscode.window.tabGroups.all.flatMap(group => group.tabs).find(tab =>
          tab.input instanceof vscode.TabInputWebview && tab.label.includes("Control Studio"));
        assert.ok(tab, "close the owned Control Studio tab");
        assert.ok(await vscode.window.tabGroups.close(tab!));
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
          scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
        session = await attachControlStudio(connection, port, previousEditor, loop.id);
        await pollValue("restored workflow handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
        await pollValue("restored controller choice", () => inner("return d.querySelector('[data-engineer-family]').value;"), value => value === family, 10000);
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-settling]').value;"), "12");
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-duration]').value;"), "24");
        await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>{d.defaultView.__libraryPidEvent=e.detail;});return 'listen after reopen';");
        await capture("04-reopened-study");
        await click('[data-engineer-stage-tab="model"]');
        await pollValue("restored independent fit", () => inner("const r=d.querySelector('[data-plot-view=fit]')?.getBoundingClientRect();return String(!!r&&r.width>0&&r.height>0);"), value => value === "true", 5000);
        await capture("04-reopened-model-fit");
        await click("[data-engineer-use-model]");
        const restoredTuning = await run("04-retuned-study", "[data-engineer-tune]", "candidate");
        assert.strictEqual(restoredTuning.family, family);
      }
      if (process.env.TRUST_CONTROL_STUDIO_VERIFY_RECOVERY === "1") {
        const failed = await run("05-verification-failed", "[data-engineer-verify]", "verification-failed", 60000);
        assert.strictEqual(failed.stageEvidence.validationPassed, false);
        assert.ok(failed.candidate?.run?.samples?.length > 10, "retain the unsuccessful actual-ST response");
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-workflow]').dataset.selectedStage;"), "verify", "show actual failed response at its owning stage");
        await capture("05-failed-response");
        await click("[data-engineer-prerequisite-action]");
        await set("[data-engineer-settling]", "12");
        await run("05-recovery-retuned", "[data-engineer-tune]", "candidate");
      }
      const verified = await run("05-verified", "[data-engineer-verify]", "verified", 60000);
      assert.strictEqual(verified.initialization.status, "Pass");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-workflow]').dataset.selectedStage;"), "verify",
        "keep actual verification results visible until application review");
      const actual = verified.scenarioResults[0].result.run;
      const firstOutput = actual.samples.find((sample: any) => sample.timeSeconds > 0).values["P1.ManipulatedValue"].value;
      const interval = loop.controller.taskPeriodMs / 1000;
      const initialOutput = actual.conditions.initialState["P1.Controller.StateValue.SaturatedOutput"];
      const expectedFirstOutput = Math.max(actual.parameters["P1.Parameters.OutputMinimum"],
        initialOutput + interval * actual.parameters["P1.Parameters.RateMinimum"],
        Math.min(actual.parameters["P1.Parameters.OutputMaximum"],
          initialOutput + interval * actual.parameters["P1.Parameters.RateMaximum"],
          actual.parameters["P1.Parameters.ProportionalGain"] + interval * actual.parameters["P1.Parameters.IntegralGain"]));
      assert.ok(Math.abs(firstOutput - expectedFirstOutput) < 1e-6,
        `the first ${family} output must follow reviewed history and actuator limits: ${firstOutput} vs ${expectedFirstOutput}`);
      await click("[data-engineer-apply]");
      await pollValue("application review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
      await capture("06-review");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-readiness]').textContent;"), "Ready to apply");
      const markerBounds = JSON.parse(await inner(`
        const host=d.querySelector('[data-control-studio-chart]').getBoundingClientRect();
        return JSON.stringify([...d.querySelectorAll('.chart-annotation')].map(marker=>{
          const rect=marker.getBoundingClientRect();return {left:rect.left-host.left,top:rect.top-host.top,
            right:host.right-rect.right,bottom:host.bottom-rect.bottom,title:marker.title};
        }));
      `)) as { left: number; top: number; right: number; bottom: number; title: string }[];
      assert.ok(markerBounds.some(marker => /Output entered minimum saturation/.test(marker.title)),
        "the initial saturation event must remain inspectable");
      for (const marker of markerBounds) {
        assert.ok(marker.left > 20 && marker.right > 20 && marker.top > 20 && marker.bottom > 20,
          "live event markers must stay inside chart axes and away from legend/time labels");
        assert.match(marker.title, /^Simulation · \d.* s · .+/);
      }
      await click('[data-chart-action="zoom-in"]');
      assert.strictEqual(await inner("return String(d.querySelectorAll('.chart-annotation').length);"), "0",
        "zoom must hide the initial saturation event outside the current time window");
      await click('[data-chart-action="reset"]');
      assert.strictEqual(await inner("return String(d.querySelectorAll('.chart-annotation').length);"), String(markerBounds.length));
      const reviewRows = Number(await inner("return String(d.querySelectorAll('[data-engineer-evidence-stage=apply] [data-parameter-comparison] tbody tr').length);"));
      assert.strictEqual(reviewRows, loop.parameterBinding.parameters.length, "the real review must expose every bound parameter as its own row");
      const applied = await run("07-commissioned", "[data-engineer-confirm]", "commissioned", 45000);
      const transfers = applied.commissioning?.transfers;
      assert.ok(Array.isArray(transfers) && transfers.length >= 2, "retain actual task-boundary and target transfers");
      const entry = transfers.find((transfer: any) => transfer.transaction?.applied_values?.["P1.Controller.StateValue.NominalOutput"] !== undefined);
      assert.ok(entry, "the runtime must report all actual library state writes");
      for (const state of loop.controller.pidInitialization.states) {
        assert.ok(Number.isFinite(entry.transaction.applied_values[state.symbol]), `${state.symbol} was adopted`);
        assert.ok(Number.isFinite(entry.transaction.observed_values[state.symbol]), `${state.symbol} was observed after ST`);
      }
      assert.strictEqual(entry.transaction.confirmation, "output-image");
      await click("[data-engineer-view-results]");
      await capture("08-recorded-results");
      const observedLayout = JSON.parse(await inner(`const table=Array.from(d.querySelectorAll('[data-engineer-evidence-stage=apply] table')).find(t=>t.caption?.textContent==='Observed values');
        const cells=table?.querySelector('tr')?.children;return JSON.stringify(cells?{label:cells[0].getBoundingClientRect().width,value:cells[1].getBoundingClientRect().width}:{});`));
      assert.ok(observedLayout.label > observedLayout.value, "observed-value labels need more room than single numeric values");
      await inner(`const tables=Array.from(d.querySelectorAll('[data-engineer-evidence-stage=apply] table'));
        const table=tables.find(t=>t.caption?.textContent==='Observed values');table?.querySelector('tr:last-child')?.scrollIntoView({block:'nearest'});return 'inspect last observed setting';`);
      await capture("08-observed-settings");
      await click("[data-engineer-inspector-close=details]");
      await click("[data-engineer-rollback]");
      await pollValue("rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
      await capture("09-rollback-review");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-readiness]').textContent;"), "Ready to restore");
      const rollbackText = await inner("return d.querySelector('[data-engineer-status]').textContent;");
      assert.ok(!/interrupted|\{\"/.test(rollbackText), "normal rollback must describe the operator action without an interruption claim or raw JSON");
      await click("[data-engineer-confirm]");
      await pollValue("rollback restored", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Previous values restored/.test(value), 15000);
      await capture("10-restored");
    } finally {
      fs.writeFileSync(path.join(captures, "library-pid-journey.json"), JSON.stringify(events, null, 2));
      connection?.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
