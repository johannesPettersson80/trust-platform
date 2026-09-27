import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep, type CdpConnection } from "./plant-rendered-harness";
suite("Rendered explicit feedback analysis", () => {
  test("analyzes supplied custom-controller feedback with exact poles and an honest Nyquist curve", async function () {
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
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const folder = fs.mkdtempSync(path.join(os.tmpdir(), "feedback-input-"));
    const plantFile = path.join(folder, "plant.json"), controllerFile = path.join(folder, "controller.json"), badControllerFile = path.join(folder, "bad-controller.json");
    const channel = (id: string, unit: string) => ({ id, unit, operatingRange: { minimum: -100, maximum: 100 } });
    const base = { kind: "transfer-function", revision: "shaft-plant", source: "provided", validation: "unvalidated", evidence: [], delaySeconds: 0 };
    fs.writeFileSync(plantFile, JSON.stringify({ ...base, kind: "continuous-state-space", inputs: [channel("torque", "Nm"), channel("bias", "Nm")], outputs: [channel("angle", "rad")], states: [channel("position", "rad"), channel("velocity", "rad/s")], a: [[0,1],[0,-8]], b: [[0,0],[1,1]], c: [[1,0]], d: [[0,0]] }));
    fs.writeFileSync(controllerFile, JSON.stringify({ ...base, revision: "engineer-p-law", inputs: [channel("error", "rad")], outputs: [channel("command", "Nm")], numerator: [12], denominator: [1] }));
    fs.writeFileSync(badControllerFile, JSON.stringify({ ...base, revision: "incompatible-p-law", inputs: [channel("error", "rad")], outputs: [channel("command", "V")], numerator: [12], denominator: [1] }));
    try {
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("feedback file handlers ready", () => inner("return d.querySelector('[data-finite-search]')?.dataset.ready??'';"), value => value === "true", 10000);
      await inner("Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return 'selected';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('#general-controller-file')));"), "true", "explicit controller import must be available");
      await inner("d.querySelector('#general-controller-file').closest('details').querySelector('summary').click();return 'expanded';");
      async function upload(id: string, file: string) { await connection.send("DOM.enable", {}, session); await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session); const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session); const node = tree.nodes.find((n: any) => n.nodeName === "INPUT" && n.attributes?.includes(id)); assert.ok(node); await connection.send("DOM.setFileInputFiles", { files: [file], backendNodeId: node.backendNodeId }, session); }
      await upload("general-model-file", plantFile);
      await pollValue("plant import", () => inner("return d.querySelector('[data-general-model-status]').textContent+' '+d.querySelector('[data-general-model-provenance]').textContent;"), s => s.includes("shaft-plant"), 10000);
      await upload("general-controller-file", controllerFile);
      await pollValue("controller import", () => inner("return d.querySelector('[data-general-controller-status]').textContent;"), s => s.includes("engineer-p-law"), 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-feedback-plant-channel]').options.length);"), "2", "all supplied plant channels must be selectable");
      await inner("var p=d.querySelector('[data-feedback-plant-channel]');p.value=p.options[1].value;p.dispatchEvent(new Event('change'));return p.selectedOptions[0].textContent; ");
      assert.match(await inner("return d.querySelector('[data-feedback-plant-channel]').selectedOptions[0].textContent;"), /bias.*Nm.*angle.*rad/);
      await upload("general-controller-file", badControllerFile);
      await pollValue("incompatible controller import", () => inner("return d.querySelector('[data-general-controller-status]').textContent;"), s => s.includes("incompatible-p-law"), 10000);
      await inner("d.querySelector('#general-model-min').value='0.1';d.querySelector('#general-model-max').value='100';d.querySelector('#general-model-points').value='100';d.querySelector('[data-general-model-analyze]').click();return 'incompatible analysis';");
      const incompatible = await pollValue("incompatible feedback rejection", () => inner("return d.querySelector('[data-general-model-status]').textContent;"), s => /inverse|unit|compatible|feedback/i.test(s) && !s.includes("Model analysis complete"), 10000);
      assert.match(incompatible, /inverse|unit|compatible|feedback/i);
      assert.strictEqual(await inner("return String(d.querySelector('[data-general-feedback-result]').hidden);"), "true", "incompatible feedback must not render a closed-loop result");
      await upload("general-controller-file", controllerFile);
      await pollValue("compatible controller restored", () => inner("return d.querySelector('[data-general-controller-status]').textContent;"), s => s.includes("engineer-p-law"), 10000);
      await inner("d.querySelector('#general-model-min').value='0.1';d.querySelector('#general-model-max').value='100';d.querySelector('#general-model-points').value='100';d.querySelector('[data-general-model-analyze]').click();return 'running';");
      const result = await pollValue("feedback poles", () => inner("return d.querySelector('[data-general-feedback-result]').textContent;"), s => s.includes("-6") && s.includes("-2"), 10000);
      assert.match(result, /closed-loop/i);
      assert.match(result, /finite sampled/i);
      assert.match(result, /unvalidated/);
      assert.match(result, /estimate/i);
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-general-feedback-result] canvas').length);"), "3");
      await inner("d.querySelector('[data-general-model-save]').click();return 'saving';");
      await pollValue("saved feedback", () => inner("return d.querySelector('[data-general-model-status]').textContent;"), s => s.includes("Saved revision"), 10000);
      const previous = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", request);
      const restored = await attachControlStudio(connection, port, previous);
      await pollValue("restored feedback", () => evaluateInWebview(connection, restored, "Array.from(d.querySelectorAll('[data-control-studio-mode]')).find(b=>b.textContent.trim()==='Tune').click();return d.querySelector('[data-general-feedback-result]').textContent;"), s => s.includes("-6") && s.includes("engineer-p-law"), 10000);
      await evaluateInWebview(connection, restored, "d.querySelector('[data-general-feedback-result]').scrollIntoView();return 'visible';");
      await sleep(400);
      const shot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (shot) {
        captureRenderedWorkbench(shot);
        await evaluateInWebview(connection, restored, "d.querySelector('[data-feedback-nyquist]').scrollIntoView();return 'visible';");
        await sleep(400);
        captureRenderedWorkbench(shot.replace(/\.png$/, "-nyquist.png"));
      }
      await evaluateInWebview(connection, restored, "const details=d.querySelector('#general-controller-file').closest('details');if(!details.open)details.querySelector(':scope > summary').click();return 'controller controls';");
      await evaluateInWebview(connection, restored, "d.querySelector('[data-feedback-clear]').click();return 'clear';");
      await pollValue("cleared controller", () => evaluateInWebview(connection, restored, "return d.querySelector('[data-general-controller-status]').textContent;"), s => s.includes("plant-only"), 10000);
      assert.strictEqual(await evaluateInWebview(connection, restored, "return String(d.querySelector('[data-feedback-clear]').hidden);"), "true", "Remove controller is not an action when no controller is attached");
      assert.strictEqual(await evaluateInWebview(connection, restored, "return String(d.querySelector('[data-feedback-controller-channel]').disabled);"), "true");
      await evaluateInWebview(connection, restored, "d.querySelector('[data-general-model-analyze]').click();return 'plant-only analysis';");
      await pollValue("restored plant-only analysis", () => evaluateInWebview(connection, restored, "return d.querySelector('[data-general-model-status]').textContent;"), s => s.includes("Model analysis complete"), 10000);
      assert.strictEqual(await evaluateInWebview(connection, restored, "return String(d.querySelector('[data-general-feedback-result]').hidden);"), "true", "clearing the controller must restore plant-only analysis");
      if(shot){await evaluateInWebview(connection,restored,"d.querySelector('[data-general-model-status]').scrollIntoView({block:'center'});return 'plant-only';");await sleep(300);captureRenderedWorkbench(shot.replace(/\.png$/,"-plant-only.png"));}
    }
    finally {
      connection.close();
      try { await vscode.commands.executeCommand("workbench.action.closeAllEditors"); } catch { /* the host may already be closing */ }
      fs.rmSync(folder, { recursive: true, force: true });
    }
  });
});
async function attachControlStudio(connection: CdpConnection, port: number, previousEditorSession = ""): Promise<string> {
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
        const marker = await evaluateInWebview(connection, sessionId, `var root=d.querySelector('[data-control-studio]');return String(Boolean(root&&root.dataset.editorSessionId&&root.dataset.editorSessionId!==${JSON.stringify(previousEditorSession)}));`);
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
