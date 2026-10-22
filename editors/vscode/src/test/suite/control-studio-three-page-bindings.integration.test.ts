import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio, engineAnswerRecorderScript } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio three-page binding selection", () => {
  test("lists the actual compiled PID instances and opens an explicitly selected target with a fresh scope", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const sourcePath = path.join(project, "src/Main.st"), source = fs.readFileSync(sourcePath, "utf8");
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
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      let session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("bound host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      const oldScope = await inner("return d.querySelector('[data-studio-three-page]').dataset.editorSessionId;");
      // The page is ready when the engine opens the loop; the binding list is the engine's own answer to setupDiscover, which the panel
      // publishes after the open answer (panel.ts openLoopOnEngine), so the options arrive after host-ready. The test waits for them.
      const offered = await pollValue("the compiled PID instances offered for selection", () => inner("return String([...d.querySelector('[data-studio-controller]').options].some(option=>option.textContent.includes('P1.OtherController')&&option.dataset.family==='PI'));"), value => value === "true", 15000);
      assert.strictEqual(offered, "true", "Every compatible compiled PID instance is selectable by its exact name");
      await inner("const select=d.querySelector('[data-studio-controller]');const option=[...select.options].find(option=>option.textContent.includes('P1.OtherController')&&option.dataset.family==='PI');select.value=option.value;select.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'selected other PID';");
      session = await attachControlStudio(connection, port, "", extra.id);
      await pollValue("selected target host state", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      assert.notStrictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.editorSessionId;"), oldScope);
      // The engine lists the loops and names the selected loop's file (threePageBindingRoute.ts, §0.1a setupDiscover), and it serves
      // the loop it opens: its open answer gives each stage tab the engine's status (webview/engineShared.ts applyStageStatuses).
      assert.notStrictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=model]').dataset.status ?? '';"), "",
        "The engine serves the selected loop, so its stage tabs show the engine's status");
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
    // The engine serves a two-input, two-output loop that names no controller law (C12); a PID law with channels outside the served shapes is
    // refused (`rust_engine_support`), and it opens only the loops that the project declares (`loops.rs`). Lesson 61's loop (the coupled
    // vessels) is the served shape, and the engine's own tests open it for the editors' per-channel rows (`live_sides_tests.rs`,
    // `requirement_outputs_tests.rs`), so the journey saves it into the project.
    const loop = JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../../../examples/control/10-multivariable-control/61-mimo-paths/control-loop.json"), "utf8"));
    const loopFile = path.join(project, "control-loops", `${loop.id}.json`);
    fs.mkdirSync(path.dirname(loopFile), { recursive: true });
    fs.writeFileSync(loopFile, `${JSON.stringify(loop, null, 2)}\n`);
    const signalOf = (signalId: string) => signalId === "manipulatedVariable" ? loop.signals.manipulatedVariable : signalId === "processVariable" ? loop.signals.processVariable
      : loop.signals.additional.find((item: { id: string }) => item.id === signalId);
    const [firstOutput, secondOutput] = loop.channels.outputs as Array<{ modelChannelId: string; signalId: string }>;
    const secondInput = signalOf(loop.channels.inputs[1].signalId).symbol, secondMeasurement = signalOf(secondOutput.signalId).symbol;
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      // The editors start from the engine's open answer (`experimentLimits`, `liveSides`, `requirementOutputs`), so the page is ready once the engine has answered it.
      await pollValue("MIMO Control Studio", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      await inner(engineAnswerRecorderScript());

      await inner("d.querySelector('[data-studio-limits]').click();return 'limits';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-limit]').length);"), "9", "MIMO Limits has two bounds per input/output plus duration");
      assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-studio-limit]')].filter(x=>x.dataset.studioSignal===${JSON.stringify(secondInput)}).length);`), "2");
      assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-studio-limit]')].filter(x=>x.dataset.studioSignal===${JSON.stringify(secondMeasurement)}).length);`), "2");
      const limitsCapture = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-mimo.png");
      if (limitsCapture) await captureRenderedWorkbenchCdp(connection, limitsCapture.replace(/\.png$/, "-limits.png"));
      await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'cancel limits';");

      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      assert.strictEqual(await inner("return String(d.querySelectorAll('[data-studio-requirement]').length);"), "13", "MIMO Requirements has five fields per output (with the optional maximum overshoot, B4.3), the duration, the input disturbance and the Auto-entry tolerance");
      assert.strictEqual(await inner("const x=d.querySelector('[data-studio-requirement=transferTolerance]');return x.value+'|'+x.required;"), "|false", "The Auto-entry tolerance starts blank and optional");
      assert.strictEqual(await inner(`return String([...d.querySelectorAll('[data-studio-requirement]')].filter(x=>x.dataset.studioSignal===${JSON.stringify(secondMeasurement)}).length);`), "5", "each output has five fields, the optional maximum overshoot included (B4.3)");
      assert.strictEqual(await inner(`return String(d.querySelector('[data-studio-requirement="referenceChange:${secondOutput.modelChannelId}"]')?.value||'');`), "", "Second-output target starts blank");
      const [first, second] = [firstOutput.modelChannelId, secondOutput.modelChannelId];
      for (const [key, value] of Object.entries({
        [`referenceChange:${first}`]: "5", [`maximumError:${first}`]: "1", [`settlingBand:${first}`]: "0.2", [`settlingTimeSeconds:${first}`]: "8",
        [`referenceChange:${second}`]: "2", [`maximumError:${second}`]: "0.5", [`settlingBand:${second}`]: "0.1", [`settlingTimeSeconds:${second}`]: "6", durationSeconds: "16",
      })) {
        await inner(`const x=d.querySelector('[data-studio-requirement="${key}"]');if(x){x.value="${value}";x.dispatchEvent(new Event('input',{bubbles:true}));}return 'set';`);
      }
      const requirementsCapture = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-mimo.png");
      await captureRenderedWorkbenchCdp(connection, requirementsCapture.replace(/\.png$/, "-requirements.png"));
      // Done sends the rows as typed, and the engine records them: a refusal would reopen the editor under the field it names, so the check
      // waits for the engine's answer to the change.
      const inputsAnswers = "return JSON.stringify((d.defaultView.__engineAnswers||[]).filter(answer=>answer.method==='inputs').map(answer=>answer.kind));";
      const answered = (JSON.parse(await inner(inputsAnswers)) as string[]).length;
      await inner("d.querySelector('[data-studio-editor-done]').click();return 'done requirements';");
      const answers = JSON.parse(await pollValue("the engine's answer to the requirements", () => inner(inputsAnswers), value => (JSON.parse(value) as string[]).length > answered, 15000)) as string[];
      assert.strictEqual(answers[answers.length - 1], "control-studio.engine.result", "the engine records the requirements of both outputs");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('dialog[open]')));"), "false");

    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      fs.rmSync(loopFile, { force: true });
    }
  });
});
