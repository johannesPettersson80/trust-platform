import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Control Studio model structure learner handoff", () => {
  test("imports original estimation, edits the comparison settings and retains Candidate A through Fit and Save/Open", async function () {
    if (process.env.TRUST_MODEL_STRUCTURE_UI !== "1") this.skip();
    this.timeout(120000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("workbench").update("colorTheme", "Default Dark Modern", vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual(loop.id, "control-lesson-24");
    const file = requiredEnvironment("TRUST_IDENTIFICATION_RECORDING_FILE");
    const source = JSON.parse(fs.readFileSync(file, "utf8"));
    assert.strictEqual(source.identificationStudy.phase, "accepted");
    assert.strictEqual(source.identificationStudy.estimation.recording.data.rawSamples.length, 303);
    assert.ok(source.recording.rawSamples.length > 400, "the source must actually display a different validation run");
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop }); opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const click = (selector: string) => inner(`d.querySelector(${JSON.stringify(selector)}).click();return 'clicked';`);
      const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
      const status = () => inner("return d.querySelector('[data-identify-state]').textContent;");
      const capture = async (name: string) => { await sleep(250); await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`)); };
      const field = (key: string) => inner(`return d.querySelector('[data-step-field="${key}"]').value;`);
      const setField = (key: string, value: string) => inner(`var f=d.querySelector('[data-step-field="${key}"]');f.value=${JSON.stringify(value)};f.dispatchEvent(new Event('input',{bubbles:true}));return 'edited';`);
      await pollValue("handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), v => v === "true", 10000);
      await stage("Identify");
      await inner("d.defaultView.addEventListener('message',e=>{var m=e.data;var r=d.querySelector('[data-control-studio]');if(m?.kind==='control-studio.engineering.result'&&m.stage==='identify'&&m.payload?.fit)r.dataset.structureReview=JSON.stringify(m.payload);if(m?.kind==='control-studio.workflow-result'&&m.phase==='opened')r.dataset.structureOpened=JSON.stringify(m.payload.result);});return 'observe';");
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes("identify-import-file"));
      assert.ok(input);
      await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: input.backendNodeId }, session);
      await pollValue("original estimation imported", status, v => /Original estimation imported/.test(v), 10000);
      assert.match(await inner("return d.querySelector('[data-identify-recording-source]').textContent;"), /303 samples/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-step-response-enabled]').checked);"), "true");
      assert.strictEqual(await field("delay.minimum"), String(source.identificationStudy.estimation.stepResponseSettings.delay.minimum));
      await capture("01-original-estimation");
      await inner("var m=d.querySelector('#identify-model');if(m.disabled)throw new Error('Model structure is disabled');m.value='sopdt';m.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-step-response-settings]').open=true;return 'comparison';");
      await capture("01b-select-comparison");
      await inner("d.querySelector('[data-step-field=\"sensorTimeConstant.minimum\"]').scrollIntoView({block:'center'});return 'settings';");
      await capture("02-comparison-settings");
      await setField("sensorTimeConstant.minimum", ""); await click("[data-identify-run]");
      assert.strictEqual(await inner("return String(d.activeElement===d.querySelector('[data-step-field=\"sensorTimeConstant.minimum\"]')&&d.activeElement.getAttribute('aria-invalid')==='true');"), "true");
      await capture("03-invalid-additional-time");
      const lessonProcedure = process.env.TRUST_MODEL_STRUCTURE_LESSON === "1";
      const edits = lessonProcedure
        ? { "sensorTimeConstant.minimum": ".1", "sensorTimeConstant.maximum": "10", "sensorTimeConstant.step": ".1",
          "modelAdequacy.maximumAbsoluteResidual": ".2", "modelAdequacy.maximumFinalError": ".1" }
        : { "sensorTimeConstant.minimum": ".2", "sensorTimeConstant.maximum": "8", "sensorTimeConstant.step": ".2",
          "modelAdequacy.maximumAbsoluteResidual": ".3", "modelAdequacy.maximumFinalError": ".15" };
      for (const [key, value] of Object.entries(edits)) await setField(key, value);
      await click("[data-identify-run]");
      await pollValue("comparison qualified", status, v => /Estimation recording qualified/.test(v), 10000);
      await inner("d.querySelector('[data-step-response-settings]').open=false;return 'collapse';");
      await click("[data-identify-fit]");
      const review = JSON.parse(await pollValue("comparison fit", () => inner("return d.querySelector('[data-control-studio]').dataset.structureReview??'';"), v => Boolean(v), 20000));
      assert.deepStrictEqual(review.fit.modelStructure.candidateA.analysis.best, source.modelFit.stepResponse.best,
        "the original first-order fit must survive the public import and B fitting");
      assert.deepStrictEqual(review.fit.modelStructure.candidateA.analysis.consistency, source.modelFit.stepResponse.consistency);
      assert.deepStrictEqual(review.fit.modelStructure.candidateB.analysis.settings.sensorTimeConstant,
        { minimum: Number(edits["sensorTimeConstant.minimum"]), maximum: Number(edits["sensorTimeConstant.maximum"]), step: Number(edits["sensorTimeConstant.step"]) });
      assert.deepStrictEqual(review.fit.modelStructure.requirements,
        { maximumAbsoluteResidual: Number(edits["modelAdequacy.maximumAbsoluteResidual"]), maximumFinalError: Number(edits["modelAdequacy.maximumFinalError"]) });
      assert.strictEqual(review.fit.modelStructure.physicalOrigin, "unresolved");
      assert.strictEqual(review.phase, "validation_required");
      fs.writeFileSync(path.join(captures, "comparison-review.json"), JSON.stringify(review, null, 2));
      if (lessonProcedure) {
        for (const plot of ["fit", "residuals"]) {
          await click(`[data-plot-tab="${plot}"]`);
          await capture(`lesson-${plot}`);
        }
        await inner("d.querySelector('[data-model-structure] details').open=true;d.querySelector('[data-model-structure] details').scrollIntoView({block:'start'});return 'parameters';");
        await capture("lesson-parameters");
        await inner("d.querySelector('[data-model-structure] details tbody tr:last-child').scrollIntoView({block:'end'});return 'uncertainty';");
        await capture("lesson-uncertainty");
        assert.strictEqual(review.fit.modelStructure.recommended, "first_order",
          "the recorded first-order heater does not justify the illustrative extra lag");
        await inner("d.querySelector('[data-model-structure] details').open=false;d.querySelector('[data-structure-choice]').scrollIntoView({block:'center'});return 'decision';");
        await click('[data-structure-choice="first_order"]');
        await pollValue("provisional choice", status, v => /retained provisionally/.test(v), 10000);
        await capture("lesson-provisional-choice");
        await click("[data-structure-save]");
        await stage("Validate");
      } else {
        await stage("Validate"); await click("[data-result-save]");
      }
      await pollValue("comparison saved", () => inner("return d.querySelector('[data-result-state]').textContent;"), v => /Saved revision 1/.test(v), 10000);
      await inner("d.querySelector('[data-result-history]').closest('details').open=true;var h=d.querySelector('[data-result-history]');h.value=[...h.options].find(o=>o.value)?.value;h.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-result-open]').click();return 'open';");
      const saved = JSON.parse(await pollValue("comparison opened", () => inner("return d.querySelector('[data-control-studio]').dataset.structureOpened??'';"), v => Boolean(v), 20000));
      assert.strictEqual(saved.projectId, loop.projectId);
      assert.strictEqual(saved.identificationStudy.current.requestedKind, "sopdt");
      assert.deepStrictEqual(saved.identificationStudy.current.firstOrderStepResponseSettings, source.identificationStudy.estimation.stepResponseSettings);
      assert.deepStrictEqual(saved.recording.rawSamples, source.identificationStudy.estimation.recording.data.rawSamples);
      assert.strictEqual(saved.model, undefined); assert.strictEqual(saved.candidate, undefined);
      if (lessonProcedure) assert.strictEqual(saved.identificationStudy.structureChoice, "first_order");
      fs.writeFileSync(path.join(captures, "saved-comparison.json"), JSON.stringify(saved, null, 2));
      await stage("Identify");
      await inner("d.querySelector('[data-step-response-settings]').open=true;d.querySelector('[data-step-field=\"sensorTimeConstant.minimum\"]').scrollIntoView({block:'center'});return 'restored settings';");
      for (const [key, value] of Object.entries(edits)) assert.strictEqual(Number(await field(key)), Number(value));
      await capture("04-restored-comparison-settings");
      for (const [name, theme, kind] of [["light", "Default Light Modern", "vscode-light"],
        ["contrast", "Default High Contrast", "vscode-high-contrast"]]) {
        await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
        await pollValue("comparison theme", () => inner(`return String(d.body.classList.contains(${JSON.stringify(kind)}));`), value => value === "true", 10000);
        await inner("d.querySelector('[data-step-field=\"sensorTimeConstant.minimum\"]').scrollIntoView({block:'center'});return 'settings';");
        await capture(`05-${name}-comparison-settings`);
      }
    } finally {
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
