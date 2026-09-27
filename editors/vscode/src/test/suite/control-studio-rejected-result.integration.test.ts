import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { decodeWorkflowResult, decodeWorkflowResultHistory } from "../../controlStudio/workflowResult";
import { IdentificationWorkflow } from "../../controlStudio/identification";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Control Studio rejected saved result", () => {
  test("preserves the current draft and its export after incompatible history is rejected", async function () {
    if (process.env.TRUST_REJECTED_RESULT_TEST !== "1") this.skip();
    this.timeout(40000);
    const fixture = JSON.parse(fs.readFileSync(requiredEnvironment("TRUST_REJECTED_RESULT_FIXTURE"), "utf8"));
    const seed = JSON.parse(fs.readFileSync(path.join(path.dirname(requiredEnvironment("TRUST_REJECTED_RESULT_FIXTURE")), "seed-state.json"), "utf8"));
    const seedHistory = seed['trust.controlStudio.result.project:control-lesson-23.control-lesson-23'];
    for (const item of seedHistory) assert.ok(decodeWorkflowResult(item), `seed revision ${item.resultRevision} must pass outer decoding`);
    assert.ok(decodeWorkflowResultHistory(seedHistory), "the history envelope must remain valid so Open can check the incompatible analysis");
    const seededDraft = seed['trust.controlStudio.draft.project:control-lesson-23.simulator.control-lesson-23'].workflow;
    assert.ok(decodeWorkflowResult(seededDraft), "the initial working draft must decode");
    assert.doesNotThrow(() => new IdentificationWorkflow().restore(seededDraft.identificationStudy));
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let opened = false;
    const originalDialog = vscode.window.showSaveDialog;
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      const session = await attachControlStudio(connection, port, "", loop.id);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      const stage = (name: string) => inner(`d.querySelector('[data-control-studio-mode][aria-label^="${name},"]').click();return 'stage';`);
      try {
        await pollValue("current preview restored", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Preview only/.test(value), 10000);
      } catch (error) {
        fs.writeFileSync(path.join(captures, "restore-diagnostic.txt"), await inner("return d.body.innerText;"));
        await captureRenderedWorkbenchCdp(connection, path.join(captures, "restore-diagnostic.png"));
        throw error;
      }
      const exportCurrent = async (name: string) => {
        const destination = path.join(captures, `${name}.json`);
        let dialogs = 0;
        vscode.window.showSaveDialog = async options => {
          dialogs++; assert.ok(options?.defaultUri?.fsPath.endsWith(".json"));
          return vscode.Uri.file(destination);
        };
        await stage("Validate");
        await inner("d.querySelector('[data-result-export-json]').click();return 'export current result';");
        await pollValue("host export written", async () => String(fs.existsSync(destination)), value => value === "true", 10000);
        assert.strictEqual(dialogs, 1);
        return JSON.parse(fs.readFileSync(destination, "utf8"));
      };
      const before = await exportCurrent("before-rejected-open");
      assert.strictEqual(before.resultId, fixture.currentResultId);
      assert.strictEqual(before.identificationStudy.phase, "preview");
      await inner(`var history=d.querySelector('[data-result-history]');history.closest('details').open=true;history.value=String(${fixture.rejectedRevision});history.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-result-open]').click();return 'open incompatible result';`);
      const rejection = await pollValue("saved authority rejection", () => inner("return d.querySelector('[data-result-state]').textContent;"), value => /contradict/.test(value), 10000);
      await inner("d.querySelector('[data-result-state]').scrollIntoView({block:'center'});return 'visible rejection';");
      await sleep(200);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "00-rejected-open.png"));
      await stage("Identify");
      assert.match(await inner("return d.querySelector('[data-identify-state]').textContent;"), /Preview only/);
      assert.strictEqual(await inner("return d.querySelector('[data-historical-result-badge]').textContent;"), "Working draft");
      assert.match(await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-description');"), /303 retained samples/);
      await sleep(200);
      await captureRenderedWorkbenchCdp(connection, path.join(captures, "01-current-preview-preserved.png"));
      const after = await exportCurrent("after-rejected-open");
      fs.writeFileSync(path.join(captures, "rejection.json"), JSON.stringify({ rejection, beforeId: before.resultId, afterId: after.resultId,
        beforeRevision: before.resultRevision, afterRevision: after.resultRevision }, null, 2));
      assert.strictEqual(after.resultRevision, before.resultRevision, "a rejected Open must preserve the current exported revision");
      assert.deepStrictEqual(after, before, "a rejected Open must not change the current export to the rejected saved result");
    } finally {
      vscode.window.showSaveDialog = originalDialog;
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
