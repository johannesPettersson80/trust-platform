import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/*
 * The loop of these journeys is served by the engine, but the recovery states they project are the page's own: no engine answer
 * carries an unconfirmed cancellation (`cancellationUnconfirmed`), a retried manual recovery (`recovery-required` with
 * `retryManualRecovery`) or a reconciled recovery (`recoveryResolved`); webview/engineApplication.ts sends none of them, and the engine
 * reports an unconfirmed cancellation as a failure without a next action (crates/trust-control/src/identification/connected_run.rs
 * restoration_unconfirmed). Those states keep the TypeScript route's envelope (hostUpdate). A recovery action that reaches the engine
 * is answered on the engine route (engineFailure).
 */
suite("Control Studio three-page recovery rendered projection", () => {
  test("shows Identify cancellation recovery while keeping page navigation usable", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      // The engine route does not send this state (see above).
      await inner(hostUpdate({ status: "failed", cancellationUnconfirmed: true,
        reason: "Cancellation still needs confirmation." }));
      await pollValue("visible cancellation recovery", () => inner("const a=d.querySelector('[data-studio-recovery-action]');return String(Boolean(a&&!a.hidden&&!a.disabled));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-recovery-action]').textContent;"), "Confirm cancellation");
      assert.strictEqual(await inner("const a=d.querySelector('[data-studio-recovery-action]'),r=a?.getBoundingClientRect();return String(Boolean(r&&r.top>=0&&r.left>=0&&r.bottom<=d.defaultView.innerHeight&&r.right<=d.defaultView.innerWidth));"), "true", "Recovery action must be fully visible in the viewport");
      assert.match(await inner("return d.querySelector('[data-studio-message=model]').textContent;"), /Cancellation still needs confirmation/);
      assert.strictEqual(await inner("return String(Array.from(d.querySelectorAll('[data-engineer-stage-tab]')).every(tab=>!tab.disabled));"), "true");
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'navigation remains usable';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-recovery-action]').checkVisibility());"), "true", "The recovery action follows page navigation");
      assert.match(await inner("return d.querySelector('[data-studio-message=tune]').textContent;"), /Cancellation still needs confirmation/);
    }, "recovery-navigation");
  });

  test("restores the same retry action after a generic recovery failure", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      // The engine route does not send this state (see above).
      await inner(hostUpdate({ status: "recovery-required", retryManualRecovery: true,
        reason: "Reconcile the retained operation." }));
      await pollValue("visible retry recovery", () => inner("const a=d.querySelector('[data-studio-recovery-action]');return String(Boolean(a&&!a.hidden&&!a.disabled));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-recovery-action]').textContent;"), "Retry recovery");
      assert.strictEqual(await inner("const a=d.querySelector('[data-studio-recovery-action]'),r=a?.getBoundingClientRect();return String(Boolean(r&&r.top>=0&&r.left>=0&&r.bottom<=d.defaultView.innerHeight&&r.right<=d.defaultView.innerWidth));"), "true", "Retry action must be fully visible in the viewport");
      await inner("d.querySelector('[data-studio-recovery-action]').click();return 'retry';");
      // Retry recovery asks the engine for the Restore review (engine/routes/applicationRoute.ts); a refusal reaches the page as the
      // engine's failure of `restore`, which the Apply receiver shows (webview/engineOpened.ts, engineApplication.ts failurePayload).
      await inner(engineFailure("restore", { message: "Reconciliation could not be confirmed.", cause: "runtime", nextAction: "none" }));
      await pollValue("retry recovery restored", () => inner("const a=d.querySelector('[data-studio-recovery-action]');return String(Boolean(a&&!a.hidden&&!a.disabled));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return d.querySelector('[data-studio-recovery-action]').textContent;"), "Retry recovery");
    }, "retry-failure");
  });

  test("clears recovery locks on terminal resolution and does not add a second button for normal Apply review", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      // The engine route does not send these recovery states (see above).
      await inner(hostUpdate({ status: "recovery-required", retryManualRecovery: true,
        reason: "Reconcile the retained operation." }));
      await pollValue("recovery action", () => inner("return String(Boolean(d.querySelector('[data-studio-recovery-action]')));"), value => value === "true", 10000);
      await inner(hostUpdate({ status: "failed", recoveryResolved: true, applicationAction: "restore",
        reason: "Recovery was reconciled." }));
      await pollValue("recovery action cleared", () => inner("return String(!d.querySelector('[data-studio-recovery-action]'));"), value => value === "true", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "false");
      // A review without a confirmation step is the TypeScript route's: the engine's Apply review adds its Confirm Apply button to
      // this area (webview/threePageApplication.ts showReview), so it keeps the TypeScript envelope here.
      await inner(hostUpdate({ status: "review", applicationAction: "apply", reason: "Normal reviewed Apply." }));
      assert.strictEqual(await inner("return String(Array.from(d.querySelectorAll('[data-studio-application] button')).filter(button=>!button.hidden).length);"), "1");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-keep]').hidden);"), "true", "Keep in project waits for a successful Apply");
      assert.strictEqual(await inner("return String(!d.querySelector('[data-studio-recovery-action]'));"), "true");
    }, "resolved-apply-review");
  });
});

async function withStudio(exercise: (inner: (body: string) => Promise<string>) => Promise<void>, capture: string): Promise<void> {
  const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
  const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8"));
  await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
  const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
  const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
  try {
    await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop",
      scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
    const session = await attachControlStudio(connection, port, "", loop.id);
    const inner = (body: string) => evaluateInWebview(connection, session, body);
    await pollValue("three-page recovery handlers", () => inner("return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
    await exercise(inner);
    const screenshot = requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${capture}.png`);
    await captureRenderedWorkbenchCdp(connection, screenshot);
  } finally {
    connection.close();
    await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  }
}

/** Posts a state of the page's own recovery as the TypeScript route sends it: `control-studio.engineering.result` for the engineer stage. */
function hostUpdate(payload: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engineering.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,stage:'engineer',payload:${JSON.stringify(payload)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}

/** Posts the engine's refusal of a method as the host does (engine/enginePanelRoute.ts postFailure): its Failure, in the loop's scope. */
function engineFailure(method: string, failure: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engine.failure',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,method:${JSON.stringify(method)},failure:${JSON.stringify(failure)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}
