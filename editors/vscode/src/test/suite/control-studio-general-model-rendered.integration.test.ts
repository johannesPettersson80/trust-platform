import { revealControlForCapture } from "./control-studio-capture-visibility";
import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep, type CdpConnection } from "./plant-rendered-harness";
suite("Rendered supplied model workflow", () => {
  test("imports a rectangular model file and analyzes its channels without a live PID", async function () {
    if (process.env.TRUST_PLANT_RENDERED_TEST !== "1")
      this.skip();
    this.timeout(60000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME;
    if (theme)
      await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const request = loopRequest();
    request.loopDefinition.controller.type = "EngineerLaw";
    (request.loopDefinition.controller as any).design = "project";
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
    await vscode.commands.executeCommand("workbench.action.closeSidebar");
    await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "provided-model-"));
    const file = path.join(folder, "model.json");
    const malformed = path.join(folder, "malformed.json");
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -10, maximum: 10 } });
    fs.writeFileSync(file, JSON.stringify({ kind: "continuous-state-space", revision: "engineer-rectangular", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0,
      inputs: [channel("heater", "W"), channel("fan", "V")], outputs: [channel("temperature", "degC")], states: [channel("thermal", "degC")], a: [[-2]], b: [[1, 3]], c: [[2]], d: [[0, 1]] }));
    try {
      const session = await attachControlStudio(connection, port, "", request.scope.projectId);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("model file handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      assert.strictEqual(await inner("return String(['inspect','zoom-in','zoom-out','reset','expand','export-figure'].every(action=>{const button=d.querySelector('[data-chart-action=\"'+action+'\"]');return !button||button.disabled||button.getBoundingClientRect().height===0;}));"), "true", "empty Monitor cannot offer sample-dependent plot tools");
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'clicked';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('#general-model-file')));"), "true", "Import model JSON control is available for arbitrary custom ST");
      assert.deepStrictEqual(JSON.parse(await inner("return JSON.stringify(['[data-general-model-analyze]','[data-general-model-cancel]','[data-general-model-save]'].map(s=>{var b=d.querySelector(s);return {className:b.className,disabled:b.disabled};}));")), [
        { className: "trust-button trust-button--primary", disabled: true },
        { className: "trust-button", disabled: true },
        { className: "trust-button", disabled: true },
      ], "supplied-model actions must use shared Control Studio buttons");
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && Array.isArray(n.attributes) && n.attributes.includes("general-model-file"));
      assert.ok(node, "real file input must be reachable");
      fs.writeFileSync(malformed, "{\"kind\":");
      await connection.send("DOM.setFileInputFiles", { files: [malformed], backendNodeId: node.backendNodeId }, session);
      const malformedStatus = await pollValue("malformed supplied model", () => inner("return d.querySelector('[data-general-model-status]').textContent;"), s => /invalid|unexpected|JSON/i.test(s), 10000);
      assert.match(malformedStatus, /invalid|unexpected|JSON/i);
      const emptyPlotBounds=JSON.parse(await inner("const plot=d.querySelector('.plot-column').getBoundingClientRect();const monitor=d.querySelector('.monitor').getBoundingClientRect();return JSON.stringify({plotBottom:plot.bottom,monitorBottom:monitor.bottom});"));
      assert.ok(emptyPlotBounds.plotBottom<=emptyPlotBounds.monitorBottom+1, `an unavailable plot must not overflow into model controls: ${JSON.stringify(emptyPlotBounds)}`);
      const malformedControls = JSON.parse(await inner("return JSON.stringify(['[data-general-model-analyze]','[data-general-model-save]'].map(s=>d.querySelector(s).disabled));"));
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) { await sleep(500); await revealControlForCapture(inner, "[data-general-model-status]"); await sleep(300); await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-invalid-model.png")); }
      await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session);
      await pollValue("imported model", () => inner("return d.querySelector('[data-general-model-status]').textContent+' '+d.querySelector('[data-general-model-provenance]').textContent;"), s => s.includes("engineer-rectangular"), 10000);
      await inner("d.defaultView.addEventListener('trust-control-studio-general-model',function cancelOnce(e){if(e.detail.status==='running'){d.defaultView.removeEventListener('trust-control-studio-general-model',cancelOnce);d.querySelector('[data-general-model-cancel]').click();}});d.querySelector('[data-general-model-analyze]').click();return 'cancel active analysis';");
      await pollValue("cancelled supplied analysis", () => inner("return d.querySelector('[data-general-model-status]').textContent;"), s => /cancelled/i.test(s), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-general-model-save]').disabled);"), "true");
      if (screenshot) { await sleep(500); await revealControlForCapture(inner, "[data-general-model-status]"); await sleep(300); await captureRenderedWorkbenchCdp(connection, screenshot.replace(/\.png$/, "-cancelled-model.png")); }
      await inner("d.querySelector('#general-model-min').value='2';d.querySelector('#general-model-max').value='20';d.querySelector('#general-model-points').value='3';d.querySelector('[data-general-model-analyze]').click();return 'started';");
      const result = await pollValue("calculated supplied model", () => inner("return d.querySelector('[data-general-model-status]').textContent+' '+d.querySelector('[data-general-model-result]').textContent;"), s => s.includes("Model analysis complete") && s.includes("unvalidated") && s.includes("-2"), 10000);
      assert.match(result, /not closed-loop stability/i);
      assert.strictEqual(await inner("return String(d.querySelector('[data-tune-generate]').disabled);"), "true",
        "frequency analysis of an unvalidated rectangular model does not create a tunable candidate for the bound loop");
      assert.strictEqual(await inner("return String(d.querySelector('[data-general-model-channel]').options.length);"), "2");
      const responseCollapsed = await inner("return String(d.querySelector('[data-general-model-table]').closest('details').open);");
      await inner("d.querySelector('[data-general-model-table]').closest('details').querySelector(':scope > summary').click();return 'opened';");
      const responseExpanded = await inner("return String(d.querySelector('[data-general-model-table]').closest('details').open);");
      assert.match(await inner("return d.querySelector('[data-general-model-table]').textContent;"), /0.5/);
      await inner("var c=d.querySelector('[data-general-model-channel]');c.value='1';c.dispatchEvent(new Event('change'));return 'selected';");
      assert.match(await inner("return d.querySelector('[data-general-model-table]').textContent;"), /2.5/);
      assert.match(await inner("return d.querySelector('[data-general-model-channel]').selectedOptions[0].textContent;"), /fan.*V.*temperature.*degC/);
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-plot-view=general-model] canvas').length);"), "2");
      const gridState = await inner("var grid=d.querySelector('#general-model-points');var state={grid:Boolean(grid),min:Boolean(d.querySelector('#general-model-min')),max:Boolean(d.querySelector('#general-model-max')),tune:Boolean(Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune'&&b.getAttribute('aria-selected')==='true'))};if(grid){grid.value='4';grid.dispatchEvent(new Event('input'));}return JSON.stringify(state);");
      assert.strictEqual(gridState, '{"grid":true,"min":true,"max":true,"tune":true}', `supplied model grid controls disappeared: ${gridState}`);
      await pollValue("edited supplied model invalidation", () => inner("return d.querySelector('[data-general-model-status]').textContent+'|'+String(d.querySelector('[data-general-model-save]').disabled);"), s => s.includes("Calculate again") && s.endsWith("|true"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-general-model-result]').hidden);"), "true", "editing the analysis grid must hide stale supplied-model results");
      await inner("d.querySelector('[data-general-model-analyze]').click();return 'reanalyzing';");
      await pollValue("reanalyzed supplied model", () => inner("return d.querySelector('[data-general-model-status]').textContent+'|'+String(d.querySelector('[data-general-model-save]').disabled);"), s => s.includes("Model analysis complete") && s.endsWith("|false"), 10000);
      await inner("d.querySelector('[data-general-model-save]').click();return 'saving';");
      await pollValue("saved supplied model", () => inner("return d.querySelector('[data-general-model-status]').textContent+' | '+d.querySelector('[data-result-state]').textContent;"), s => s.includes("Saved revision"), 10000);
      await inner("d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result'&&e.data?.payload?.result)d.defaultView.__savedEvidence=e.data.payload.result.evidence.model;});var h=d.querySelector('[data-result-history]');h.value='1';h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'opened';");
      const savedEvidence = await pollValue("saved model evidence", () => inner("return d.defaultView.__savedEvidence||'';"), s=>s.length>0, 10000);
      assert.strictEqual(savedEvidence, "Model-predicted");
      const previous = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      const restored = await attachControlStudio(connection, port, previous, request.scope.projectId);
      await pollValue("restored editor handlers", () => evaluateInWebview(connection, restored,
        "return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await evaluateInWebview(connection, restored,
        "Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'restored Tune';");
      await sleep(300);
      assert.strictEqual(await evaluateInWebview(connection, restored, "return [d.querySelector('#general-model-min').value,d.querySelector('#general-model-max').value,d.querySelector('#general-model-points').value].join(',');"), "2,20,4");
      const restoredAnalysis = await evaluateInWebview(connection, restored,
        "return JSON.stringify({status:d.querySelector('[data-general-model-status]').textContent,hidden:d.querySelector('[data-general-model-result]').hidden,saveDisabled:d.querySelector('[data-general-model-save]').disabled});");
      assert.deepStrictEqual(JSON.parse(restoredAnalysis), { status: "Model analysis complete. Prediction only; no controller was applied.", hidden: false, saveDisabled: false },
        "reopening a result must retain its saved general model analysis when the engineer study also has a selected model");
      await evaluateInWebview(connection, restored, "d.querySelector('[data-general-model-save]').click();return 'saving restored analysis';");
      await pollValue("saved restored supplied model", () => evaluateInWebview(connection, restored,
        "return d.querySelector('[data-general-model-status]').textContent+' | '+d.querySelector('[data-result-state]').textContent;"),
        s => s.includes("Saved revision"), 10000);
      await evaluateInWebview(connection, restored,
        "d.defaultView.addEventListener('message',e=>{if(e.data?.kind==='control-studio.workflow-result'&&e.data.phase==='opened')d.defaultView.__reopenedAnalysis=e.data.payload.result;});var h=d.querySelector('[data-result-history]');h.value=[...h.options].find(o=>o.textContent.includes('Revision 2')).value;h.dispatchEvent(new Event('change'));d.querySelector('[data-result-open]').click();return 'reopened restored analysis';");
      await pollValue("reopened restored result", () => evaluateInWebview(connection, restored,
        "return d.querySelector('[data-result-state]').textContent;"), s => /Opened|Restored/.test(s), 10000);
      const savedAgain = JSON.parse(await pollValue("second saved result", () => evaluateInWebview(connection, restored,
        "return JSON.stringify(d.defaultView.__reopenedAnalysis??null);"), value => value !== "null", 10000));
      assert.strictEqual(savedAgain.resultRevision, 2);
      assert.ok(savedAgain.generalModelAnalysis, "saving a restored model must retain its computed frequency analysis");
      const reopenedAnalysis = await evaluateInWebview(connection, restored,
        "return JSON.stringify({status:d.querySelector('[data-general-model-status]').textContent,hidden:d.querySelector('[data-general-model-result]').hidden,saveDisabled:d.querySelector('[data-general-model-save]').disabled});");
      assert.deepStrictEqual(JSON.parse(reopenedAnalysis), { status: "Model analysis complete. Prediction only; no controller was applied.", hidden: false, saveDisabled: false },
        "a second Save/Open must preserve the general model analysis after restoration");
      await evaluateInWebview(connection, restored, "d.querySelector('[data-general-model-result]').scrollIntoView();return 'visible';");
      await pollValue("visible supplied plots", () => evaluateInWebview(connection, restored,
        "Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();var r=d.querySelector('[data-general-model-result]');r.scrollIntoView({block:'start'});var box=r.getBoundingClientRect();return String(box.height>0&&box.top>=0&&box.top<d.defaultView.innerHeight);"), s=>s==="true", 10000);
      await evaluateInWebview(connection, restored, "d.querySelector('[data-plot-tab=general-model]').click();return 'model response';");
      assert.strictEqual(await evaluateInWebview(connection, restored,
        "return String(!d.querySelector('[data-plot-view=general-model]').hidden);"), "true",
        "the restored-model capture must show its actual frequency response");
      assert.deepStrictEqual(malformedControls, [true, true], "malformed supplied JSON must reject without enabling analysis or save");
      assert.strictEqual(responseCollapsed, "false", "calculated channel response must be collapsed until requested");
      assert.strictEqual(responseExpanded, "true", "calculated channel response must open from its summary");
      await sleep(400);
      const shot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (shot)
        await captureRenderedWorkbenchCdp(connection, shot);
    }
    finally {
      try { await vscode.commands.executeCommand("workbench.action.closeAllEditors"); } catch { /* the host may already be closing */ }
      connection.close();
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });
});
async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = "", expectedProjectId = ""): Promise<string> {
  return pollValue("Control Studio webview", async () => {
    const targets = await httpJson(port, "/json");
    for (const target of targets.filter((entry: any) => /index\.html/.test(entry.url ?? ""))) {
      let sessionId: string | undefined;
      try {
        const attached = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
        sessionId = typeof attached.sessionId === "string" ? attached.sessionId : undefined;
        if (!sessionId)
          continue;
        await connection.send("Runtime.enable", {}, sessionId);
        const marker = await evaluateInWebview(connection, sessionId, `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}&&(!${JSON.stringify(expectedProjectId)}||root.dataset.projectId===${JSON.stringify(expectedProjectId)})));`);
        if (marker === "true")
          return sessionId;
        await connection.send("Target.detachFromTarget", { sessionId });
      }
      catch {
        if (sessionId)
          await connection.send("Target.detachFromTarget", { sessionId }).catch(() => undefined);
      }
    }
    return "";
  }, (value) => value.length > 0, 20000);
}
function loopRequest() {
  const loopDefinition = {
    schemaVersion: 1,
    id: "loop:supplied-model-recording-gap",
    name: "20 ms supplied-model recording gap",
    revision: "sha256:supplied-model-recording-gap-loop",
    projectId: "project:supplied-model-recording-gap",
    runtimeId: "simulator",
    controller: {
      instance: "P1.Controller",
      type: "Control.PID",
      revision: "sha256:recording-gap-controller",
      action: "direct",
      taskPeriodMs: 20,
      modeSignal: "%MX8.0",
    },
    signals: {
      setpoint: { symbol: "Setpoint", address: "%ML0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      processVariable: { symbol: "ProcessValue", address: "%IL0", iecType: "LREAL", direction: "read", unit: "°C", range: { minimum: 0, maximum: 100 } },
      manipulatedVariable: { symbol: "ManipulatedValue", address: "%QL0", iecType: "LREAL", direction: "write", unit: "%", range: { minimum: 0, maximum: 100 } },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 20 },
  };
  return {
    schemaVersion: 1,
    source: "ConfiguredLoop",
    scope: { projectId: loopDefinition.projectId, runtimeId: loopDefinition.runtimeId },
    loopDefinition,
  };
}
