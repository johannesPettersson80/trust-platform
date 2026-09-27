import * as assert from "assert";
import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, captureRenderedWorkbench } from "./plant-rendered-harness";

suite("Control Studio guided engineer workflow", () => {
  test("configures compiled ST through real prompts then finds tunes and verifies without handwritten loop JSON", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_GUIDED_ENGINEER !== "1") this.skip();
    this.timeout(120000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    assert.strictEqual(fs.existsSync(path.join(project, "control-loop.json")), false);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    await vscode.workspace.getConfiguration("window").update("dialogStyle", "custom", vscode.ConfigurationTarget.Global);
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(path.join(project, "src/Main.st")));
    const started = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(started.ok, true, JSON.stringify(started));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const targets = await httpJson(port, "/json");
      const target = targets.find((target: any) => String(target.url).includes("workbench"));
      assert.ok(target, "The actual workbench must be available for guided input");
      const { sessionId } = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
      await connection.send("Runtime.enable", {}, sessionId);
      const outer = async (body: string): Promise<string> => {
        const result = await connection.send("Runtime.evaluate", { expression: `(function(){${body}})()`, returnByValue: true }, sessionId);
        if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
        return String(result.result.value);
      };
      const configure = vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
      const answer = async (title: string, value: string, pick = false) => {
        await pollValue(title, () => outer("return document.querySelector('.quick-input-title')?.textContent ?? '';"), text => text === title, 15000);
        if (pick) {
          const choices = await outer("return document.querySelector('.quick-input-list')?.textContent ?? '';" );
          assert.ok(choices.includes(value), `Compiled choices for ${title} must include ${value}: ${choices}`);
        }
        await connection.send("Input.insertText", { text: value }, sessionId);
        await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, sessionId);
        await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, sessionId);
      };
      await answer("Loop name", "Guided engineer");
      await answer("Controller", "Main", true);
      await answer("Process variable", "Main.PV", true);
      await answer("Controller output", "Main.MV", true);
      await answer("Setpoint", "Main.SP", true);
      await answer("Control workflow", "Tune existing proportional controller", true);
      await answer("Process unit", "m"); await answer("Process minimum", "-20"); await answer("Process maximum", "20");
      await answer("Output unit", "V"); await answer("Output minimum", "-20"); await answer("Output maximum", "20");
      await answer("Proportional gain", "Main.Kp", true);
      await answer("Gain minimum", "0"); await answer("Gain maximum", "100");
      await answer("Manual command", "Main.ManualCommand", true);
      await answer("Manual mode", "Main.Manual", true);
      await answer("Manual mode polarity", "TRUE means Manual", true);
      await answer("Observation duration (seconds)", "2");
      await answer("Control channels", "Single channel", true);
      await answer("Actuator capability", "Continuous output", true);
      await answer("Maximum rising output rate (output units/s)", "");
      await answer("Maximum falling output rate (output units/s)", "");
      await answer("Main.MV: applied actuator feedback", "Use the controller output", true);
      await new Promise(resolve => setTimeout(resolve, 400));
      console.log("Setup review state", await outer("return document.body.innerText.slice(-6000);"));
      captureRenderedWorkbench("/tmp/control-guided-review.png");
      await pollValue("reviewed setup", () => outer("return document.querySelector('.monaco-dialog-box')?.textContent ?? '';"), text => text.includes("Save loop"), 10000);
      await outer("[...document.querySelectorAll('.monaco-dialog-box button,.monaco-dialog-box [role=button]')].find(b=>b.textContent.includes('Save loop')).click();return 'save';");
      await configure;
      const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loops/guided-engineer.json"), "utf8"));
      assert.strictEqual(loop.analysis, undefined, "Setup must not invent a plant");
      assert.deepStrictEqual(loop.controller.law, { kind: "proportional", gainSymbol: "Main.Kp" });
      const studio = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, studio, body);
      await pollValue("engineer handlers", () => inner("return d.querySelector('[data-engineer-workflow]').dataset.ready;"), text => text === "true", 10000);
      await inner("d.defaultView.__engineerLatest=undefined;d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__engineerLatest=e.detail);return 'listen';");
      await inner("d.querySelector('[data-engineer-find]').click();return 'find';");
      const found = JSON.parse(await pollValue("identified model", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
        try { const event=JSON.parse(text); return ["model-ready", "failed"].includes(event.status); } catch { return false; }
      }, 15000));
      assert.strictEqual(found.status, "model-ready");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      const tuned = JSON.parse(await pollValue("candidate", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
        try { const event=JSON.parse(text); return ["candidate", "failed"].includes(event.status); } catch { return false; }
      }, 10000));
      assert.strictEqual(tuned.status, "candidate");
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const verified = JSON.parse(await pollValue("actual ST verification", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"), text => {
        try { const event=JSON.parse(text); return ["verified", "verification-failed", "failed"].includes(event.status); } catch { return false; }
      }, 15000));
      assert.strictEqual(verified.status, "verified");
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) { await vscode.commands.executeCommand("notifications.clearAll"); captureRenderedWorkbench(screenshot); }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });
});
