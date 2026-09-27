import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment, sleep } from "./plant-rendered-harness";

suite("Control Studio unsaved identification draft", () => {
  test("retains an unsaved preview and its edited settings without adding a history revision", async function () {
    const phase = process.env.TRUST_IDENTIFICATION_UNSAVED;
    if (!["edit", "reopen", "import", "import-reopen", "form-edit", "form-reopen", "replace"].includes(phase ?? "")) this.skip();
    this.timeout(45000);
    assert.strictEqual(process.env.TRUST_HOT_HEATER_REOPEN_ONLY, "1");
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    const captures = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    let session = "", previousEditor = "", opened = false;
    const inner = (body: string) => evaluateInWebview(connection, session, body);
    const stage = (name: string) => inner(`d.querySelector('[data-control-studio-mode][aria-label^="${name},"]').click();return 'stage';`);
    const capture = async (name: string) => {
      await sleep(200);
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-mode][aria-selected=\"true\"]').textContent.trim();"), "Identify");
      await captureRenderedWorkbenchCdp(connection, path.join(captures, `${name}.png`));
    };
    const open = async () => {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop }); opened = true;
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      session = await attachControlStudio(connection, port, previousEditor, loop.id);
      await pollValue("identification restore settled", () => inner("return d.querySelector('[data-identify-state]').textContent;"),
        value => /Preview only|recording qualified|Accepted for design|Recording selected|Analysis settings changed/.test(value), 10000);
      await stage("Identify");
    };
    const history = async () => JSON.parse(await inner("return JSON.stringify([...d.querySelector('[data-result-history]').options].map(o=>[o.value,o.textContent]));"));
    const importRecording = async () => {
      await connection.send("DOM.enable", {}, session);
      await connection.send("DOM.getDocument", { depth: -1, pierce: true }, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const node = tree.nodes.find((candidate: any) => candidate.nodeName === "INPUT" && candidate.attributes?.includes("identify-import-file"));
      assert.ok(node);
      await connection.send("DOM.setFileInputFiles", { files: [requiredEnvironment("TRUST_IDENTIFICATION_RECORDING_FILE")], backendNodeId: node.backendNodeId }, session);
      await pollValue("selected imported recording", () => inner("var source=d.querySelector('[data-identify-recording-source]');return JSON.stringify({visible:!source.hidden,source:source.textContent,state:d.querySelector('[data-identify-state]').textContent,chart:d.querySelector('[data-control-studio-chart]').getAttribute('aria-description')});"), value => {
        const shown = JSON.parse(value);
        return shown.visible && /303 samples/.test(shown.source) && /Imported recording ready/.test(shown.state) && /303 retained samples/.test(shown.chart);
      }, 10000);
    };
    const closeAndReopen = async () => {
      previousEditor = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
      const tab = vscode.window.tabGroups.all.flatMap(group => group.tabs).find(tab => tab.input instanceof vscode.TabInputWebview && tab.label.includes("Control Studio"));
      assert.ok(tab); assert.ok(await vscode.window.tabGroups.close(tab!)); opened = false;
      await open();
    };
    try {
      await open();
      const before = await history();
      if (phase === "replace") {
        await stage("Validate");
        await inner("var h=d.querySelector('[data-result-history]');h.closest('details').open=true;h.value=[...h.options].find(o=>o.textContent.includes('Revision 2')).value;h.dispatchEvent(new Event('change',{bubbles:true}));d.querySelector('[data-result-open]').click();return 'open accepted history';");
        await pollValue("accepted history opened", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Accepted for design/.test(value), 10000);
        await stage("Identify");
        await inner("var f=d.querySelector('[data-step-field=\"delay.minimum\"]');f.value='0.9';f.dispatchEvent(new Event('input',{bubbles:true}));return 'edit saved result';");
        await capture("01-saved-result-now-draft");
        const badgeAfterEdit = await inner("return d.querySelector('[data-historical-result-badge]').textContent;");
        const staleEvidence = await inner("return String(['[data-identify-model-details]','[data-identify-qualification]','[data-identify-details]'].some(s=>{var e=d.querySelector(s);return e&&!e.hidden;}));");
        await importRecording();
        await capture("02-replacement-recording-plot");
        const chart = await inner("return d.querySelector('[data-control-studio-chart]').getAttribute('aria-description');");
        assert.deepStrictEqual({ badgeAfterEdit, importedSamplesShown: /303 retained samples/.test(chart), staleEvidence },
          { badgeAfterEdit: "Working draft", importedSamplesShown: true, staleEvidence: "false" },
          "editing and importing must replace saved-result presentation immediately, before reopening");
        assert.deepStrictEqual(await history(), before);
        return;
      }
      if (phase?.startsWith("form-")) {
        if (phase === "form-edit") {
          await inner("d.querySelector('[data-step-response-settings]').open=true;var f=d.querySelector('[data-step-field=\"delay.minimum\"]');f.value='1.0';f.dispatchEvent(new Event('input',{bubbles:true}));var b=d.querySelector('[data-step-field=measurementBound]');b.value='';b.dispatchEvent(new Event('input',{bubbles:true}));b.scrollIntoView({block:'center'});return 'raw edits without Check';");
          await capture("01-unchecked-raw-fields");
          await closeAndReopen();
        }
        assert.strictEqual(await inner("return d.querySelector('[data-step-field=measurementBound]').value;"), "",
          "an unchecked blank field must survive close/reopen without reviving the last checked value");
        assert.strictEqual(await inner("return d.querySelector('[data-step-field=\"delay.minimum\"]').value;"), "1.0",
          "raw editing text must retain its exact value before numeric validation");
        assert.match(await inner("return d.querySelector('[data-identify-state]').textContent;"), /Analysis settings changed/);
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-run]').disabled);"), "false");
        assert.strictEqual(await inner("return String(d.querySelector('[data-identify-fit]').hidden);"), "true");
        assert.strictEqual(await inner("return String(d.querySelector('[data-tune-generate]').disabled);"), "true");
        assert.strictEqual(await inner("return d.querySelector('[data-historical-result-badge]').textContent;"), "Working draft");
        assert.deepStrictEqual(await history(), before, "raw editing must preserve every saved history revision");
        await capture("02-restored-unchecked-draft");
        await inner("d.querySelector('[data-step-response-settings]').open=true;d.querySelector('[data-step-field=measurementBound]').scrollIntoView({block:'center'});return 'raw settings';");
        await capture("03-restored-raw-fields");
        if (phase === "form-reopen") {
          await inner("var b=d.querySelector('[data-step-field=measurementBound]');b.value='0.05';b.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-step-response-settings]').open=false;d.querySelector('[data-identify-run]').click();return 'check corrected fields';");
          await pollValue("corrected draft qualifies", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /recording qualified/.test(value), 10000);
          await capture("04-corrected-draft-checked");
        }
        fs.writeFileSync(path.join(captures, "draft-history.json"), JSON.stringify({ phase, history: before }, null, 2));
        return;
      }
      if (phase === "edit") {
        await inner("var f=d.querySelector('[data-step-field=\"delay.minimum\"]');f.value='0.9';f.dispatchEvent(new Event('input',{bubbles:true}));d.querySelector('[data-identify-run]').click();return 'check';");
        await pollValue("edited recording checked", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Estimation recording qualified/.test(value), 10000);
        await inner("d.querySelector('[data-identify-fit]').click();return 'fit';");
        await pollValue("unsaved preview fitted", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Preview only/.test(value), 10000);
        await capture("01-unsaved-preview");
        await closeAndReopen();
      }
      if (phase === "import") {
        await importRecording();
        await capture("01-imported-unsaved-recording");
        await closeAndReopen();
      }
      await capture("02-restored-draft");
      assert.strictEqual(await inner("return d.querySelector('[data-step-field=\"delay.minimum\"]').value;"), "0.9",
        "close/reopen must retain the unsaved input instead of restoring an older saved setting");
      assert.match(await inner("return d.querySelector('[data-identify-state]').textContent;"), phase?.startsWith("import") ? /Recording selected/ : /Preview only/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-tune-generate]').disabled);"), "true");
      assert.deepStrictEqual(await history(), before, "draft persistence cannot append or replace immutable saved history");
      assert.match(await inner("return d.querySelector('[data-result-state]').textContent;"), /draft/i);
      assert.strictEqual(await inner("return d.querySelector('[data-historical-result-badge]').textContent;"), "Working draft");
      await inner("d.querySelector('[data-step-response-settings]').open=true;d.querySelector('[data-step-field=\"delay.minimum\"]').scrollIntoView({block:'center'});return 'settings';");
      await capture("03-restored-draft-settings");
      if (phase === "import-reopen") {
        assert.match(await inner("return d.querySelector('[data-identify-recording-source]').textContent;"), /303 samples/);
        await inner("d.querySelector('[data-step-response-settings]').open=false;d.querySelector('[data-identify-run]').click();return 'check imported validation';");
        await pollValue("retained estimation still available", () => inner("return d.querySelector('[data-identify-state]').textContent;"), value => /Validation recording qualified/.test(value), 10000);
        assert.strictEqual(await inner("return d.querySelector('[data-identify-fit]').textContent;"), "Fit validation run");
        await capture("04-imported-validation-ready");
      }
      fs.writeFileSync(path.join(captures, "draft-history.json"), JSON.stringify({ phase, history: before }, null, 2));
    } finally {
      if (opened) await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
      connection.close();
    }
  });
});
