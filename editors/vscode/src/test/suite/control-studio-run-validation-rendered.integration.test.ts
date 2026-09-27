import { revealControlForCapture } from "./control-studio-capture-visibility";
import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep, type CdpConnection } from "./plant-rendered-harness";
suite("Rendered recorded-run validation", () => {
  test("imports declared run evidence and evaluates matched engineer requirements", async function () {
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
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "validation-input-"));
    const run = { id: "current-from-engineer", evidence: "ST-executed", parameters: { gain: 2 }, conditions: { initialState: { axis: 0 }, events: [], durationSeconds: 2, sampleTimesSeconds: [0, 1, 2], seed: 7, modelRevision: "m", programRevision: "p", parameterSchemaRevision: "s" }, channels: [{ id: "axis", unit: "m" }], samples: [0, 1, 2].map(i => ({ timeSeconds: i, sequence: i, values: { axis: { value: 1, quality: "good" } } })) };
    const currentFile = path.join(folder, "current.json"), candidateFile = path.join(folder, "candidate.json"), malformedFile = path.join(folder, "malformed.json");
    fs.writeFileSync(currentFile, JSON.stringify(run));
    fs.writeFileSync(malformedFile, "{not valid JSON");
    const candidate = JSON.parse(JSON.stringify(run));
    candidate.id = "candidate-from-engineer";
    candidate.parameters.gain = 3;
    candidate.samples[1].values.axis.quality = "forced";
    fs.writeFileSync(candidateFile, JSON.stringify(candidate));
    const requirements = [{ id: "axis-error", kind: "error", channel: "axis", window: { start: 0, end: 2 }, reference: 1, statistic: "rms", maximum: 0.1 }];
    try {
      const session = await attachControlStudio(connection, port, "", request.scope.projectId);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("validation handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Validate').click();return 'selected';");
      await inner("d.querySelector('[data-manual-run-validation] > summary').click();return 'open imported runs';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('#run-validation-current')));"), "true", "engineer run import must exist in Validate");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-evaluate]').disabled);"), "true", "Evaluate starts disabled without a current run");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "true", "Save starts disabled without a completed evaluation");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-result]').hidden);"), "true", "Validate starts without a result");
      await inner("d.defaultView.__runValidationEvents=[];d.defaultView.addEventListener('trust-control-studio-run-validation',e=>d.defaultView.__runValidationEvents.push(e.detail));return 'listen';");
      async function upload(id: string, file: string) { await connection.send("DOM.enable", {}, session); await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session); const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session); const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes(id)); assert.ok(node); await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session); await pollValue(`${id} file selected`, () => inner(`return String(d.querySelector('#${id}').files?.length??0);`), value => value === "1", 5000); }
      await upload("run-validation-current", currentFile);
      await pollValue("current import", () => inner("return d.querySelector('[data-run-validation-current]').textContent;"), s => s.includes("current-from-engineer"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-evaluate]').disabled);"), "false", "a current-only run enables Evaluate");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "true", "Save stays disabled before evaluation");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-result]').hidden);"), "true", "current import has no stale result");
      await upload("run-validation-current", malformedFile);
      await pollValue("malformed current rejection", () => inner("return d.querySelector('[data-run-validation-status]').textContent;"), s => /JSON|run|invalid|Unexpected/i.test(s) && !s.includes("Evaluation complete"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-evaluate]').disabled);"), "false", "malformed replacement does not discard the accepted current run");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "true");
      await inner("d.querySelector('#run-validation-requirements').value='{}';d.querySelector('#run-validation-requirements').dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-run-validation-evaluate]').click();return 'invalid requirements';");
      await pollValue("invalid requirements rejection", () => inner("return d.querySelector('[data-run-validation-status]').textContent;"), s => s.includes("1–64 requirements"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-result]').hidden);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "true");
      await inner("d.querySelector('#run-validation-requirements').value=" + JSON.stringify(JSON.stringify(requirements)) + ";d.querySelector('#run-validation-requirements').dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-run-validation-evaluate]').click();return 'current-only evaluation';");
      await pollValue("current-only evaluation", () => inner("return String(!d.querySelector('[data-run-validation-result]').hidden);"), s => s === "true", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "false");
      await inner("d.defaultView.addEventListener('trust-control-studio-run-validation',function cancelOnce(e){if(e.detail.status==='running'){d.defaultView.removeEventListener('trust-control-studio-run-validation',cancelOnce);d.querySelector('[data-run-validation-cancel]').click();}});d.querySelector('[data-run-validation-evaluate]').click();return 'cancel active evaluation';");
      await pollValue("cancelled run validation", () => inner("return d.querySelector('[data-run-validation-status]').textContent;"), s => /cancelled/i.test(s), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled&&d.querySelector('[data-run-validation-result]').hidden);"), "true");
      const cancelledShot=process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if(cancelledShot){await sleep(500);await revealControlForCapture(inner, "[data-run-validation-status]");await sleep(300);await captureRenderedWorkbenchCdp(connection, cancelledShot.replace(/\.png$/,"-cancelled-validation.png"));}
      await inner("d.querySelector('[data-run-validation-evaluate]').click();return 'reevaluate after cancellation';");
      await pollValue("completed after cancellation", () => inner("return String(!d.querySelector('[data-run-validation-save]').disabled);"), s => s === "true", 10000);
      await inner("const r=d.querySelector('#run-validation-requirements');r.value=r.value+' ';r.dispatchEvent(new Event('input',{bubbles:true}));const o=d.querySelector('#run-validation-objective');o.value='axis-error';o.dispatchEvent(new Event('input',{bubbles:true}));return 'edited';");
      await pollValue("edited validation invalidation", () => inner("return d.querySelector('[data-run-validation-status]').textContent;"), s => s.includes("Evaluate again"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-result]').hidden);"), "true");
      assert.strictEqual(await inner("return String(d.querySelector('[data-run-validation-save]').disabled);"), "true");
      await upload("run-validation-candidate", candidateFile);
      await pollValue("candidate import", () => inner("return d.querySelector('[data-run-validation-candidate]').textContent;"), s => s.includes("candidate-from-engineer"), 10000);
      await inner("d.querySelector('#run-validation-requirements').value=" + JSON.stringify(JSON.stringify(requirements)) + ";d.querySelector('#run-validation-objective').value='axis-error';d.querySelector('[data-run-validation-evaluate]').click();return 'running';");
      const result = await pollValue("evaluated imported requirements", () => inner("return d.querySelector('[data-run-validation-result]').textContent;"), s => s.includes("Inconclusive"), 10000);
      assert.match(result, /forced|good-quality/);
      assert.match(result, /ST-executed/);
      assert.match(result, /imported/i);
      await inner("d.querySelector('[data-run-validation-save]').click();return 'saving';");
      await pollValue("saved validation", () => inner("return d.querySelector('[data-run-validation-status]').textContent;"), s => s.includes("Saved revision"), 10000);
      const previous = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      const restored = await attachControlStudio(connection, port, previous);
      await pollValue("restored validation", () => evaluateInWebview(connection, restored, "Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Validate').click();return d.querySelector('[data-run-validation-result]').textContent;"), s => s.includes("Inconclusive"), 10000);
      await evaluateInWebview(connection, restored, "d.querySelector('[data-manual-run-validation] > summary').click();return 'inspect restored imported evaluation';");
      assert.match(await evaluateInWebview(connection, restored, "return d.querySelector('[data-run-validation-current]').textContent;"), /current-from-engineer/,
        "restored current-run identity must agree with the visible evaluated data");
      assert.match(await evaluateInWebview(connection, restored, "return d.querySelector('[data-run-validation-candidate]').textContent;"), /candidate-from-engineer/,
        "restored candidate identity must agree with the visible evaluated data");
      assert.match(await evaluateInWebview(connection, restored, "return d.querySelector('#run-validation-requirements').value;"), /axis-error/);
      await evaluateInWebview(connection, restored, "d.querySelector('[data-run-validation-evaluate]').click();return 'evaluate retained imported runs';");
      await pollValue("retained imported runs reevaluated", () => evaluateInWebview(connection, restored, "return d.querySelector('[data-run-validation-status]').textContent;"), value => value.includes("Evaluation complete: Inconclusive"), 10000);
      await evaluateInWebview(connection, restored, "d.querySelector('[data-run-validation-result]').scrollIntoView();return 'visible';");
      await sleep(400);
      assert.strictEqual(await evaluateInWebview(connection, restored, "var t=d.querySelector('[data-run-validation-result] .chart-table');return String(t.scrollHeight<=t.clientHeight);"), "true", "both imported run outcomes must be visible without a nested vertical scroll");
      assert.strictEqual(await evaluateInWebview(connection, restored, "var w=d.defaultView;return String(w.getComputedStyle(d.querySelector('#run-validation-requirements')).backgroundColor===w.getComputedStyle(d.querySelector('#run-validation-objective')).backgroundColor);"), "true", "requirements editor uses the shared input theme");
      const shot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (shot)
        await captureRenderedWorkbenchCdp(connection, shot);
    }
    finally {
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
    id: "loop:recording-gap",
    name: "20 ms recording gap",
    revision: "sha256:recording-gap-loop",
    projectId: "project:recording-gap",
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
