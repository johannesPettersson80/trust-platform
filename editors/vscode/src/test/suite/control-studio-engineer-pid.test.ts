import { pidLoop, proportionalLoop, channel, pidInitialization } from "./control-studio-pid-fixtures";
import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { EngineerWorkflowSession } from "../../controlStudio/engineerWorkflowSession";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, captureRenderedWorkbench, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

const AUTO_FAMILY = ["automatic", "P", "PID"].includes(process.env.TRUST_CONTROL_STUDIO_ENGINEER_FAMILY ?? "")
  ? process.env.TRUST_CONTROL_STUDIO_ENGINEER_FAMILY as "automatic" | "P" | "PID" : "PI";
const CANCEL_AUTO = process.env.TRUST_CONTROL_STUDIO_CANCEL_AUTO === "1";

suite("Control Studio connected PID engineer path", () => {
  test("invalidates a cleared model without reporting a failed tuning operation", () => {
    const session = new EngineerWorkflowSession(pidLoop(), () => undefined, () => undefined);
    try {
      const result = session.modelChanged();
      assert.strictEqual(result.status, "edited", "Preparing identification is valid before a model exists");
      assert.strictEqual(result.failure, undefined);
      assert.strictEqual((result.stageEvidence as any).hasModel, false);
    } finally { session.dispose(); }
  });

  test("exposes all typed PID gains through the actual ST runtime", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const loop = JSON.parse(fs.readFileSync(path.join(vscode.workspace.workspaceFolders![0].uri.fsPath, "control-loop.json"), "utf8"));
    const start = await runtimeLifecycleService.startLocalSimulator();
    assert.strictEqual(start.ok, true, JSON.stringify(start));
    try {
      const connection = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      const status = await connection.client.status();
      assert.deepStrictEqual(status.values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
    } finally { await runtimeLifecycleService.stopRuntime(); }
  });

  test("offers Auto by default for a typed PI adapter while retaining Manual staging", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(60_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("PID engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-family]').value;"), "automatic");
      assert.strictEqual(await inner("var f=d.querySelector('[data-engineer-family]');return String(Array.from(f.options).find(o=>o.textContent.trim().startsWith('PI')).disabled);"), "false");
      assert.strictEqual(await inner("var f=d.querySelector('[data-engineer-family]');var pi=Array.from(f.options).find(o=>o.textContent.trim().startsWith('PI'));pi.value='PI';f.value='PI';f.dispatchEvent(new Event('change'));return d.querySelector('[data-engineer-application-mode]').value;"), "auto");
      assert.strictEqual(await inner("var f=d.querySelector('[data-engineer-family]');return String(Array.from(f.options).find(o=>o.textContent.trim().startsWith('PD')).disabled);"), "false");
      assert.strictEqual(await inner("var f=d.querySelector('[data-engineer-family]');return String(Array.from(f.options).find(o=>o.textContent.trim().startsWith('PID')).disabled);"), "false");
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("renders and runs the model-data recovery action from an incompatible supplied model", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const modelPath = path.join(project, "unvalidated-model.json");
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        source: "provided", validation: "unvalidated", evidence: [], revision: "unvalidated-model",
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] }));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      await connection.send("DOM.enable", {}, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      assert.ok(input, "Supply model exposes the real file input");
      await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: input.backendNodeId }, session);
      await pollValue("model-data recovery", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => value.includes("independent validation"), 10_000);
      assert.strictEqual(await inner("const b=d.querySelector('[data-engineer-find]');return JSON.stringify({primary:b.classList.contains('trust-button--primary'),title:b.title});"),
        JSON.stringify({ primary: true, title: "Next action: Find model." }));
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-engineer-find]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 250));
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
      await inner("d.querySelector('[data-engineer-find]').click();return 'find';");
      await pollValue("Find model recovery", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Baseline must match|Identification.*trial/.test(value), 20_000);
      await inner("d.querySelector('[data-engineer-cancel]').click();return 'cancel';");
      await pollValue("Find model cancellation", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Operation cancelled|cancelled/i.test(value), 20_000);
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("runs the rendered multisine Find-model journey against actual ST", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.experiment = { execution: "control-studio", kind: "multisine", amplitude: 1, durationSeconds: 10, seed: 7 };
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-find]').click();return 'find';");
      const status = await pollValue("multisine fitted model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Model found and validated|failed/i.test(value), 90_000);
      assert.match(status, /Model found and validated/, status);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "false");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      const tuned = await pollValue("multisine model candidate", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /candidate|failed/i.test(value), 20_000);
      assert.match(tuned, /candidate/i, tuned);
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const verified = await pollValue("multisine model actual-ST Verify", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Verification passed|failed/i.test(value), 60_000);
      assert.match(verified, /Verification passed/, verified);
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 250));
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("runs the rendered PRBS Find-model journey against actual ST", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    loop.experiment = { execution: "control-studio", kind: "prbs", amplitude: 1, durationSeconds: 10, seed: 7 };
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.defaultView.__engineerLatest=undefined;d.defaultView.addEventListener('trust-control-studio-engineer',e=>d.defaultView.__engineerLatest=e.detail);d.querySelector('[data-engineer-find]').click();return 'find';");
      const status = await pollValue("PRBS fitted model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Model found and validated|failed/i.test(value), 90_000);
      assert.match(status, /Model found and validated/, status);
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      const tuned = JSON.parse(await pollValue("PRBS model candidate", () => inner("return JSON.stringify(d.defaultView.__engineerLatest??{});"),
        value => { const event = JSON.parse(value); return ["candidate", "failed"].includes(event.status); }, 20_000));
      assert.strictEqual(tuned.status, "candidate", tuned.reason);
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const verified = await pollValue("PRBS model actual-ST Verify", () => inner("return d.querySelector('[data-engineer-status]').textContent;"),
        value => /Verification passed|failed/i.test(value), 60_000);
      assert.match(verified, /Verification passed/, verified);
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 250));
        await captureRenderedWorkbenchCdp(connection, screenshot);
      }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("runs the rendered PI Tune, Verify and Manual Apply journey against actual ST", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const modelPath = path.join(project, "pi-model.json");
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        source: "provided", validation: "validated", evidence: ["fixture"], revision: "pi-model",
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] }));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("PID engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      await connection.send("DOM.enable", {}, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      assert.ok(input, "Supply model exposes the real file input");
      await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: input.backendNodeId }, session);
      await pollValue("selected PI model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("PI control"), 10_000);
      await inner("var f=d.querySelector('[data-engineer-family]');f.value='PI';f.dispatchEvent(new Event('change'));var m=d.querySelector('[data-engineer-application-mode]');m.value='manual';m.dispatchEvent(new Event('change'));d.querySelector('[data-engineer-tune]').click();return m.value;" );
      await pollValue("PI candidate", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("PI candidate ready"), 10_000);
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      await pollValue("PI actual Verify", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Verification passed"), 30_000);
      await inner("d.querySelector('[data-engineer-apply]').click();return 'review';");
      await pollValue("PI Manual review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'apply';");
      await pollValue("PI Manual observed", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("staged in Manual and observed"), 15_000);
      const parameters = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0 });
      await inner("d.querySelector('[data-engineer-rollback]').click();return 'rollback review';");
      await pollValue("PI rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'rollback';");
      await pollValue("PI rollback observed", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Previous values restored and observed"), 15_000);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'visible';");
        await new Promise(resolve => setTimeout(resolve, 250));
        captureRenderedWorkbench(screenshot);
      }
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test(`runs the rendered ${AUTO_FAMILY} Auto commissioning and rollback journey against actual ST`, async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(120_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const theme = process.env.TRUST_CONTROL_STUDIO_RENDERED_THEME?.trim();
    if (theme) await vscode.workspace.getConfiguration("workbench").update("colorTheme", theme, vscode.ConfigurationTarget.Global);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const modelPath = path.join(project, "pi-auto-model.json");
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        source: "provided", validation: "validated", evidence: ["fixture"], revision: "pi-auto-model",
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] }));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
        scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await pollValue("PID Auto engineer workflow", () => inner("return d.querySelector('[data-engineer-workflow]')?.dataset.ready ?? '';"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      await connection.send("DOM.enable", {}, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      assert.ok(input, "Supply model exposes the real file input");
      await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: input.backendNodeId }, session);
      await pollValue("selected typed controller model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Model ready"), 10_000);
      await inner(`var f=d.querySelector('[data-engineer-family]');f.value='${AUTO_FAMILY}';f.dispatchEvent(new Event('change'));var m=d.querySelector('[data-engineer-application-mode]');m.value='auto';m.dispatchEvent(new Event('change'));return m.value;`);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-application-mode]').value;"), "auto");
      if (CANCEL_AUTO) await inner("d.querySelector('[data-engineer-duration]').value='20';d.querySelector('[data-engineer-duration]').dispatchEvent(new Event('change'));return 'long duration';");
      await inner("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      await pollValue(`${AUTO_FAMILY} Auto candidate`, () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => AUTO_FAMILY === "PI" ? /PI candidate ready|typed Auto initialization/.test(value) : AUTO_FAMILY === "PID" ? /PID candidate ready|Constrained PID candidate/.test(value) : AUTO_FAMILY === "P" ? value.includes("Candidate ready") : /simplest (?:model-screened|passing) candidate/.test(value), 10_000);
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      await pollValue("PI Auto actual Verify", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Verification passed"), 30_000);
      await inner("d.querySelector('[data-engineer-apply]').click();return 'review';");
      const review = await pollValue("PI Auto review", () => inner("const status=d.querySelector('[data-engineer-status]').textContent;return !d.querySelector('[data-engineer-confirm]').hidden||status.includes('failed')?status:'';"), value => value !== "", 10_000);
      assert.ok(!review.includes("failed"), review);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'commission';");
      if (CANCEL_AUTO) {
        await new Promise(resolve => setTimeout(resolve, 150));
        await inner("d.querySelector('[data-engineer-cancel]').click();return 'cancel';");
        const cancelled = await pollValue("PID Auto cancellation", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /cancelled.*Recovery confirmed|Auto commissioning failed/.test(value), 30_000);
        assert.match(cancelled, /cancelled.*Recovery confirmed/i);
        const recovered = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
        assert.deepStrictEqual((await recovered.client.status()).values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
        assert.strictEqual(await pollValue("PID Auto cancellation Manual mode", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 5_000), "Manual");
        return;
      }
      await pollValue("PI Auto commissioning", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /PID Auto commissioning passed|failed/.test(value), 30_000);
      const commissioned = await inner("return d.querySelector('[data-engineer-status]').textContent;");
      assert.match(commissioned, /PID Auto commissioning passed/);
      const parameters = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": (AUTO_FAMILY === "automatic" || AUTO_FAMILY === "P") ? 11.5 : 1.5, "Main.Ki": (AUTO_FAMILY === "automatic" || AUTO_FAMILY === "P") ? 0 : 1.5, "Main.Kd": AUTO_FAMILY === "PID" ? 0.075 : 0 });
      assert.strictEqual(await inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), "Auto");
      const screenshot = process.env.TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT;
      if (screenshot) {
        await inner("d.querySelector('[data-engineer-status]').scrollIntoView({block:'center'});return 'show Auto commissioning';");
        await new Promise(resolve => setTimeout(resolve, 250));
        captureRenderedWorkbench(screenshot);
      }
      await inner("d.querySelector('[data-engineer-rollback]').click();return 'rollback review';");
      await pollValue("PI Auto rollback review", () => inner("return String(!d.querySelector('[data-engineer-confirm]').hidden);"), value => value === "true", 10_000);
      await inner("d.querySelector('[data-engineer-confirm]').click();return 'rollback';");
      await pollValue("PI Auto rollback", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Previous values restored|failed/.test(value), 15_000);
      assert.match(await inner("return d.querySelector('[data-engineer-status]').textContent;"), /Previous values restored/);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
      assert.strictEqual(await pollValue("PI Auto rollback Manual mode", () => inner("return d.querySelector('[data-control-studio-controller-mode]').textContent;"), value => value === "Manual", 5_000), "Manual");
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("retains a failed actual-ST PD alternative and blocks Apply", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_ACTUAL_PID !== "1") this.skip();
    this.timeout(90_000);
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
    assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15_000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    try {
      const modelPath = path.join(project, "pd-model.json");
      fs.writeFileSync(modelPath, JSON.stringify({ kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        source: "provided", validation: "validated", evidence: ["fixture"], revision: "pd-model", inputs: [channel("u", "V")], outputs: [channel("y", "m")] }));
      await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
      const session = await attachControlStudio(connection, port);
      const inner = (body: string) => evaluateInWebview(connection, session, body);
      await inner("d.querySelector('[data-engineer-supply]').click();return 'supply';");
      await connection.send("DOM.enable", {}, session);
      const tree = await connection.send("DOM.getFlattenedDocument", { depth: -1, pierce: true }, session);
      const input = tree.nodes.find((node: any) => node.nodeName === "INPUT" && node.attributes?.includes("general-model-file"));
      assert.ok(input); await connection.send("DOM.setFileInputFiles", { files: [modelPath], backendNodeId: input.backendNodeId }, session);
      await pollValue("PD model", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Model ready"), 10_000);
      await inner("var f=d.querySelector('[data-engineer-family]');f.value='PD';f.dispatchEvent(new Event('change'));var m=d.querySelector('[data-engineer-application-mode]');m.value='manual';m.dispatchEvent(new Event('change'));d.querySelector('[data-engineer-tune]').click();return 'tune';");
      await pollValue("PD candidate", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("Constrained PD candidate"), 10_000);
      await inner("d.querySelector('[data-engineer-verify]').click();return 'verify';");
      const failed = await pollValue("PD actual failure", () => inner("return d.querySelector('[data-engineer-status]').textContent;"), value => /Verification Fail|failed/.test(value), 30_000);
      assert.match(failed, /Verification Fail/);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true");
      const parameters = await require("../../controlStudio/runtimeParameterConnection").connectRuntimeParameters(loop);
      assert.deepStrictEqual((await parameters.client.status()).values, { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 });
    } finally { connection.close(); await runtimeLifecycleService.stopRuntime(); }
  });

  test("creates an unapplied PI candidate from a typed PID law", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "PI", referenceChange: 1, maximumError: 0.1,
          settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" } }, scope,
      async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PI");
      assert.deepStrictEqual(result?.values, { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0 });
      assert.match(String(result?.reason), /PI candidate ready|manual staging/i);
    } finally { session.dispose(); }
  });

  test("creates declared PD and PID alternatives from the same typed first-order adapter", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const scope = { projectId: loop.projectId, editorSessionId: "pid-alternatives", loopId: loop.id, loopRevision: loop.revision };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const posts: any[] = [];
    const tune = (family: "PD" | "PID") => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: family,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family, referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2,
        durationSeconds: 4, applicationMode: "manual" } }, scope, async value => { posts.push(value); });
    try {
      await tune("PD");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      assert.strictEqual(posts[posts.length - 1]?.design?.candidate?.structure, "PD");
      assert.deepStrictEqual(posts[posts.length - 1]?.values, { "Main.Kp": 1.5, "Main.Ki": 0, "Main.Kd": 0.075 });
      assert.match(String(posts[posts.length - 1]?.reason), /PD candidate ready|constrained PD/i);
      await tune("PID");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      assert.strictEqual(posts[posts.length - 1]?.design?.candidate?.structure, "PID");
      assert.deepStrictEqual(posts[posts.length - 1]?.values, { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0.075 });
      assert.match(String(posts[posts.length - 1]?.reason), /PID candidate ready|constrained PID/i);
    } finally { session.dispose(); }
  });

  test("keeps an automatically recommended proportional controller eligible for Auto commissioning without a PID state adapter", async () => {
    const loop = proportionalLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order-p", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "p-automatic", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-p",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "P");
      assert.match(String(result?.reason), /P is the simplest (?:model-screened|passing) candidate/i);
    } finally { session.dispose(); }
  });

  test("switches a typed PID controller from PI to explicit P with zero integral and derivative gains", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "explicit-p", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "explicit-p", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const tune = async (family: string) => session.handle({ schemaVersion: 1,
      kind: "control-studio.engineer.tune", messageId: family, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family, applicationMode: "manual", referenceChange: 1, maximumError: 0.1,
        settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 } }, scope,
      async value => { posts.push(value); });
    try {
      await tune("PI");
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      await tune("P");
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result.design.candidate.structure, "P");
      assert.strictEqual(result.values["Main.Ki"], 0);
      assert.strictEqual(result.values["Main.Kd"], 0);
      assert.strictEqual(session.snapshotStudy()?.family, "P");
    } finally { session.dispose(); }
  });

  test("reopens a pinned P gain on a PID loop without restoring Apply authority", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "pinned-p", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const reopened = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pin-p", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const send = (owner: EngineerWorkflowSession, action: string, payload: Record<string, unknown>) =>
      owner.handle({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await send(session, "tune", { family: "P", applicationMode: "manual", gainMode: "manual", proportionalGain: 12,
        referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4 });
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate");
      const saved = session.snapshotStudy();
      assert.doesNotThrow(() => reopened.restoreStudy(saved), "a valid PID proportional pin must reopen");
      assert.deepStrictEqual(reopened.snapshotStudy()?.gainChoice, saved?.gainChoice);
      assert.strictEqual(reopened.snapshotStudy()?.family, "P");
      await send(reopened, "apply", {});
      assert.strictEqual(posts[posts.length - 1]?.status, "failed");
      assert.match(String(posts[posts.length - 1]?.reason), /verify|mandatory scenario/i);
      const incompatible = structuredClone(saved!);
      incompatible.gainChoice!.pin!.unit = "wrong-unit";
      assert.throws(() => reopened.restoreStudy(incompatible), /does not match/i);
      assert.deepStrictEqual(reopened.snapshotStudy()?.gainChoice, saved?.gainChoice);
    } finally { session.dispose(); reopened.dispose(); }
  });

  test("routes every main-workflow failure to its owning corrective operation", async () => {
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const payload = { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
      settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" };
    const run = async (loop: any, selectedModel: any, values: Record<string, unknown>) => {
      const session = new EngineerWorkflowSession(loop, () => selectedModel, () => undefined);
      const scope = { projectId: loop.projectId, editorSessionId: `workflow-route-${Math.random()}`, loopId: loop.id, loopRevision: loop.revision };
      const posts: any[] = [];
      try {
        await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: scope.editorSessionId,
          projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
          expectedLoopRevision: scope.loopRevision, payload: { ...payload, ...values } }, scope,
        async value => { posts.push(value); });
        return posts[posts.length - 1];
      } finally { session.dispose(); }
    };

    const missingModel = await run(proportionalLoop(), undefined, {});
    assert.deepStrictEqual(missingModel?.failure, { cause: "model-or-data", nextAction: "find-model" });

    const invalidSettings = await run(proportionalLoop(), model, { referenceChange: 0 });
    assert.deepStrictEqual(invalidSettings?.failure, { cause: "settings", nextAction: "retune" });

    const missingPidAdapter = await run(pidLoop(), model, { family: "PI", applicationMode: "auto" });
    assert.deepStrictEqual(missingPidAdapter?.failure, { cause: "controller-capability", nextAction: "choose-controller" });

    const constrained = proportionalLoop();
    constrained.parameterBinding.parameters[0].maximum = 0.1;
    const infeasibleTask = await run(constrained, model, {});
    assert.deepStrictEqual(infeasibleTask?.failure, { cause: "task-infeasible", nextAction: "revise-requirements" });
  });

  test("recommends P first for a typed PID adapter until the declared task proves an added term necessary", () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    try {
      const result = session.modelChanged();
      assert.strictEqual(result.status, "model-ready");
      assert.match(String(result.reason), /Choose and tune a compatible controller/i);
      assert.match(String(result.reason), /PI, PD and PID|P, PI, PD, PID/i);
    } finally { session.dispose(); }
  });

  test("blocks unvalidated or channel-incompatible supplied models before recommendation", () => {
    const base: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "supplied", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const rejected = (model: any) => {
      const session = new EngineerWorkflowSession(proportionalLoop(), () => model, () => undefined);
      try { return session.modelChanged(); } finally { session.dispose(); }
    };
    const unvalidated = rejected({ ...base, validation: "unvalidated", evidence: [] });
    assert.strictEqual(unvalidated.status, "unavailable");
    assert.deepStrictEqual(unvalidated.failure, { cause: "model-or-data", nextAction: "find-model" });
    assert.match(String(unvalidated.reason), /independent validation/i);
    assert.strictEqual((unvalidated.model as any)?.revision, "supplied", "The blocked model remains available for inspection");
    assert.strictEqual((unvalidated.modelAnalysis as any)?.model.revision, "supplied", "Model inspection is independent of controller qualification");

    const wrongOutputUnit = rejected({ ...base, revision: "wrong-output-unit", outputs: [channel("y", "°C")] });
    assert.strictEqual(wrongOutputUnit.status, "unavailable");
    assert.deepStrictEqual(wrongOutputUnit.failure, { cause: "model-or-data", nextAction: "find-model" });
    assert.match(String(wrongOutputUnit.reason), /output(?:\s+\w+)?\s+unit/i);

    const multivariable = rejected({ ...base, revision: "mimo", inputs: [channel("u1", "V"), channel("u2", "V")] });
    assert.strictEqual(multivariable.status, "model-ready");
    assert.strictEqual(multivariable.failure, undefined);
    assert.match(String(multivariable.reason), /multivariable|channel|compatible/i);
  });

  test("subjects a model returned by Find to the same compatibility gate as Supply", async () => {
    const loop = proportionalLoop();
    let selected: any;
    const search = require("../../controlStudio/runtimeSearchConnection");
    const automatic = require("../../controlStudio/automaticIdentification");
    const oldSearch = search.connectRuntimeSearch;
    const oldRun = automatic.AutomaticIdentification.prototype.run;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => ({}) });
    automatic.AutomaticIdentification.prototype.run = async function () { return { accepted: {
      model: { kind: "first_order_dead_time", gain: 1, timeConstantSeconds: 1, deadTimeSeconds: 0, stable: true, normalizedError: 0 },
      fit: { points: [], warnings: [], operatingRange: { inputMinimum: 0, inputMaximum: 1, outputMinimum: 0, outputMaximum: 1 } },
      evidence: "Identified from simulation data", review: {},
    }, runs: [] }; };
    const session = new EngineerWorkflowSession(loop, () => selected, () => {
      selected = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
        revision: "unvalidated-find", source: "fitted", validation: "unvalidated", evidence: [],
        inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    });
    const scope = { projectId: loop.projectId, editorSessionId: "find-compatibility", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.find", messageId: "find-compatibility",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.deepStrictEqual(result?.failure, { cause: "model-or-data", nextAction: "find-model" });
      assert.match(String(result?.reason), /independent validation/i);
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      automatic.AutomaticIdentification.prototype.run = oldRun;
    }
  });

  test("keeps a legal identified MIMO model on Identify while Tune stays gated for the scalar PID target", async () => {
    const loop = pidLoop();
    const mimoModel: any = {
      kind: "discrete-state-space", revision: "identified-mimo", source: "fitted", validation: "validated", evidence: ["fit", "independent"],
      inputs: [channel("u1", "V"), channel("u2", "V")], outputs: [channel("y1", "m"), channel("y2", "m")],
      states: [channel("x1", "m")], sampleTimeSeconds: loop.controller.taskPeriodMs / 1000, delaySeconds: 0,
      a: [[0.5]], b: [[0.2, 0.1]], c: [[1], [1]], d: [[0, 0], [0, 0]],
    };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const automatic = require("../../controlStudio/automaticIdentification");
    const oldSearch = search.connectRuntimeSearch;
    const oldRun = automatic.AutomaticIdentification.prototype.run;
    let selected: any;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => ({ project_revision: "project" }) });
    automatic.AutomaticIdentification.prototype.run = async function () { return { model: mimoModel, runs: [] }; };
    const session = new EngineerWorkflowSession(loop, () => selected, () => assert.fail("MIMO must use the general-model handoff"),
      undefined, undefined, undefined, value => { selected = value; });
    const scope = { projectId: loop.projectId, editorSessionId: "mimo-identify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const message = (action: string) => ({ schemaVersion: 1, kind: `control-studio.engineer.${action}`, messageId: action,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
      expectedLoopRevision: scope.loopRevision, payload: {} });
    try {
      await session.handle(message("find"), scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result.status, "model-ready", "Identify must present the accepted model even when the current controller cannot tune it");
      assert.deepStrictEqual(result.model, mimoModel, "the accepted MIMO model must remain available for Identify review");
      assert.strictEqual(result.stageEvidence?.hasModel, false, "Tune must remain gated until a compatible declared controller target is selected");

      await session.handle(message("tune"), scope, async value => { posts.push(value); });
      assert.strictEqual(posts[posts.length - 1].status, "failed", "Tune must retain the controller/model compatibility boundary");
      assert.match(String(posts[posts.length - 1].reason), /multivariable|channel|compatible/i);
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      automatic.AutomaticIdentification.prototype.run = oldRun;
    }
  });

  test("routes an unavailable declared Find method to controller capability before a runtime write", async () => {
    const loop = proportionalLoop();
    loop.experiment = { execution: "control-studio", kind: "prbs", amplitude: 1, durationSeconds: 10, seed: 7 };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const oldSearch = search.connectRuntimeSearch;
    let requests = 0;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => { requests += 1; return {}; } });
    const session = new EngineerWorkflowSession(loop, () => undefined, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "find-method", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.find", messageId: "find-method",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: {} }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.deepStrictEqual(result?.failure, { cause: "model-or-data", nextAction: "find-model" });
      assert.ok(requests > 0, "the implemented PRBS method may dispatch bounded acquisition before reporting retained-data failure");
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
    }
  });

  test("retains automatic P PI PD and PID attempts and chooses the simplest passing typed candidate", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-automatic", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
        payload: { family: "automatic", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "P");
      assert.deepStrictEqual(result?.values, { "Main.Kp": 11.5, "Main.Ki": 0, "Main.Kd": 0 });
      assert.deepStrictEqual(result?.attempts.map((attempt: any) => [attempt.structure, attempt.status]), [
        ["P", "Feasible"], ["PI", "Feasible"], ["PD", "Rejected"], ["PID", "Feasible"],
      ]);
      assert.match(String(result?.reason), /simplest (?:model-screened|passing) candidate.*P/i);
    } finally { session.dispose(); }
  });

  test("automatically selects PI when the declared task proves P cannot meet the gain bounds", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "automatic-pi", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-pi",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.001, settlingBand: 0.0001, settlingTimeSeconds: 2, durationSeconds: 4,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PI");
      assert.deepStrictEqual(result?.values, { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0 });
      assert.deepStrictEqual(result?.attempts.map((attempt: any) => [attempt.structure, attempt.status]), [
        ["P", "Rejected"], ["PI", "Feasible"], ["PD", "Rejected"], ["PID", "Feasible"],
      ]);
      assert.match(String(result?.reason), /PI is the simplest.*P/i);
    } finally { session.dispose(); }
  });

  test("rejects an automatic candidate that exceeds declared actuator output limits before Verify", async () => {
    const loop = proportionalLoop();
    loop.limits = { outputMinimum: -0.5, outputMaximum: 0.5 };
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "automatic-output-limit", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "automatic-output-limit",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "failed", String(result?.reason));
      assert.deepStrictEqual(result?.failure, { cause: "task-infeasible", nextAction: "revise-requirements" });
      assert.match(String(result?.reason), /output limit/i);
    } finally { session.dispose(); }
  });

  test("routes a valid unsupported model structure to controller selection rather than retuning", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 2, 1], delaySeconds: 0,
      revision: "second-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "unsupported-structure", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    try {
      await session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "unsupported-structure",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId,
        expectedLoopRevision: scope.loopRevision, payload: { family: "automatic", referenceChange: 1,
          maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4,
          applicationMode: "manual" } }, scope, async value => { posts.push(value); });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "candidate", String(result?.reason));
      assert.strictEqual(result?.design?.candidate?.structure, "PD");
      assert.match(String(result?.reason), /candidate|actual-ST/i);
    } finally { session.dispose(); }
  });

  test("permits Auto PI tuning only when the loop declares its typed initialization adapter", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = {
      kind: "parallel-pid-v1",
      integralState: { symbol: "Main.Integral", iecType: "LREAL", unit: "V", minimum: -20, maximum: 20 },
      derivativeState: { symbol: "Main.PreviousError", iecType: "LREAL", unit: "m", minimum: -20, maximum: 20 },
      proportionalSetpointWeight: 1,
      derivativeSetpointWeight: 1,
      maximumSnapshotAgeMs: 100,
    };
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const tune = (value: any) => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi-auto",
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision,
      payload: { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto", ...value } }, scope,
    async value => { posts.push(value); });
    try {
      await tune({});
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate", "the declared adapter makes an unapplied PI Auto candidate eligible for Verify");
      assert.match(String(posts[posts.length - 1]?.reason), /PI candidate ready|Auto initialization/i);

      const missing = pidLoop();
      const blocked = new EngineerWorkflowSession(missing, () => model, () => undefined);
      const blockedPosts: any[] = [];
      await blocked.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: "pi-auto-missing",
        projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: missing.id, expectedLoopRevision: missing.revision,
        payload: { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
          settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" } },
      { ...scope, loopId: missing.id, loopRevision: missing.revision }, async value => { blockedPosts.push(value); });
      assert.strictEqual(blockedPosts[blockedPosts.length - 1]?.status, "failed");
      assert.match(String(blockedPosts[blockedPosts.length - 1]?.reason), /typed PI\/PID initialization adapter/i);
      blocked.dispose();
    } finally { session.dispose(); }
  });

  test("verifies the declared PI retained states and first output before Auto commissioning", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const requests: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => { throw new Error("Verify must use its existing client boundary"); } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
    }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any) {
      requests.push({ request, requirements });
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto-verify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "auto" });
      await invoke("verify", { applicationMode: "auto" });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "verified");
      const configuredPlantRequests = requests.filter(value => value.request.simulation_override === undefined);
      assert.strictEqual(configuredPlantRequests.length, 3, "Auto PI must execute baseline, candidate and retained-state initialization records; prediction requests are separate");
      const initialization = configuredPlantRequests[2];
      assert.deepStrictEqual(initialization.request.initial_values, {
        "Main.Manual": 1, "Main.SP": 0, "Main.MV": 0, "Main.ManualCommand": 0,
        "Main.Integral": 0, "Main.PreviousError": 0,
      });
      assert.deepStrictEqual(initialization.request.events, [
        { at_ms: 60, symbol: "Main.SP", value: 0 },
        { at_ms: 60, symbol: "Main.Manual", value: 0 },
      ]);
      assert.ok(initialization.request.observations.some((value: any) => value.symbol === "Main.Integral"));
      assert.ok(initialization.request.observations.some((value: any) => value.symbol === "Main.PreviousError"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "transfer-continuity"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "integral-state"));
      assert.ok(initialization.requirements.some((value: any) => value.id === "derivative-state"));
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("routes a failed actual-ST verification back to Tune with its retained evidence", async () => {
    const loop = proportionalLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch;
    const oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => ({}) });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1 }, program_revision: "program", binding_revision: "binding",
    }) } });
    const calls: any[] = [];
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any, requirements: any[]) {
      calls.push(request);
      const configuredCall = calls.filter(value => value.simulation_override === undefined).length;
      return { projectRevision: "project", request, run: { id: `run-${calls.length}` }, evaluation: {
        status: configuredCall === 1 ? "Pass" : "Fail",
        requirements: requirements.map((requirement: any) => ({ id: requirement.id,
          status: configuredCall > 1 && ["error", "settling"].includes(requirement.kind) ? "Fail" : "Pass" })),
      } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "verify-failure-route", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId,
      loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "P", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify", { applicationMode: "manual" });
      const result = posts[posts.length - 1];
      assert.strictEqual(result?.status, "verification-failed");
      assert.deepStrictEqual(result?.failure, { cause: "settings", nextAction: "retune" });
      assert.match(String(result?.reason), /Next: Tune again/i);
      assert.strictEqual(calls.filter(request => request.simulation_override === undefined).length, 2,
        "the retained baseline and failed candidate must both execute actual-ST requests; prediction requests are separate");
    } finally {
      session.dispose();
      search.connectRuntimeSearch = oldSearch;
      parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("reviews Auto PI against the declared parallel PID state-transfer binding", async () => {
    const loop = pidLoop();
    loop.controller.pidInitialization = pidInitialization();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const runtime = require("../../controlStudio/runtimeConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldRuntime = runtime.bindRuntimeConnection, oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const observed = { values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
      active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null };
    const bindings: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error(`unexpected search operation ${operation}`);
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => observed,
      review: async (values: any) => ({ observed, values }),
    } });
    runtime.bindRuntimeConnection = async () => ({ source: "simulation", request: async (operation: string, payload: any) => {
      assert.strictEqual(operation, "control.transfer.status");
      bindings.push(payload.binding);
      return { program_revision: "program", binding_revision: "transfer", status: "idle", reason: null, transaction: null,
        supported_tracking: ["parallel-pid-v1"], values: { "Main.Manual": true, "Main.SP": 0, "Main.ManualCommand": 0,
          "Main.Integral": 0, "Main.PreviousError": 0, "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0, "Main.PV": 0, "Main.MV": 0 } };
    } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-auto-review", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      const automatic = { applicationMode: "auto" as const };
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, ...automatic });
      await invoke("verify", automatic);
      await invoke("apply", automatic);
      assert.strictEqual(posts[posts.length - 1]?.status, "review");
      assert.deepStrictEqual(bindings[0].states.map((state: any) => state.symbol),
        ["Main.SP", "Main.ManualCommand", "Main.Integral", "Main.PreviousError"]);
      assert.deepStrictEqual(bindings[0].guards.map((guard: any) => guard.symbol),
        ["Main.Kp", "Main.Ki", "Main.Kd", "Main.PV"]);
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      runtime.bindRuntimeConnection = oldRuntime; verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("verifies the full mapped PI record through the existing verification client", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const requests: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async () => { throw new Error("Verify must use its existing client boundary"); } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: { status: async () => ({
      values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
    }) } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      requests.push(request);
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "pid-verify", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId,
      editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02,
        settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify");
      assert.strictEqual(posts[posts.length - 1]?.status, "verified");
      assert.deepStrictEqual(requests[1].parameter_overrides.map((value: any) => [value.symbol, value.value]),
        [["Main.Kp", 1.5], ["Main.Ki", 1.5], ["Main.Kd", 0]]);
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("reviews and manually applies the full verified PI record", async () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"], normalizedError: 0.01,
      inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const search = require("../../controlStudio/runtimeSearchConnection");
    const parameters = require("../../controlStudio/runtimeParameterConnection");
    const verification = require("../../controlStudio/verificationClient");
    const oldSearch = search.connectRuntimeSearch, oldParameters = parameters.connectRuntimeParameters;
    const oldVerify = verification.RuntimeVerificationClient.prototype.verify;
    const observed = { values: { "Main.Kp": 1, "Main.Ki": 0, "Main.Kd": 0 }, program_revision: "program", binding_revision: "binding",
      active_revision: "active", status: "applied", previous_revision: null, previous_values: null, reason: null };
    const reviewed: any[] = []; const applied: any[] = []; const owners: any[] = [];
    search.connectRuntimeSearch = async () => ({ source: "simulation", request: async (operation: string) => {
      if (operation === "control.simulation.run.status") return { ready: true, project_revision: "project", execution: "isolated-simulation", source_basis: "loaded-runtime-sources" };
      throw new Error("existing client seam");
    } });
    parameters.connectRuntimeParameters = async () => ({ source: "simulation", client: {
      status: async () => observed,
      review: async (values: any) => { reviewed.push(values); return { observed, values }; },
      apply: async (review: any) => { applied.push(review.values); return { ...observed, values: review.values, active_revision: "pi", previous_revision: "active", previous_values: observed.values }; },
    } });
    verification.RuntimeVerificationClient.prototype.verify = async function (request: any) {
      return { projectRevision: "project", request, run: {}, evaluation: { status: "Pass" } };
    };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined, undefined,
      async owner => { owners.push(owner); });
    const scope = { projectId: loop.projectId, editorSessionId: "pid-apply", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (action: string, payload: Record<string, unknown> = {}) => session.handle({ schemaVersion: 1,
      kind: `control-studio.engineer.${action}`, messageId: action, projectId: scope.projectId, editorSessionId: scope.editorSessionId,
      loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope, async value => { posts.push(value); });
    try {
      await invoke("tune", { family: "PI", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual" });
      await invoke("verify");
      await invoke("apply", { applicationMode: "manual" });
      await invoke("confirm");
      const expected = { "Main.Kp": 1.5, "Main.Ki": 1.5, "Main.Kd": 0 };
      assert.deepStrictEqual(reviewed, [expected]);
      assert.deepStrictEqual(applied, [expected]);
      assert.deepStrictEqual(owners.map(owner => owner.operation), ["apply"]);
      assert.strictEqual(posts[posts.length - 1]?.status, "applied");
    } finally {
      session.dispose(); search.connectRuntimeSearch = oldSearch; parameters.connectRuntimeParameters = oldParameters;
      verification.RuntimeVerificationClient.prototype.verify = oldVerify;
    }
  });

  test("restores a typed PID family as historical study evidence", () => {
    const loop = pidLoop();
    const model: any = { kind: "transfer-function", numerator: [1], denominator: [1, 1], delaySeconds: 0,
      revision: "first-order", source: "provided", validation: "validated", evidence: ["bench"],
      normalizedError: 0.01, inputs: [channel("u", "V")], outputs: [channel("y", "m")] };
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    try {
      session.restoreStudy({ selectedModel: model, family: "PID", applicationMode: "auto",
        task: { referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 1, durationSeconds: 2 }, campaigns: [] });
      assert.strictEqual(session.snapshotStudy()?.family, "PID");
    } finally { session.dispose(); }
  });

  test("adopts inbound state design preferences before parsing the draft", async () => {
    const loop = stateDesignLoop();
    const model: any = stateDesignModel(loop);
    const session = new EngineerWorkflowSession(loop, () => model, () => undefined);
    const scope = { projectId: loop.projectId, editorSessionId: "state-draft-order", loopId: loop.id, loopRevision: loop.revision };
    const posts: any[] = [];
    const invoke = (payload: Record<string, unknown>) => session.handle({ schemaVersion: 1, kind: "control-studio.engineer.tune", messageId: `tune-${posts.length}`,
      projectId: scope.projectId, editorSessionId: scope.editorSessionId, loopId: scope.loopId, expectedLoopRevision: scope.loopRevision, payload }, scope,
    async value => { posts.push(value); });
    try {
      session.restoreStudy({ campaigns: [], tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[" } } } as any);
      await invoke({ family: "LQR", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual",
        tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[1]]", inputWeights: "[[1]]", solverMode: "infinite" } } });
      assert.strictEqual(posts[posts.length - 1]?.status, "candidate", String(posts[posts.length - 1]?.reason));
      await invoke({ family: "LQR", referenceChange: 1, maximumError: 0.1, settlingBand: 0.02, settlingTimeSeconds: 2, durationSeconds: 4, applicationMode: "manual",
        tuningPreferences: { method: "state-design", stateDesignDraft: { stateWeights: "[[" } } });
      assert.strictEqual(posts[posts.length - 1]?.status, "failed", "A malformed raw state draft must be rejected before design");
    } finally { session.dispose(); }
  });
});

function stateDesignLoop(): any {
  const loop = pidLoop();
  loop.controller = { ...loop.controller, type: "State feedback", law: undefined, stateFeedback: {
    family: "LQR", convention: "sampled-deviation-negative-feedback", gainSymbols: [["Main.Kp"]], referenceGainSymbols: [["Main.N"]], stateWeights: [[1]], inputWeights: [[1]],
  } };
  loop.parameterBinding = { ...loop.parameterBinding, parameters: [
    { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "state feedback gain", minimum: -100, maximum: 100 },
    { symbol: "Main.N", iec_type: "LREAL", unit: "V/m", convention: "reference gain", minimum: -100, maximum: 100 },
  ] };
  return loop;
}

function stateDesignModel(loop: any): any {
  const unit = loop.signals.processVariable.unit;
  return { kind: "continuous-state-space", revision: "state-draft-order-model", source: "provided", validation: "validated", evidence: ["independent"], delaySeconds: 0,
    inputs: [{ id: "u", unit: loop.signals.manipulatedVariable.unit, operatingRange: loop.signals.manipulatedVariable.range }],
    outputs: [{ id: "y", unit, operatingRange: loop.signals.processVariable.range }], states: [{ id: "x", unit, operatingRange: loop.signals.processVariable.range }],
    a: [[-1]], b: [[1]], c: [[1]], d: [[0]], operatingPoint: { inputs: [0], outputs: [0] } };
}
