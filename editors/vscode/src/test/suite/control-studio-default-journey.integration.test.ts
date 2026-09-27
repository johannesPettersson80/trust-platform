import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { exerciseNativePlotExport } from "./control-studio-native-export-interaction";
import { exerciseVisiblePlotToolbar } from "./control-studio-toolbar-interactions";
import { exerciseReplacementModel } from "./control-studio-replacement-model-interaction";
import { exerciseMissingModelNavigation, exerciseRequiredGoals, exerciseStageInspectors } from "./control-studio-stage-inspector-interaction";
import { installManualRecoveryFault } from "./control-studio-manual-recovery-fault";
import { installRestartRecoveryFault, reviewRestartedRuntime } from "./control-studio-restart-review-interaction";
import { exerciseAcquisitionReview, exerciseRequiredScenarioReview } from "./control-studio-guided-recovery-actions";
import { exerciseRejectedManualTune } from "./control-studio-manual-tune-rejection";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment,
  captureRenderedWorkbench, captureRenderedWorkbenchCdp, type CdpConnection } from "./plant-rendered-harness";

suite("Control Studio unchanged default journey", () => {
  test("identifies tunes verifies and commissions the nominal project using visible default actions", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_DEFAULT_JOURNEY !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const declaredManual = process.env.TRUST_CONTROL_STUDIO_DECLARED_MANUAL_AUDIT === "1";
    if (declaredManual) {
      loop.controller.project = { family: "P", convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states: [] }, axes: [{ symbol: "Main.Kp", values: [12] }],
        objective: { requirementId: "response-deadline", direction: "minimize" }, requiredScenarioIds: ["extra-check"],
        maximumCandidates: 1, maximumCycles: 10000, wallBudgetMs: 10000 };
      delete loop.controller.manualOutputType;
      fs.writeFileSync(path.join(project, "control-loop.json"), JSON.stringify(loop, null, 2));
    }
    const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT");
    const captures = path.dirname(screenshot);
    let connection: CdpConnection | undefined;
    const observed: Record<string, unknown> = {};
    const manualRecovery = process.env.TRUST_CONTROL_STUDIO_MANUAL_RECOVERY_AUDIT === "1" ? installManualRecoveryFault() : undefined;
    const restartRecovery = process.env.TRUST_CONTROL_STUDIO_RESTART_REVIEW_AUDIT === "1" ? installRestartRecoveryFault() : undefined;
    try {
      assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
      connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection!, session, body);
      const capture = async (name: string) => {
        await new Promise(resolve => setTimeout(resolve, 250));
        await vscode.commands.executeCommand("notifications.clearAll");
        captureRenderedWorkbench(path.join(captures, `${name}.png`));
      };
      const click = async (selector: string) => {
        if (/^\[data-engineer-(find|tune|verify|apply|confirm|cancel|rollback|manual-staging|retry-recovery)\]$/.test(selector)) {
          await inner("d.defaultView.__defaultLatest=undefined;return 'observe next operation';");
        }
        const result = await inner(`const b=d.querySelector(${JSON.stringify(selector)});const r=b?.getBoundingClientRect();
          if(!b||b.disabled||b.hidden||!r||r.width===0||r.height===0)return 'unavailable';b.click();return 'clicked';`);
        assert.strictEqual(result, "clicked", `${selector} must be visibly available`);
      };
      const terminal = async (name: string, statuses: string[], timeout: number) => {
        const event = JSON.parse(await pollValue(name, () => inner("return JSON.stringify(d.defaultView.__defaultLatest??{});"),
          value => statuses.includes(JSON.parse(value).status), timeout));
        observed[name] = event;
        await capture(name);
        return event;
      };
      await pollValue("handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), value => value === "true", 10000);
      await inner("d.defaultView.addEventListener('trust-control-studio-engineer',e=>{d.defaultView.__defaultLatest=e.detail;});return 'listen';");
      if (process.env.TRUST_CONTROL_STUDIO_NAVIGATION_AUDIT === "1") await exerciseMissingModelNavigation(inner, capture);
      await capture("01-start");
      if (declaredManual) await exerciseAcquisitionReview(inner, click, capture);
      await click("[data-engineer-find]");
      const model = await terminal("02-model", ["model-ready", "failed"], 40000);
      assert.strictEqual(model.status, "model-ready", model.reason);
      if (process.env.TRUST_CONTROL_STUDIO_TOOLBAR_AUDIT === "1") await exerciseVisiblePlotToolbar(inner, capture, "model", true);
      await click("[data-engineer-use-model]");
      await capture("03-tune-target");
      if (declaredManual) {
        await click("[data-engineer-options] > summary");
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-gain-controls]').hidden&&d.querySelector('[data-engineer-gain-value]').disabled);"), "true", "project-owned tuning uses declared parameters rather than builtin gain controls");
        await capture("declared-tuning-settings");
        await click("[data-engineer-inspector-close=settings]");
      }
      assert.strictEqual(await inner("return ['reference','error','settling'].map(n=>d.querySelector('[data-engineer-'+n+']').value).join(',');"), "1,0.1,2");
      if (process.env.TRUST_CONTROL_STUDIO_NAVIGATION_AUDIT === "1") await exerciseRequiredGoals(inner, capture);
      if (process.env.TRUST_CONTROL_STUDIO_MANUAL_TUNE_FAILURE_AUDIT === "1") await exerciseRejectedManualTune(inner, click, capture);
      await click("[data-engineer-tune]");
      let tuned = await terminal("04-candidate", ["candidate", "failed"], 15000);
      if (declaredManual) {
        assert.strictEqual(tuned.failure?.nextAction, "configure-scenarios");
        await exerciseRequiredScenarioReview(inner, click, capture);
        await click("[data-engineer-tune]");
        tuned = await terminal("04-declared-candidate", ["candidate", "failed"], 15000);
      }
      assert.strictEqual(tuned.status, "candidate", tuned.reason);
      if (!declaredManual) {
      assert.ok(tuned.plots?.timeResponse?.length > 0, "the supported first-order P candidate must provide its predicted response");
      assert.ok(tuned.plots.timeResponse.every((point: any) => point.currentPv === undefined && point.currentMv === undefined),
        "a candidate-only prediction cannot invent a current-controller response");
      assert.deepStrictEqual(tuned.plots.requirements, [], "generic relative-to-baseline criteria cannot replace the engineer's actual ST task requirements");
      const predictedPv = tuned.plots.timeResponse.map((point: any) => point.candidatePv);
      assert.ok(Math.max(...predictedPv) <= predictedPv[predictedPv.length - 1] + 1e-6,
        "a zero-delay first-order P prediction must not invent delayed oscillations");
      await click("[data-engineer-stage-tab=tune]");
      const predictionGeometry = await pollValue("visible prediction canvas", () => inner(`
        const host=d.querySelector('[data-engineering-time]');const canvas=host?.querySelector('canvas');
        const rect=host?.getBoundingClientRect();const painted=canvas?.getBoundingClientRect();
        return JSON.stringify({stage:d.querySelector('[data-engineer-stage-tab][aria-selected=true]')?.dataset.engineerStageTab,
          width:rect?.width,height:rect?.height,canvasWidth:painted?.width,canvasHeight:painted?.height});`), value => {
          const geometry=JSON.parse(value);
          return geometry.stage === "tune" && geometry.width > 200 && geometry.height > 100 &&
            Math.abs(geometry.width-geometry.canvasWidth) < 2 && Math.abs(geometry.height-geometry.canvasHeight) < 2;
        }, 5000);
      observed.predictionGeometry=JSON.parse(predictionGeometry);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "04-prediction-compositor.png"));
      await capture("04-prediction");
      await click("[data-engineer-stage-tab=verify]");
      }
      if (manualRecovery) {
        await click("[data-engineer-stage-tab=apply]");
        await click("[data-engineer-options] > summary");
        await inner("const s=d.querySelector('[data-engineer-application-mode]');s.value='manual';s.dispatchEvent(new Event('change'));return 'manual';");
        await capture("04-manual-staging-mode");
        await click("[data-engineer-inspector-close=settings]");
        await click("[data-engineer-stage-tab=verify]");
      }
      await click("[data-engineer-verify]");
      let verified = await terminal("05-verified", ["verified", "verification-failed", "failed"], 40000);
      if (declaredManual) {
        assert.strictEqual(verified.failure?.nextAction, "choose-application-mode");
        await capture("manual-staging-offered");
        await inner("d.defaultView.__defaultLatest=undefined;return 'observe Manual verification';");
        await click("[data-engineer-manual-staging]");
        verified = await terminal("05-manual-verified", ["verified", "verification-failed", "failed"], 40000);
        assert.strictEqual(await inner("return d.querySelector('[data-engineer-application-mode]').value;"), "manual");
      }
      assert.strictEqual(verified.status, "verified", verified.reason);
      await click("[data-engineer-apply]");
      await pollValue("application review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
      await capture("06-review");
      await click("[data-engineer-confirm]");
      if (restartRecovery) {
        await reviewRestartedRuntime(connection, port, inner, loop, restartRecovery, capture);
        return;
      }
      if (manualRecovery) {
        const failure = JSON.parse(await pollValue("Manual acknowledgement failure", () => inner("return JSON.stringify({status:d.defaultView.__defaultLatest?.status,reason:d.defaultView.__defaultLatest?.reason});"), value => ["failed", "recovery-required"].includes(JSON.parse(value).status), 30000));
        assert.strictEqual(failure.status, "recovery-required", "a lost Manual acknowledgement must offer recovery rather than a new tuning action");
        await pollValue("visible Manual recovery", () => inner("const b=d.querySelector('[data-engineer-retry-recovery]');const r=b.getBoundingClientRect();const p=d.defaultView.__defaultLatest;return JSON.stringify({visible:!b.disabled&&r.width>0&&r.height>0,status:p?.status,reason:p?.reason,retry:p?.retryManualRecovery});"), value => JSON.parse(value).visible === true, 30000);
        await capture("06-manual-recovery-required");
        assert.strictEqual(await inner("return String(['find','tune','verify','apply'].every(n=>d.querySelector('[data-engineer-'+n+']').disabled));"), "true", "unconfirmed application must block new dependent operations");
        assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-scenarios]').disabled);"), "true", "recovery locks scenario edits as well as other task inputs");
        await inner("d.defaultView.__defaultLatest=undefined;return 'observe recovery review';");
        await click("[data-engineer-rollback]");
        const retriedReview = JSON.parse(await pollValue("recovery review failure", () => inner("return JSON.stringify({status:d.defaultView.__defaultLatest?.status});"), value => ["failed", "recovery-required", "review"].includes(JSON.parse(value).status), 10000));
        assert.strictEqual(retriedReview.status, "recovery-required", "a failed recovery review must retain the pending-operation route");
        manualRecovery.unblock();
        await click("[data-engineer-retry-recovery]");
      }
      if (process.env.TRUST_CONTROL_STUDIO_CANCEL_AUDIT === "1") {
        await pollValue("live Auto before Cancel", () => inner("return String(d.querySelector('[data-control-studio-controller-mode]').textContent==='Auto'&&Number(d.querySelector('[data-control-studio-pv]').textContent.split(' ')[0])>0.1);"), value => value === "true", 10000);
        await capture("06-live-auto-before-cancel");
        await click("[data-engineer-cancel]");
        const cancelled = await terminal("07-cancelled", ["cancelled", "failed"], 20000);
        assert.strictEqual(cancelled.commissioning?.recoveryConfirmed, true, cancelled.reason);
        assert.strictEqual(cancelled.commissioning?.parameters?.values?.["Main.Kp"], 1);
        await pollValue("observed Manual recovery", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 10000);
        await capture("08-cancel-recovery");
        return;
      }
      const applied = await terminal("07-applied", ["commissioned", "applied", "failed"], 30000);
      assert.strictEqual(applied.status, manualRecovery || declaredManual ? "applied" : "commissioned", applied.reason);
      if (manualRecovery) assert.strictEqual(manualRecovery.writes(), 1, "Retry recovery must not repeat the committed gain write");
      await click("[data-engineer-view-results]");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-results]').open);"), "true");
      await capture("07-result-details");
      await click("[data-engineer-inspector-close=details]");
      await click("[data-engineer-rollback]");
      await pollValue("rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10000);
      await capture("08-rollback-review");
      await click("[data-engineer-confirm]");
      await pollValue("rollback confirmed", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Previous values restored/.test(value), 10000);
      await capture("09-restored");
      if (process.env.TRUST_CONTROL_STUDIO_NAVIGATION_AUDIT === "1") {
        await exerciseStageInspectors(inner, capture);
        await click("[data-engineer-stage-tab=apply]");
      }
      if (process.env.TRUST_CONTROL_STUDIO_TOOLBAR_AUDIT === "1") await exerciseVisiblePlotToolbar(inner, capture, "live", false);
      await exerciseNativePlotExport(inner, project, capture);
      if (process.env.TRUST_CONTROL_STUDIO_MODEL_REPLACEMENT === "1") await exerciseReplacementModel(connection, session, inner, project, capture);
    } finally {
      fs.writeFileSync(path.join(captures, "default-journey.json"), JSON.stringify(observed, null, 2));
      connection?.close();
      manualRecovery?.restore();
      restartRecovery?.restore();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      await runtimeLifecycleService.stopRuntime();
    }
  });
});
