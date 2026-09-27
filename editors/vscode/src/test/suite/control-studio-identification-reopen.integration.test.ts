import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { formatLocalTimestamp } from "../../controlStudio/timestampFormat";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";
import { validateImportedStepWithNativeTest } from "./control-studio-step-validation-harness";

suite("Control Studio identification process restart", () => {
  test("restores the saved analysis in a new editor process and preserves independent model acceptance", async function () {
    const phase = process.env.TRUST_IDENTIFICATION_RESTART_PHASE;
    if (!["preview", "worker-preview", "accepted", "invalidate", "invalidated", "inspect"].includes(phase ?? "")) this.skip();
    const workerPreview = phase === "worker-preview";
    this.timeout(180000);
    assert.strictEqual(process.env.TRUST_HOT_HEATER_REOPEN_ONLY, "1", "restart proof must reuse a previous process's workspace/profile");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const previewFile = process.env.TRUST_CONTROL_STUDIO_LESSON_SEQUENCE === "16" ? "saved-comparison.json" : "opened-imported-result.json";
    const originalPreview = workerPreview ? JSON.parse(fs.readFileSync(path.join(
      requiredEnvironment("TRUST_HOT_HEATER_RESUME_RUN"), "evidence", previewFile), "utf8")) : undefined;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let started = false, opened = false;
    try {
      if (phase === "preview") {
        const start = await runtimeLifecycleService.startLocalSimulator();
        assert.strictEqual(start.ok, true, JSON.stringify(start)); started = true;
      }
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop }); opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = (name: string) => inner(`var b=[...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.getAttribute('aria-label')?.startsWith(${JSON.stringify(name + ',')}));b.click();return 'stage';`);
      const capture = async (name: string) => {
        await sleep(250);
        if (/^(01|02|03|04|11|12|13|inspect)-/.test(name)) {
          assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-selected=\"true\"]').textContent.trim();"), "Identify",
            `${name} must capture visible Identify, not assert against hidden controls`);
        }
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `restart-${name}.png`));
      };
      await pollValue("reopened handlers", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), v => v === "true", 10000);
      const invalidated = phase === "invalidated" || phase === "inspect";
      const expected = phase === "preview" || workerPreview ? /Preview only/ : invalidated ? /Estimation recording qualified/ : /Accepted for design/;
      assert.match(await pollValue("identification automatically restored from persistent storage", () => inner("return d.querySelector('[data-identify-state]')?.textContent??'';"), v => expected.test(v), 15000), expected);
      // Saved chart navigation is restored before the Identify projection arrives.
      // Navigate after that projection, then verify the actual selected tab at capture.
      await stage("Identify");
      const originalSettings = originalPreview?.identificationStudy.current.stepResponseSettings;
      const heldStep = !workerPreview || Boolean(originalSettings);
      assert.strictEqual(await inner("return String(d.querySelector('[data-step-response-enabled]').checked);"), String(heldStep),
        "reopening must preserve the saved model-selection mode");
      if (heldStep) {
        assert.strictEqual(await inner("return d.querySelector('[data-step-field=" + JSON.stringify("delay.minimum") + "]').value;"), workerPreview ? String(originalSettings.delay.minimum) : invalidated ? "0.8" : "0.7");
        assert.strictEqual(await inner("return d.querySelector('[data-step-field=" + JSON.stringify("delay.step") + "]').value;"), workerPreview ? String(originalSettings.delay.step) : "0.05");
      } else {
        assert.strictEqual(await inner("return d.querySelector('#identify-model').value;"), originalPreview.identificationStudy.current.requestedKind);
      }
      await capture("01-restored-identification");
      if (heldStep) {
        await inner("d.querySelector('[data-step-response-settings]>summary').click();d.querySelector('[data-step-field=" + JSON.stringify("delay.step") + "]').scrollIntoView({block:'center'});return 'settings';");
        await capture("02-restored-settings");
        await inner("d.querySelector('[data-step-response-settings]>summary').click();return 'close settings';");
      }
      await inner("var root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',e=>{var m=e.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened')root.dataset.restartResult=JSON.stringify(m.payload.result);if(m?.kind==='control-studio.engineering.result'&&m.stage==='identify'&&m.payload?.phase!=='qualified')root.dataset.stepReview=JSON.stringify(m.payload);});return 'observe';");
      const openLatest = async (revision: number) => {
        await stage("Validate");
        await inner("var h=d.querySelector('[data-result-history]').closest('details');h.open=true;return 'history';");
        const value = await inner(`return [...d.querySelector('[data-result-history]').options].find(o=>o.textContent.includes('Revision ${revision}'))?.value??'';`);
        assert.ok(value, `saved revision ${revision} must survive the previous editor process`);
        await inner(`delete d.querySelector('[data-control-studio]').dataset.restartResult;var h=d.querySelector('[data-result-history]');h.value=${JSON.stringify(value)};h.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-result-open]').click();return 'open';`);
        const saved = JSON.parse(await pollValue("saved analysis opened", () => inner("return d.querySelector('[data-control-studio]').dataset.restartResult??'';"), v => Boolean(v), 10000));
        assert.strictEqual(await inner("return d.querySelector('[data-historical-result-badge]').textContent;"),
          `Saved result · ${formatLocalTimestamp(saved.createdAt).display}`,
          "the value-strip timestamp must belong to this opened revision, not the previous result");
        return saved;
      };
      const assertInvalidated = async (saved: any) => {
        assert.strictEqual(saved.identificationStudy.phase, "qualified");
        assert.strictEqual(saved.model, undefined, "the saved accepted model must be invalidated");
        assert.strictEqual(saved.generalModelAnalysis, undefined);
        assert.strictEqual(saved.engineerStudy?.selectedModel, undefined, "the shared tuning session must not retain the old model");
        assert.strictEqual(saved.candidate, undefined);
        assert.strictEqual(saved.validation, undefined);
        assert.ok(saved.recording.rawSamples.length >= 451, "invalidation must preserve the complete original record");
        await stage("Identify");
        assert.notStrictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Identify,\"]').dataset.status;"), "complete");
        assert.strictEqual(await inner("return String(d.querySelector('[data-tune-generate]').disabled);"), "true",
          "Tune must stop offering the superseded accepted model");
      };
      let result = await openLatest(phase === "preview" || workerPreview ? 1 : invalidated ? 3 : 2);
      if (invalidated) {
        await assertInvalidated(result);
        await capture("03-invalidated-after-process-restart");
        fs.writeFileSync(path.join(captures, "reopened-identification-result.json"), JSON.stringify(result, null, 2));
        if (phase === "inspect") {
          for (const revision of [1, 2]) {
            const saved = await openLatest(revision);
            const wanted = revision === 1 ? /Preview only/ : /Accepted for design/;
            await pollValue("opened identification phase", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => wanted.test(v), 10000);
            await stage("Identify");
            assert.strictEqual(saved.identificationStudy.phase, revision === 1 ? "preview" : "accepted");
            assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-label^=\"Tune,\"]').dataset.status;"), revision === 1 ? "needs-data" : "ready");
            await capture(`inspect-revision-${revision}`);
            await inner("d.querySelector('[data-step-response-settings]').open=true;d.querySelector('[data-step-field=" + JSON.stringify("delay.step") + "]').scrollIntoView({block:'center'});return 'settings';");
            assert.strictEqual(await inner("return d.querySelector('[data-step-field=" + JSON.stringify("delay.step") + "]').value;"), "0.05");
            await capture(`inspect-revision-${revision}-settings`);
            await inner("d.querySelector('[data-step-response-settings]').open=false;return 'close settings';");
          }
          await openLatest(3);
        }
        return;
      }
      assert.strictEqual(result.identificationStudy.phase, workerPreview ? "preview" : phase === "invalidate" ? "accepted" : phase);
      assert.strictEqual(result.identificationStudy.estimation.recording.data.rawSamples.length, 303);
      assert.strictEqual(result.identificationStudy.estimation.recording.data.provenance.projectId, "project:control-lesson-21");
      if (workerPreview) {
        assert.deepStrictEqual(result.identificationStudy, originalPreview.identificationStudy,
          "worker replay must retain every saved analysis input and original record");
        assert.deepStrictEqual(result.recording, originalPreview.recording);
        assert.strictEqual(result.model, undefined, "worker replay must not promote a saved preview to design acceptance");
        await stage("Identify");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-record-validation]').hidden);"), "false");
        fs.writeFileSync(path.join(captures, "worker-preview-reopened.json"), JSON.stringify(result, null, 2));
        await capture("03-worker-preview-restored");
        return;
      }
      if (phase === "preview") {
        assert.strictEqual(result.model, undefined);
        await stage("Identify");
        await inner("d.querySelector('[data-identify-record-validation]').click();return 'validation';");
        await validateImportedStepWithNativeTest(inner, capture, captures);
        await stage("Validate");
        await inner("d.querySelector('[data-result-save]').click();return 'save accepted';");
        await pollValue("accepted revision saved", () => inner("return d.querySelector('[data-result-state]').textContent;"), v => /Saved revision 2/.test(v), 10000);
        result = await openLatest(2);
        await capture("14-accepted-result-saved");
      }
      assert.strictEqual(result.identificationStudy.phase, "accepted");
      assert.notStrictEqual(result.identificationStudy.estimation.estimationRecordingId, result.identificationStudy.current.estimationRecordingId);
      assert.ok(result.identificationStudy.current.recording.data.rawSamples.length >= 451);
      assert.strictEqual(result.identificationStudy.current.recording.data.acquisition.completeness, "complete");
      assert.strictEqual(result.model.gain, result.modelFit.stepResponse.best.gain);
      assert.deepStrictEqual(result.identificationStudy.current.recording.data.rawSamples, result.recording.rawSamples);
      fs.writeFileSync(path.join(captures, "reopened-identification-result.json"), JSON.stringify(result, null, 2));
      if (phase === "accepted") {
        await stage("Identify");
        await capture("03-accepted-after-process-restart");
        assert.match(await inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), /Historical/);
      }
      if (phase === "invalidate") {
        await stage("Identify");
        await inner("var f=d.querySelector('[data-step-field=" + JSON.stringify("delay.minimum") + "]');f.value='0.8';f.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-identify-run]').click();return 'check changed settings';");
        await pollValue("changed inputs qualified", () => inner("return d.querySelector('[data-identify-state]').textContent;"), v => /Estimation recording qualified/.test(v), 10000);
        await capture("03-inputs-changed");
        assert.strictEqual(await inner("return String(d.querySelector('[data-tune-generate]').disabled);"), "true",
          "Check must invalidate Tune immediately when the accepted model's inputs change");
        await stage("Validate");
        await inner("d.querySelector('[data-result-save]').click();return 'save invalidated';");
        await pollValue("invalidated revision saved", () => inner("return d.querySelector('[data-result-state]').textContent;"), v => /Saved revision 3/.test(v), 10000);
        result = await openLatest(3);
        await assertInvalidated(result);
        await capture("04-invalidated-saved");
        fs.writeFileSync(path.join(captures, "invalidated-identification-result.json"), JSON.stringify(result, null, 2));
      }
    } finally {
      if (started) await runtimeLifecycleService.stopRuntime();
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
