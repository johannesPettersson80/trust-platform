import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio three-page binding selection", () => {
  test("lists the actual compiled PID instances and opens an explicitly selected target with a fresh scope", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const sourcePath = path.join(project, "src/main.st"), source = fs.readFileSync(sourcePath, "utf8");
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    // A second compiled instance tests target selection; this fixture does not claim
    // that its unused controller has passed process validation or can be applied.
    const extra = JSON.parse(JSON.stringify(loop).split("P1.Controller").join("P1.OtherController")
      .split("P1.Parameters").join("P1.OtherParameters").split("P1.ParameterRevision").join("P1.OtherParameterRevision"));
    extra.id += "-other-controller"; extra.revision += "-other-controller"; extra.name = "Other controller";
    extra.parameterBinding.loop_id = extra.id;
    const loopFile = path.join(project, "control-loops/other.json");
    fs.mkdirSync(path.dirname(loopFile), { recursive: true });
    fs.writeFileSync(loopFile, JSON.stringify(extra));
    fs.writeFileSync(sourcePath, source.replace("    Controller : Control.PID;", "    Controller : Control.PID;\n    OtherController : Control.PID;\n    OtherParameters : Control.PIDParameters;\n    OtherParameterRevision : DINT;"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(sourcePath));
      const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
      assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted", JSON.stringify(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId })));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      let session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("bound host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      const oldScope = await inner("return d.querySelector('[data-studio-three-page]').dataset.editorSessionId;");
      assert.strictEqual(await inner("return String([...d.querySelector('[data-studio-controller]').options].some(option=>option.textContent.includes('P1.OtherController')&&option.dataset.family==='PI'));"), "true", "Every compatible compiled PID instance is selectable by its exact name");
      await inner("const select=d.querySelector('[data-studio-controller]');const option=[...select.options].find(option=>option.textContent.includes('P1.OtherController')&&option.dataset.family==='PI');select.value=option.value;select.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected other PID';");
      session = await attachControlStudio(connection, port, "", extra.id);
      await pollValue("selected target host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      assert.notStrictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.editorSessionId;"), oldScope);
      const target = JSON.parse(await inner("return d.querySelector('[data-studio-three-page]').dataset.studioLoop;"));
      assert.strictEqual(target.controller.instance, "P1.OtherController");
      assert.ok(target.parameterBinding.parameters.every((parameter: { symbol: string }) => parameter.symbol.startsWith("P1.OtherParameters.")));
      assert.strictEqual(vscode.debug.activeDebugSession, undefined, "Selection does not start or write a runtime");
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'inspect selected PID';");
      await captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-selected-controller.png"));
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      fs.writeFileSync(sourcePath, source); fs.unlinkSync(loopFile);
    }
  });

  test("edits per-output requirements and per-channel limits in the rendered MIMO workspace", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const base = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const loop = JSON.parse(JSON.stringify(base));
    loop.id = `${base.id}-mimo-requirements`; loop.revision = `${base.revision}-mimo-requirements`; loop.name = "MIMO requirements review"; loop.parameterBinding.loop_id = loop.id; loop.schemaVersion = 2;
    loop.signals.additional = [
      ...(loop.signals.additional ?? []),
      { id: "processVariable2", channel: "output-2", role: "PV", symbol: "P1.ProcessValue2", address: "%ID32", iecType: "REAL", direction: "read", unit: "bar", range: { minimum: 2, maximum: 8 } },
      { id: "setpoint2", channel: "output-2", role: "SP", symbol: "P1.Setpoint2", address: "%MD32", iecType: "REAL", direction: "read", unit: "bar", range: { minimum: 2, maximum: 8 } },
      { id: "manipulatedVariable2", channel: "input-2", role: "MV", symbol: "P1.ManipulatedValue2", address: "%QD32", iecType: "REAL", direction: "write", unit: "%", range: { minimum: 10, maximum: 90 } },
    ];
    loop.channels = {
      inputs: [
        ...(loop.channels?.inputs ?? [{ modelChannelId: "input-1", signalId: "manipulatedVariable", actuator: { kind: "continuous", minimum: 0, maximum: 100 } }]),
        { modelChannelId: "input-2", signalId: "manipulatedVariable2", actuator: { kind: "continuous", minimum: 15, maximum: 85, rateUpPerS: 4, rateDownPerS: 3, minimumOnSeconds: 1, minimumOffSeconds: 2 } },
      ],
      outputs: [
        ...(loop.channels?.outputs ?? [{ modelChannelId: "output-1", signalId: "processVariable", setpointSignalId: "setpoint" }]),
        { modelChannelId: "output-2", signalId: "processVariable2", setpointSignalId: "setpoint2" },
      ],
    };
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      const { validateControlLoopDefinition } = require("../../controlStudio/loopDefinition");
      assert.strictEqual(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId }).kind, "accepted", JSON.stringify(validateControlLoopDefinition(loop, { projectId: loop.projectId, runtimeId: loop.runtimeId })));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("MIMO Control Studio", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.ready;"), value => value === "true", 10000);

      await inner("d.querySelector('[data-studio-limits]').click();return 'limits';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-limit]').length);"), "9", "MIMO Limits has two bounds per input/output plus duration");
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-studio-limit]')].filter(x=>x.dataset.studioSignal==='P1.ManipulatedValue2').length);"), "2");
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-studio-limit]')].filter(x=>x.dataset.studioSignal==='P1.ProcessValue2').length);"), "2");
      const limitsCapture = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-mimo.png");
      if (limitsCapture) await captureRenderedWorkbenchCdp(connection, limitsCapture.replace(/\.png$/, "-limits.png"));
      await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancel limits';");

      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-requirement]').length);"), "9", "MIMO Requirements has four fields per output plus duration");
      assert.strictEqual(await inner("return String([...d.querySelectorAll('[data-studio-requirement]')].filter(x=>x.dataset.studioSignal==='P1.ProcessValue2').length);"), "4");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-requirement=\\'referenceChange:output-2\\']')?.value||'');"), "", "Second-output target starts blank");
      for (const [key, value] of Object.entries({
        "referenceChange:output-1": "5", "maximumError:output-1": "1", "settlingBand:output-1": "0.2", "settlingTimeSeconds:output-1": "8",
        "referenceChange:output-2": "2", "maximumError:output-2": "0.5", "settlingBand:output-2": "0.1", "settlingTimeSeconds:output-2": "6", durationSeconds: "16",
      })) {
        await inner(`const x=d.querySelector('[data-studio-requirement="${key}"]');if(x){x.value="${value}";x.dispatchEvent(new Event('input',{bubbles:true}));}return 'set';`);
      }
      const requirementsCapture = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-mimo.png");
      await captureRenderedWorkbenchCdp(connection, requirementsCapture.replace(/\.png$/, "-requirements.png"));
      await inner("d.querySelector('[data-studio-editor-done]').click();return 'done requirements';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('dialog[open]')));"), "false");

    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
