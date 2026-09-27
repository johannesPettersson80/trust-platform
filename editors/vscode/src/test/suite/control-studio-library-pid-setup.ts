import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { execFile } from "child_process";
import { promisify } from "util";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import { captureRenderedWorkbenchCdp, httpJson, pollValue, requiredEnvironment, type CdpConnection } from "./plant-rendered-harness";

/** Exercises the public native wizard and OS file chooser; no window API is mocked. */
export async function configureLibraryPidThroughDialogs(connection: CdpConnection, port: number,
  project: string, reference: ControlLoopDefinition, captures: string): Promise<ControlLoopDefinition> {
  const adapterPath = path.join(project, "pid-adapter.json");
  fs.writeFileSync(adapterPath, JSON.stringify({ controller: reference.controller, parameterBinding: reference.parameterBinding }, null, 2));
  assert.strictEqual(fs.existsSync(path.join(project, "control-loop.json")), false);
  await vscode.workspace.getConfiguration("window").update("dialogStyle", "custom", vscode.ConfigurationTarget.Global);
  const target = (await httpJson(port, "/json")).find((entry: any) => String(entry.url).includes("workbench"));
  assert.ok(target, "The actual workbench must expose the setup dialogs");
  const { sessionId } = await connection.send("Target.attachToTarget", { targetId: target.id, flatten: true });
  await connection.send("Runtime.enable", {}, sessionId);
  const outer = async (body: string) => {
    const result = await connection.send("Runtime.evaluate", { expression: `(function(){${body}})()`, returnByValue: true }, sessionId);
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return String(result.result.value);
  };
  let step = 0;
  const waitForTitle = (title: string) => pollValue(title,
    () => outer("return document.querySelector('.quick-input-title')?.textContent??'';"),
    actual => {
      const prompt = actual.replace(/\s*\(\d+\/\d+\)\s*$/, "");
      return prompt === title || prompt.endsWith(` · ${title}`);
    }, 15000);
  const backTo = async (from: string, title: string, previous: string, pick = false) => {
    await waitForTitle(from);
    await captureRenderedWorkbenchCdp(connection, path.join(captures, `setup-back-from-${from.replace(/[^a-z]+/gi, "-").toLowerCase()}.png`));
    assert.strictEqual(await outer(`const widget=document.querySelector('.quick-input-widget');
      const back=widget?.querySelector('[aria-label^="Back"],[title^="Back"]');
      if(!back?.checkVisibility())return 'missing';back.click();return 'back';`), "back",
    "every setup prompt after the first answer must provide a visible Back action");
    await waitForTitle(title);
    const retained = await outer(pick
      ? "return document.querySelector('.quick-input-list .focused')?.textContent??'';"
      : "return document.querySelector('.quick-input-widget input')?.value??'';");
    assert.ok(retained.includes(previous), `${title} must restore the previous answer ${previous}: ${retained}`);
    assert.match(await outer("return document.querySelector('.quick-input-widget')?.innerText??'';"), /[1-4]\/4/,
      "setup must show its current phase and total phases");
    assert.strictEqual(fs.existsSync(path.join(project, "control-loops/guided-pid.json")), false,
      "Back must not save a partial loop");
  };
  const answer = async (title: string, value: string, pick = false) => {
    await waitForTitle(title);
    if (pick) {
      const list = await outer("return document.querySelector('.quick-input-list')?.textContent??'';");
      assert.ok(list.includes(value), `${title} must offer ${value}`);
      if (title === "Controller") {
        const rows: string[] = JSON.parse(await outer(`return JSON.stringify(Array.from(document.querySelectorAll('.quick-input-list .monaco-list-row'))
          .filter(row=>row.checkVisibility()).map(row=>row.textContent?.replace(/\\s+/g,'')??''));`));
        assert.ok(rows.includes("P1.ControllerControl.PID"), `the exact compiled Control.PID option must remain visible: ${JSON.stringify(rows)}`);
        assert.ok(!rows.includes("P1P1"), `a repeated instance/type must not duplicate the row label: ${JSON.stringify(rows)}`);
      }
    }
    await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key: "a", code: "KeyA", modifiers: 2, windowsVirtualKeyCode: 65 }, sessionId);
    await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key: "a", code: "KeyA", modifiers: 2, windowsVirtualKeyCode: 65 }, sessionId);
    await connection.send("Input.insertText", { text: value }, sessionId);
    await captureRenderedWorkbenchCdp(connection, path.join(captures, `setup-${String(++step).padStart(2, "0")}.png`));
    await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, sessionId);
    await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 }, sessionId);
  };
  const cancelledSetup = vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
  await answer("Loop name", "Cancelled draft");
  await waitForTitle("Controller");
  await connection.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 }, sessionId);
  await connection.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 }, sessionId);
  await cancelledSetup;
  assert.strictEqual(fs.existsSync(path.join(project, "control-loops/cancelled-draft.json")), false,
    "cancelling native setup must not save the draft");
  const configure = vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
  await answer("Loop name", "Guided PID draft");
  await backTo("Controller", "Loop name", "Guided PID draft");
  await answer("Loop name", "Guided PID");
  await answer("Controller", reference.controller.instance, true);
  await backTo("Process variable", "Controller", reference.controller.instance, true);
  await answer("Controller", reference.controller.instance, true);
  await answer("Process variable", reference.signals.processVariable.symbol, true);
  await answer("Controller output", reference.signals.manipulatedVariable.symbol, true);
  await answer("Setpoint", reference.signals.setpoint!.symbol, true);
  await answer("Control workflow", "Use an existing controller adapter", true);
  await answer("Process unit", reference.signals.processVariable.unit);
  await answer("Process minimum", "invalid");
  await pollValue("invalid process bound stays editable", () => outer("return document.querySelector('.quick-input-message')?.textContent??'';"),
    text => text.includes("Enter a valid finite number"), 5000);
  await captureRenderedWorkbenchCdp(connection, path.join(captures, "setup-invalid-process-minimum.png"));
  await answer("Process minimum", "0"); await answer("Process maximum", "100");
  await answer("Output unit", "%"); await answer("Output minimum", "0"); await answer("Output maximum", "100");
  await answer("Observation duration (seconds)", "16");
  await promisify(execFile)("python3", [requiredEnvironment("TRUST_CONTROL_STUDIO_NATIVE_OPEN_DRIVER"), adapterPath,
    path.join(captures, "setup-adapter-file.png")], { timeout: 30000 });
  await answer("Control channels", "Single channel", true);
  await answer("Actuator capability", "Continuous output", true);
  await answer("Maximum rising output rate (output units/s)", "25");
  await backTo("Maximum falling output rate (output units/s)", "Maximum rising output rate (output units/s)", "25");
  await answer("Maximum rising output rate (output units/s)", "25");
  await answer("Maximum falling output rate (output units/s)", "25");
  await answer(`${reference.signals.manipulatedVariable.symbol}: applied actuator feedback`, "Use the controller output", true);
  try {
    await pollValue("reviewed loop", () => outer("return document.querySelector('.monaco-dialog-box')?.textContent??'';"), text => text.includes("Save loop"), 10000);
  } catch (error) {
    fs.writeFileSync(path.join(captures, "setup-failed-state.txt"), await outer("return document.body.innerText;"));
    await captureRenderedWorkbenchCdp(connection, path.join(captures, "setup-failed-state.png"));
    throw error;
  }
  await captureRenderedWorkbenchCdp(connection, path.join(captures, "setup-review.png"));
  assert.strictEqual(await outer("const b=[...document.querySelectorAll('.monaco-dialog-box button,.monaco-dialog-box [role=button]')].find(b=>b.textContent.includes('Save loop'));if(!b?.checkVisibility())return 'unavailable';b.click();return 'saved';"), "saved");
  await configure;
  const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loops/guided-pid.json"), "utf8")) as ControlLoopDefinition;
  assert.strictEqual(loop.controller.type, "Control.PID");
  assert.deepStrictEqual(loop.controller.pidInitialization, reference.controller.pidInitialization);
  assert.deepStrictEqual(loop.parameterBinding?.parameters, reference.parameterBinding?.parameters);
  assert.strictEqual(loop.parameterBinding?.loop_id, loop.id);
  assert.strictEqual(loop.analysis, undefined, "Setup must not invent a plant model");
  return loop;
}
