import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/**
 * The shipped two-input, two-output loop (lesson 61, the coupled vessels): its inputs are the manipulated variable and a second heater, its
 * outputs the process variable and a second vessel temperature, and it names no controller law (C12). The engine serves a two-input channel
 * mapping on a loop without a law, and its own tests open this very loop for it (`live_sides_tests.rs`, `requirement_outputs_tests.rs`); a
 * PID law with two inputs is refused. The engine opens the loops that the project declares, so the journey saves it there.
 */
function mimoLoop(): any {
  return JSON.parse(fs.readFileSync(path.resolve(__dirname, "../../../../../examples/control/10-multivariable-control/61-mimo-paths/control-loop.json"), "utf8"));
}

/** The symbol of the loop signal that a channel binds, by the signal's identifier. */
function symbolOf(loop: any, signalId: string): string {
  const signal = signalId === "manipulatedVariable" ? loop.signals.manipulatedVariable : signalId === "processVariable" ? loop.signals.processVariable
    : loop.signals.additional.find((item: any) => item.id === signalId);
  return signal.symbol;
}

suite("Control Studio Tune channel mapping", () => {
  // The mapping that the engine keeps and refuses is pinned by Rust (`loop_definition/channel_selection_tests.rs`: a swap is accepted and
  // the stages read it, a foreign or repeated signal is refused, the declared loop keeps its bindings). The TypeScript session that
  // held the mapping before the cut is gone, so its two tests of the mapping's study went with it.
  test("renders Channels… only for MIMO Tune targets and exposes exact declared signals", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const siso = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: siso.projectId, runtimeId: siso.runtimeId }, loopDefinition: siso });
      const sisoSession = await attachControlStudio(connection, port, "", siso.id);
      // The engine offers Channels… with its open answer, so the page is asked once the engine has answered.
      await pollValue("SISO Control Studio", () => evaluateInWebview(connection, sisoSession, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      await evaluateInWebview(connection, sisoSession, "d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      const sisoChannels = await evaluateInWebview(connection, sisoSession, "return String([...d.querySelectorAll('[data-engineer-channels]')].some(button => button.checkVisibility()));");
      assert.strictEqual(sisoChannels, "false", "SISO Tune must not expose a multivariable Channels editor");
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");

      const loop = mimoLoop();
      fs.mkdirSync(path.join(project, "control-loops"), { recursive: true });
      fs.writeFileSync(path.join(project, "control-loops", `${loop.id}.json`), `${JSON.stringify(loop, null, 2)}\n`);
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port, siso.id, loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("MIMO Control Studio", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engineer-channels]')?.checkVisibility()));"), "true", "the engine offers Channels… for a two-input loop");
      await inner("d.querySelector('[data-engineer-channels]').click();return 'channels';");
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-studio-channel-mapping]')));"), "true");
      const labels = await inner("return JSON.stringify(Array.from(d.querySelectorAll('[data-studio-channel-mapping] select option')).map(o=>o.textContent.trim()));");
      for (const channel of [...loop.channels.inputs, ...loop.channels.outputs]) {
        assert.ok(labels.includes(symbolOf(loop, channel.signalId)), `the editor offers the declared signal ${symbolOf(loop, channel.signalId)}: ${labels}`);
      }
      await captureRenderedWorkbenchCdp(connection,
        requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-channels.png"));
      assert.strictEqual(await inner(`const field=d.querySelector('[data-studio-channel-field]');
        const url=d.defaultView.getComputedStyle(field).backgroundImage.match(/^url\\(["']?(.*?)["']?\\)$/)?.[1];
        if(!url)return 'false';return await new Promise(resolve=>{const image=new d.defaultView.Image();image.onload=()=>resolve('true');image.onerror=()=>resolve('false');image.src=url;});`), "true",
      "Channel selectors must render the shared dropdown arrow");
      await inner(`const fields=d.querySelectorAll('[data-studio-channel-field]');fields[0].value=fields[1].value;
        d.querySelector('[data-studio-editor-done]').click();return 'duplicate';`);
      // The engine checks the selection and refuses a duplicate with its sentence, which the Tune page shows in its action area with the Channels… action
      // (the Tune page annex); the Identify page's area, where an earlier operation would have put it, stays empty.
      await pollValue("the engine refuses a duplicate command mapping", () => inner("return d.querySelector('[data-studio-message=tune]').textContent;"),
        value => value.includes("already assigned to another channel"), 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-failure-action=tune]').textContent;"), "Channels…");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-message=model]').textContent;"), "", "the refusal is not on the Identify page");
      await captureRenderedWorkbenchCdp(connection,
        requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, "-channels-error.png"));
      // Done closed the editor when it sent the selection (Tune page specification, Channels…): the engine refused it and the study keeps the
      // mapping it had, so Channels… opens on the declared mapping again. Cancel then closes the editor and changes nothing.
      await inner("d.querySelector('[data-engineer-channels]').click();return 'reopen';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-channel-field]').value;"), "manipulatedVariable", "The refused selection leaves the declared mapping");
      await inner(`const fields=d.querySelectorAll('[data-studio-channel-field]');fields[0].value=fields[1].value;
        d.querySelector('[data-studio-editor-cancel]').click();d.querySelector('[data-engineer-channels]').click();return 'cancel and reopen';`);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-channel-field]').value;"), "manipulatedVariable", "Cancel leaves the declared mapping unchanged");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
