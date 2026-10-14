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
        // The engine opens a configured loop from the project's loop files (crates/trust-control-studio/src/loops.rs open) and refuses
        // an identifier no file declares, so each theme's loop is written beside the project's own and removed after its review.
        const loopFile = path.join(project, "control-loops", `theme-${slug}.json`);
        fs.mkdirSync(path.dirname(loopFile), { recursive: true });
        fs.writeFileSync(loopFile, JSON.stringify(loop));
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
          // The engine serves the loop: its open answer gives each stage tab the engine's status (webview/engineShared.ts applyStageStatuses).
          assert.notStrictEqual(await inner("return d.querySelector('[data-engineer-stage-tab=model]').dataset.status ?? '';"), "",
            `${theme}: the engine serves the loop, so its stage tabs show the engine's status`);
          const prefix = path.join(outputDirectory, slug);
          await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
          // The loop is served by the engine, so the page's states are the engine's answers (webview/engineOpened.ts).
          await inner(engineResult("identify", IDENTIFIED));
          await assertViewport(inner, "[data-engineer-find]", "Identify action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-identify.png`);

          await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'tune';");

          // The controller families come from the engine's binding list on open (threePageBindings.ts), not from the Tune answer.
          await inner(engineResult("tune", TUNED));
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

          await inner(engineResult("validate", VALIDATED));
          await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
          await assertViewport(inner, "[data-engineer-apply]", "Apply action");
          await captureRenderedWorkbenchCdp(connection, `${prefix}-validate.png`);
        } finally {
          await captureRenderedWorkbenchCdp(connection, path.join(outputDirectory, `${slug}-final.png`));
          connection.close();
          await vscode.commands.executeCommand("workbench.action.closeAllEditors");
          fs.rmSync(loopFile, { force: true });
        }
      }
    } finally {
      await workbench.update("colorTheme", previousTheme, vscode.ConfigurationTarget.Global);
      await windowConfiguration.update("zoomLevel", previousZoom, vscode.ConfigurationTarget.Global);
    }
  });
});

type StageStatus = "complete" | "ready" | "needs-data";
const STAGE_LABELS: Record<StageStatus, string> = { complete: "Complete", ready: "Ready", "needs-data": "Needs data" };

/** The status of the page's three stages in an engine answer, with the engine's words (crates/trust-control/src/stages.rs). */
function stages(identify: StageStatus, tune: StageStatus, validate: StageStatus): { stage: string; status: StageStatus; label: string }[] {
  return ([["Identify", identify], ["Tune", tune], ["Validate", validate]] as const).map(([stage, status]) => ({ stage, status, label: STAGE_LABELS[status] }));
}

/** The fixture's identification as the engine answers it (IdentifyResult, crates/trust-control/src/messages.rs): 2 e^(-0.5s) / (4s + 1). */
const IDENTIFIED = {
  modelType: "First-order model with delay",
  equation: "G(s) = 2 e^(-0.5s) / (4s + 1)",
  keyNumbers: "Gain 2 °C per %, time constant 4 s, delay 0.5 s",
  provisional: false,
  text: "First-order model with delay\nG(s) = 2 e^(-0.5s) / (4s + 1)\nGain 2 °C per %, time constant 4 s, delay 0.5 s",
  plot: { timeSeconds: [0, 1, 2], traces: [
    { role: "plant-input", symbol: "P1.ManipulatedValue", unit: "%", values: [0, 10, 10] },
    { role: "measurement", symbol: "P1.ProcessValue", unit: "°C", values: [20, 20.5, 21] },
    { role: "prediction", symbol: "y", unit: "°C", values: [20, 20.6, 21] },
  ] },
  recordingId: `rec-${"0".repeat(32)}`,
  stageEvidence: { hasModel: true, hasCandidate: false, validationPassed: false },
  stages: stages("complete", "ready", "needs-data"),
};

/** The fixture's PI candidate as the engine answers Tune (TuneResult): the fields the page shows (webview/engineTune.ts). */
const TUNED = {
  candidate: { applied: false, structure: "PI" },
  settings: { "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5 },
  methodTitle: "Automatic",
  outcome: { status: "meets" },
  comparison: [],
  response: { timeSeconds: [0, 1, 2], traces: [{ role: "measurement", symbol: "P1.ProcessValue", unit: "°C", values: [20, 20.8, 21] }] },
  stageEvidence: { hasModel: true, hasCandidate: true, validationPassed: false },
  stages: stages("complete", "complete", "ready"),
};

/** One run of the fixture's target response with its one requirement, as Validate answers it (RunOutcome, RequirementOutcome). */
function validatedRun(runId: string): Record<string, unknown> {
  return { runId, projectRevision: "theme", evidence: "ST-executed", status: "Pass", requirements: [{
    id: "error", label: "Maximum steady-state error", channel: { role: "Measurement", unit: "°C", symbol: "P1.ProcessValue" }, limitSide: "atMost",
    status: "Pass", value: 0.02, limit: 0.1, unit: "°C", source: "model theme-model", reason: "Within limit.", automatic: false,
  }] };
}

/** The fixture's passed validation as the engine answers it (ValidateResult): Apply is available. */
const VALIDATED = {
  scenarios: [{ id: "target-response", plant: "model theme-model", label: "Target response", current: validatedRun("theme-current"), candidate: validatedRun("theme-candidate") }],
  status: "Pass",
  applyAvailable: true,
  stageEvidence: { hasModel: true, hasCandidate: true, validationHasResult: true, validationPassed: true },
  stages: stages("complete", "complete", "complete"),
  labels: { currentSettings: "Current settings", candidate: "Candidate", candidateOverall: "Candidate overall", automatic: "Automatic" },
  plot: { title: "Measured response", role: "Measurement", unit: "°C", tuned: { name: "Tuned", timeSeconds: [0, 1], values: [20, 21] } },
};

/** Posts one engine answer to the page as the host does (engine/enginePanelRoute.ts): `control-studio.engine.result` in the loop's scope. */
function engineResult(method: string, result: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engine.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,method:${JSON.stringify(method)},result:${JSON.stringify(result)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}

async function assertViewport(inner: (body: string) => Promise<string>, selector: string, label: string): Promise<void> {
  const result = JSON.parse(await inner(`const e=d.querySelector(${JSON.stringify(selector)});if(!e)return JSON.stringify({missing:true});e.scrollIntoView({block:'nearest',behavior:'instant'});await new Promise(resolve=>d.defaultView.requestAnimationFrame(()=>d.defaultView.requestAnimationFrame(resolve)));const r=e.getBoundingClientRect();return JSON.stringify({missing:false,width:r.width,height:r.height,top:r.top,bottom:r.bottom,viewport:d.defaultView.innerHeight});`));
  assert.strictEqual(result.missing, false, `${label} must be rendered`);
  assert.ok(result.width > 0 && result.height > 0, `${label} must have visible dimensions`);
  assert.ok(result.top >= -0.5 && result.bottom <= result.viewport + 0.5, `${label} must remain within the workbench viewport: ${JSON.stringify(result)}`);
}
