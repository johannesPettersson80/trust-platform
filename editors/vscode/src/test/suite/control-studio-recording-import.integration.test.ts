import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { ControlStudioWorkflowResult } from "../../controlStudio/workflowResult";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { validateImportedStepWithNativeTest } from "./control-studio-step-validation-harness";
import {
  captureRenderedWorkbenchCdp,
  connectCdp,
  devToolsPort,
  evaluateInWebview,
  httpJson,
  pollValue,
  requiredEnvironment,
  sleep,
} from "./plant-rendered-harness";

suite("Control Studio recording import", () => {
  test("imports a preserved native recording for identification without replacing the active Lesson 15 loop", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_RECORDING_IMPORT !== "1") this.skip();
    this.timeout(120_000);

    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);

    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8")) as {
      id: string; projectId: string; runtimeId: string;
    };
    assert.strictEqual(loop.id, "control-lesson-23", "the active project must remain the approved Lesson 15 source");
    const importedPath = requiredEnvironment("TRUST_IDENTIFICATION_RECORDING_FILE");
    const imported = JSON.parse(fs.readFileSync(importedPath, "utf8")) as ControlStudioWorkflowResult;
    assert.strictEqual(imported.recording.rawSamples.length, 303, "the preserved Lesson 14 source must contain all 303 native rows");

    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let panelOpened = false;

    try {
      const start = await runtimeLifecycleService.startLocalSimulator();
      fs.writeFileSync(path.join(captures, "runtime-start.json"), JSON.stringify(start, null, 2));
      assert.strictEqual(start.ok, true, JSON.stringify(start));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
        schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
      });
      panelOpened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");

      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = async (name: string) => {
        await inner(`const b=[...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.getAttribute('aria-label')?.startsWith(${JSON.stringify(name + ',')}));b.click();return 'stage';`);
      };
      const capture = async (name: string) => {
        if (process.env.TRUST_IDENTIFICATION_CANCEL_ONLY === "1" && name !== "03c-fit-cancelled") return;
        await sleep(250);
        await captureRenderedWorkbenchCdp(connection, path.join(captures, `recording-import-${name}.png`));
      };
      const upload = async (id: string, file: string) => {
        await connection.send("DOM.enable", {}, session);
        await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
        const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
        const node = tree.nodes.find((candidate: any) => candidate.nodeName === "INPUT" && candidate.attributes?.includes(id));
        assert.ok(node, `file input ${id} must exist in the rendered Identify surface`);
        await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session);
        await pollValue(`${id} selected`, () => inner(`return String(d.querySelector('#${id}')?.files?.length??0);`), value => value === "1", 10_000);
      };

      await pollValue("shared workflow handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await stage("Identify");
      const importButton = JSON.parse(await inner("const b=d.querySelector('[data-identify-import]');const r=b?.getBoundingClientRect();return JSON.stringify({exists:Boolean(b),visible:Boolean(b&&r&&r.width&&r.height&&!b.hidden),label:b?.textContent??''});"));
      assert.strictEqual(importButton.exists, true, "Identify must expose the public recording import control");
      assert.strictEqual(importButton.visible, true, "the public recording import control must be reachable");
      assert.match(importButton.label, /Import recording/i);
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-import]').classList.contains('primary'));"), "true",
        "Import is the available first action when no recording exists");
      const initialIdentifyStatus = await inner("return d.querySelector('[data-identify-state]').textContent.trim();");
      await capture("01-import-ready");

      await upload("identify-import-file", importedPath);
      await pollValue("imported recording source", () => inner("return d.querySelector('[data-identify-recording-source]')?.textContent??'';"),
        value => /Imported/.test(value) && /Simulation/.test(value) && /303/.test(value), 15_000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-test-review]')?.disabled??false);"), "false",
        "importing a recording must not create or require a new Test");
      assert.strictEqual(await inner("return String(d.querySelector('[data-plot-tab=recorded-response]')?.getAttribute('aria-selected')==='true');"), "true",
        "the imported measured response must be visible before fitting");
      assert.match(await inner("return d.querySelector('[data-control-studio-freshness]').textContent;"), /Historical/);
      const importedConnection = await inner("return d.querySelector('[data-control-studio-connection]').textContent.trim();");
      await capture("02-imported-recording");
      await inner("d.querySelector('[data-identify-recording-details] > summary').click();return 'recording source';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-identify-recording-details]').closest('.summary-panel')));"), "true",
        "source details must use the summary width rather than the narrow parameter column");
      const provenanceText = await inner("return d.querySelector('[data-identify-recording-details]').textContent;");
      assert.ok(provenanceText.includes(imported.projectId) && provenanceText.includes(imported.loop.id)
        && provenanceText.includes(imported.resultId) && provenanceText.includes(imported.loop.revision));
      await capture("02a-recording-provenance");
      await inner("d.querySelector('[data-identify-recording-details] > summary').click();return 'close source';");

      if (process.env.TRUST_IDENTIFICATION_BOUNDED_STEP === "1") {
        await inner("var root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',e=>{var m=e.data;if(m?.kind==='control-studio.engineering.result'&&m.stage==='identify'&&m.payload?.phase!=='qualified')root.dataset.stepReview=JSON.stringify(m.payload);});return 'observe step review';");
        assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-step-response-enabled]')));"), "true",
          "Identify must expose measured step-response analysis on the actual imported recording");
        const toggle = JSON.parse(await inner("var b=d.querySelector('[data-step-response-enabled]');var r=b.getBoundingClientRect();var t=d.createRange();t.selectNodeContents(b.closest('label').lastChild);var q=t.getBoundingClientRect();return JSON.stringify({width:r.width,delta:Math.abs((r.top+r.bottom-q.top-q.bottom)/2)});"));
        assert.ok(toggle.width <= 24 && toggle.delta < 5, "the step-analysis checkbox and its label must form one aligned compact control");
        await inner("d.querySelector('[data-step-response-enabled]').click();d.querySelector('[data-step-response-settings] > summary').click();return 'step analysis';");
        assert.strictEqual(await inner("return d.querySelector('#identify-model').value;"), "fopdt");
        await capture("02b-measurement-settings");
        for (const field of ["maximumWindowSlopePerSecond", "gainStep", "timeConstant.step"]) {
          await inner(`d.querySelector('[data-step-field="${field}"]').scrollIntoView({block:'center'});return 'field';`);
          await capture(`02c-setting-${field}`);
        }
        await inner("var b=d.querySelector('[data-step-field=measurementBound]');b.value='0';b.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-identify-run]').click();return 'invalid bound';");
        assert.match(await inner("return d.querySelector('[data-identify-state]').textContent;"), /Review.*measurement/);
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-fit]').hidden);"), "true");
        const form = JSON.parse(await inner("var f=d.querySelector('[data-step-field=measurementBound]');var p=d.getElementById(f.getAttribute('aria-describedby'));var a=p?.getBoundingClientRect();var b=f.closest('.workspace').getBoundingClientRect();return JSON.stringify({errorVisible:Boolean(a&&a.top>=b.top&&a.bottom<=b.bottom),fieldInvalid:f.getAttribute('aria-invalid')==='true',errorText:p?.textContent??'',exclusionHidden:d.querySelector('#identify-exclude-start').closest('details').hidden});"));
        fs.writeFileSync(path.join(captures, "invalid-bound-visible.json"), JSON.stringify(form, null, 2));
        await capture("02d-invalid-bound");
        assert.strictEqual(form.errorVisible, true, "the field's linked inline error must be visible");
        assert.strictEqual(form.fieldInvalid, true);
        assert.match(form.errorText, /greater than zero/);
        assert.strictEqual(form.exclusionHidden, true);
        await inner("var b=d.querySelector('[data-step-field=measurementBound]');b.value='.05';b.dispatchEvent(new Event('input',{bubbles:true}));return 'restore bound';");
        await inner("d.querySelector('[data-step-response-settings] > summary').click();return 'close settings';");
      }

      await inner("d.querySelector('[data-identify-run]').click();return 'qualify';");
      const qualified = await pollValue("imported qualification", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
        value => /Estimation recording qualified/.test(value), 15_000);
      assert.match(qualified, /Estimation recording qualified/);
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-workflow-pane=Identify] button.primary')].filter(button=>!button.hidden&&!button.disabled).length);"), "1",
        "a qualified recording has one primary next action");
      await capture("03-qualified");
      if (process.env.TRUST_IDENTIFICATION_BOUNDED_STEP === "1") {
        await inner("d.querySelector('[data-step-response-evidence] > summary').click();return 'window measurements';");
        assert.match(await inner("return d.querySelector('[data-step-response-report]').textContent;"), /Baseline.*35.*Final/s);
        await inner("d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'measurements visible';");
        await capture("03b-window-measurements");
      }

      if (process.env.TRUST_IDENTIFICATION_CANCEL_FIT === "1") {
        await inner("d.querySelector('[data-identify-fit]').scrollIntoView({block:'center'});return 'fit visible';");
        await inner("d.querySelector('[data-identify-fit]').click();var c=d.querySelector('[data-identify-cancel]');if(!c||c.hidden)throw new Error('Cancel must be visible while Fit is pending');c.click();return 'cancel requested';");
        const cancelled = await pollValue("cancelled fit retains qualified inputs", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
          value => /Calculation cancelled/.test(value), 10000);
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-cancel]').hidden);"), "true");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-fit]').hidden);"), "false");
        fs.writeFileSync(path.join(captures, "cancelled-fit.json"), JSON.stringify({ status: cancelled, originalSampleCount: imported.recording.rawSamples.length }, null, 2));
        await capture("03c-fit-cancelled");
        if (process.env.TRUST_IDENTIFICATION_CANCEL_ONLY === "1") return;
      }
      await inner("d.querySelector('[data-identify-fit]').click();return 'fit';");
      const fit = await pollValue("imported fit preview", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
        value => /Preview only/.test(value), 15_000);
      assert.match(fit, /Preview only/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-plot-tab=fit]')?.getAttribute('aria-selected')==='true');"), "true",
        "Fit must show the new measured-versus-predicted response");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-accept]')?.hidden??true);"), "true",
        "imported identification remains preview-only and cannot silently accept a model");
      assert.strictEqual(await inner("return String(d.querySelector('[data-identify-record-validation]')?.classList.contains('primary'));"), "true");
      await capture("04-fit-preview");
      if (process.env.TRUST_IDENTIFICATION_BOUNDED_STEP === "1") {
        const result = await inner("return d.querySelector('[data-step-response-report]').textContent;");
        assert.match(result, /Consistent grid/);
        assert.match(result, /Detectable onset/);
        assert.match(result, /Transport delay/);
        await inner("d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'grid visible';");
        await capture("04b-bounded-fit");
        await inner("d.querySelector('[data-step-response-settings] > summary').click();return 'refine settings';");
        const refined = { "delay.minimum": .7, "delay.maximum": 1.3, "delay.step": .05,
          "timeConstant.minimum": 4.4, "timeConstant.maximum": 5.5, "timeConstant.step": .05, "baselineStep": .005, "gainStep": .0005 };
        for (const [field, value] of Object.entries(refined)) await inner(`var f=d.querySelector('[data-step-field="${field}"]');f.value=${JSON.stringify(String(value))};f.dispatchEvent(new Event('input',{bubbles:true}));return 'refined';`);
        await inner("d.querySelector('[data-step-response-settings] > summary').click();d.querySelector('[data-identify-run]').click();return 'check refined settings';");
        await pollValue("refined recording check", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Estimation recording qualified/.test(value), 10_000);
        await inner("d.querySelector('[data-identify-fit]').click();return 'refined fit';");
        await pollValue("refined independent grid", () => inner("return d.querySelector('[data-step-response-report]').textContent;"), value => /11274 candidates/.test(value), 10_000);
        await inner("d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'refined grid visible';");
        await capture("04c-refined-grid");
      }
      await inner("d.querySelector('[data-identify-record-validation]').click();return 'review validation test';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-workflow-pane=Test]').hidden);"), "false");
      assert.ok(!/Test running/.test(await inner("return d.querySelector('[data-test-state]').textContent;")), "navigation must not start acquisition");
      if (process.env.TRUST_IDENTIFICATION_NATIVE_VALIDATION === "1") {
        await validateImportedStepWithNativeTest(inner, capture, captures);
        return;
      }
      await stage("Identify");

      await inner("const root=d.querySelector('[data-control-studio]');d.defaultView.addEventListener('message',e=>{const m=e.data;if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened'&&m.loopId===root.dataset.loopId)root.dataset.importOpened=JSON.stringify(m.payload?.result);});return 'observe';");
      await stage("Validate");
      assert.strictEqual(await inner("return String(d.querySelector('[data-result-save]')?.disabled??true);"), "false",
        "the imported recording must be saveable from Validate");
      await inner("d.querySelector('[data-result-save]').click();return 'save';");
      await pollValue("imported result saved", () => inner("return d.querySelector('[data-result-state]').textContent;"),
        value => /Saved revision/.test(value), 15_000);
      await capture("05-saved");

      if (await inner("return String(d.querySelector('[data-result-history]').closest('details').open);") !== "true") {
        await inner("d.querySelector('[data-workflow-pane=Validate] details:has([data-result-history]) > summary').click();return 'history';");
      }
      const revision = await inner("return [...d.querySelector('[data-result-history]').options].find(option=>option.value)?.value??'';");
      assert.ok(revision, "Save must create a selectable retained revision");
      await inner(`const h=d.querySelector('[data-result-history]');h.value=${JSON.stringify(revision)};h.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-result-open]').click();return 'open';`);
      const opened = JSON.parse(await pollValue("opened imported result", () => inner("return d.querySelector('[data-control-studio]')?.dataset.importOpened??'';"),
        value => value.length > 0, 15_000)) as ControlStudioWorkflowResult;
      assert.strictEqual(opened.projectId, loop.projectId, "the active Lesson 15 loop owns the saved result");
      assert.strictEqual(opened.loop.id, loop.id);
      if (process.env.TRUST_IDENTIFICATION_PERSISTENCE === "1") {
        assert.strictEqual(opened.identificationStudy?.phase, "preview", "Save/Open must retain the unaccepted identification preview");
        assert.strictEqual(opened.identificationStudy?.estimation?.recording?.data.rawSamples.length, 303);
        assert.strictEqual(opened.identificationStudy?.current.stepResponseSettings?.delay.minimum, .7);
        assert.strictEqual(opened.model, undefined, "a saved preview cannot become an accepted model");
      }
      assert.strictEqual(opened.recording.rawSamples.length, imported.recording.rawSamples.length);
      assert.deepStrictEqual(opened.recording.rawSamples, imported.recording.rawSamples, "Save/Open must preserve all imported samples");
      const origin = imported.recording.provenance ?? imported;
      assert.deepStrictEqual(opened.recording.provenance?.projectId, origin.projectId);
      assert.deepStrictEqual(opened.recording.provenance?.loop, origin.loop);
      assert.deepStrictEqual(opened.recording.provenance?.resultId, origin.resultId);
      assert.deepStrictEqual(opened.recording.provenance?.resultRevision, origin.resultRevision);
      assert.strictEqual(opened.candidate, undefined, "recording import must not create a controller candidate");
      assert.deepStrictEqual({ initialIdentifyStatus, importedConnection,
        savedConnection: await inner("return d.querySelector('[data-control-studio-connection]').textContent.trim();") },
      { initialIdentifyStatus: "", importedConnection: "", savedConnection: "" },
      "initial guidance belongs in the empty plot, and historical identity has one badge rather than duplicate connection text");
      await capture("06-opened");
      await stage("Monitor");
      assert.match(await pollValue("current native live source", () => inner("return d.querySelector('[data-control-studio-freshness]')?.textContent??'';"), value => /Live/.test(value), 10000), /Live/,
        "returning to Monitor must use the active Lesson 15 runtime again");
      assert.match(await inner("return d.querySelector('[data-control-studio-pv]').textContent;"), /^35(?:\.0*)? °C$/);
      assert.match(await inner("return d.querySelector('[data-control-studio-mv]').textContent;"), /^75(?:\.0*)? %$/);
      fs.writeFileSync(path.join(captures, "opened-imported-result.json"), JSON.stringify(opened, null, 2));
      await capture("07-live-after-open");
    } finally {
      await runtimeLifecycleService.stopRuntime();
      if (panelOpened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
