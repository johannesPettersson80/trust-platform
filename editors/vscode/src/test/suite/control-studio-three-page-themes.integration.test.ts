import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio approved three-page theme review", () => {
  test("captures every approved page and compact editor at all release themes", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(180000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    const base = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));

    const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT");
    const outputDirectory = `${screenshot.replace(/\.png$/, "")}-three-page-themes`;
    fs.mkdirSync(outputDirectory, { recursive: true });
    const workbench = vscode.workspace.getConfiguration("workbench");
    const windowConfiguration = vscode.workspace.getConfiguration("window");
    const previousTheme = workbench.get<string>("colorTheme");
    const previousZoom = windowConfiguration.get<number>("zoomLevel");
    const themes = [
      ["dark-plus", "Default Dark+"], ["light-plus", "Default Light+"],
      ["high-contrast", "Default High Contrast"], ["high-contrast-light", "Default High Contrast Light"],
    ] as const;
    try {
      await vscode.commands.executeCommand("workbench.action.closeSidebar");
      await vscode.commands.executeCommand("workbench.action.closeAuxiliaryBar");
      for (const [slug, theme, zoom] of [...themes.map(([slug, theme]) => [slug, theme, 0] as const), ["dark-plus-zoom", "Default Dark+", 4] as const]) {
        const loop = structuredClone(base); loop.id += `-theme-${slug}`; loop.parameterBinding.loop_id = loop.id;
        await workbench.update("colorTheme", theme, vscode.ConfigurationTarget.Global);
        await windowConfiguration.update("zoomLevel", zoom, vscode.ConfigurationTarget.Global);
        await vscode.commands.executeCommand("trust-lsp.controlStudio.open", {
          schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop,
        });
        const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
        const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
        try {
          const session = await attachControlStudio(connection, port, "", loop.id);
          const inner = (body: string) => evaluateInWebview(connection, session, body);
          await pollValue(`${theme} host ready`, () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 15000);
          await pollValue("theme applied", () => inner("return d.body.dataset.vscodeThemeId??d.body.dataset.vscodeThemeName;"), name => name === theme || name === theme.replace("Default ", ""), 10000);
          const prefix = path.join(outputDirectory, slug);
          await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
          await inner(hostUpdate({ status: "model-ready", model: { kind: "transfer-function", numerator: [2], denominator: [4, 1], delaySeconds: .5, revision: "theme-model", source: "provided", validation: "validated", evidence: [], inputs: [{id:"u", unit:"%",operatingRange:{minimum:0,maximum:100}}],outputs:[{id:"y",unit:"°C",operatingRange:{minimum:0,maximum:100}}] }, stageEvidence: {hasModel:true,hasCandidate:false,validationPassed:false},
            identificationPresentation: { units: {input:"%",output:"°C"}, predictedResponse: [{timeSeconds:0,measured:20,predicted:20},{timeSeconds:1,measured:20.5,predicted:20.6},{timeSeconds:2,measured:21,predicted:21}] } }));
          await assertViewport(inner, "[data-engineer-find]", "Identify action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-identify.png`);

          await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");

          await inner(hostUpdate({ status: "candidate", family: "PI", availableFamilies: ["P","PI","PD","PID"], values: { "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5 }, plots: {timeResponse:[{timeSeconds:0,currentPv:20,candidatePv:20},{timeSeconds:1,currentPv:20.3,candidatePv:20.8},{timeSeconds:2,currentPv:20.7,candidatePv:21}]}, stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false } }));
          await assertViewport(inner, "[data-engineer-tune]", "Tune action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-tune.png`);
          await inner("const method=d.querySelector('[data-engineer-tuning-method]');method.value='manual';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'manual';");
          await assertViewport(inner, "[data-studio-manual-value]", "Manual controller setting");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-manual-settings.png`);

          await inner("d.querySelector('[data-studio-requirements]').click();return 'requirements';");
          await assertViewport(inner, "[data-studio-editor-done]", "Requirements Done action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-requirements.png`);
          await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'close requirements';");

          await inner("d.querySelector('[data-engineer-stage-tab=model]').click();d.querySelector('[data-studio-limits]').click();return 'limits';");
          await assertViewport(inner, "[data-studio-editor-done]", "Limits Done action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-limits.png`);
          await inner("d.querySelector('[data-studio-editor-cancel]').click();return 'close limits';");

          const run = { channels: [{id:"P1.ProcessValue",unit:"°C"}], samples: [{timeSeconds:0,values:{"P1.ProcessValue":{value:20,quality:"good"}}},{timeSeconds:1,values:{"P1.ProcessValue":{value:21,quality:"good"}}}] };
          await inner(hostUpdate({ status: "verified", candidate: {status:"Pass",run,requirements:[{id:"error",kind:"error",status:"Pass",value:.02,limit:.1,unit:"°C",reason:"Within limit."}],specification:[{id:"error",kind:"error",channel:"P1.ProcessValue",maximum:.1}]}, stageEvidence:{hasModel:true,hasCandidate:true,validationPassed:true} }));
          await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
          await assertViewport(inner, "[data-engineer-apply]", "Apply action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-validate.png`);
        } finally {
          await captureRenderedWorkbenchCdp(connection, path.join(outputDirectory, `${slug}-final.png`));
          connection.close();
          await vscode.commands.executeCommand("workbench.action.closeAllEditors");
        }
      }
    } finally {
      await workbench.update("colorTheme", previousTheme, vscode.ConfigurationTarget.Global);
      await windowConfiguration.update("zoomLevel", previousZoom, vscode.ConfigurationTarget.Global);
    }
  });
});

function hostUpdate(payload: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:${JSON.stringify(payload)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}

async function assertViewport(inner: (body: string) => Promise<string>, selector: string, label: string): Promise<void> {
  const result = JSON.parse(await inner(`const e=d.querySelector(${JSON.stringify(selector)});if(!e)return JSON.stringify({missing:true});e.scrollIntoView({block:'nearest',behavior:'instant'});await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));const r=e.getBoundingClientRect();return JSON.stringify({missing:false,width:r.width,height:r.height,top:r.top,bottom:r.bottom,viewport:d.defaultView.innerHeight});`));
  assert.strictEqual(result.missing, false, `${label} must be rendered`);
  assert.ok(result.width > 0 && result.height > 0, `${label} must have visible dimensions`);
  assert.ok(result.top >= -0.5 && result.bottom <= result.viewport + 0.5, `${label} must remain within the workbench viewport: ${JSON.stringify(result)}`);
}
