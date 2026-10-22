import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/*
 * The loop of these journeys is served by the engine, and the block of an Apply's checkpoint that waits for its Restore is the engine's
 * (specification 35, B6.4, B6.7): its stage statuses hold the stages and name the reason, its offers put Restore previous settings in the
 * Apply position (`primaryAction`), and a failure's next action Restore asks for the Restore review. The answers are posted as the host
 * posts them (engineResult). The page keeps no recovery of its own: a confirmed Restore's stages end the block.
 */
suite("Control Studio three-page recovery rendered projection", () => {
  test("shows the engine's block on the pages and Restore previous settings in the Apply position, while page navigation stays usable", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      await inner(engineResult("apply", unresolvedApply()));
      await pollValue("the block's reason on Validate", () => inner(reasonOf("verify")), value => value === BLOCK_REASON, 10000);
      const apply = "d.querySelector('[data-engineer-apply]')";
      assert.strictEqual(await inner(`return ${apply}.textContent;`), "Restore previous settings");
      assert.strictEqual(await inner(`return String(${apply}.disabled);`), "false");
      assert.strictEqual(await inner(`const r=${apply}.getBoundingClientRect();return String(r.top>=0&&r.left>=0&&r.bottom<=d.defaultView.innerHeight&&r.right<=d.defaultView.innerWidth);`), "true",
        "Restore previous settings must be fully visible in the viewport");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-failure-action=verify]').textContent;"), "Restore previous settings",
        "the failure's next action is a control");
      assert.match(await inner("return d.querySelector('[data-studio-message=verify]').textContent;"), /The Restore did not confirm/,
        "the failure's sentence is shown beside its control");
      assert.strictEqual(await inner("return String(d.querySelector('[data-studio-page=verify]').hasAttribute('inert'));"), "false", "the page stays usable");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true", "Identify waits for the Restore");
      assert.strictEqual(await inner("return String(Array.from(d.querySelectorAll('[data-engineer-stage-tab]')).every(tab=>!tab.disabled));"), "true");
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();return 'navigation remains usable';");
      assert.strictEqual(await inner(reasonOf("tune")), BLOCK_REASON, "the block's reason follows page navigation");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-tune]').disabled);"), "true", "Tune waits for the Restore");
    }, "recovery-navigation");
  });

  test("the failure's Restore previous settings asks for the Restore review on Validate, and a refused review leaves Restore in the position", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=model]').click();return 'identify';");
      await inner(engineResult("apply", unresolvedApply()));
      await pollValue("the block's reason on Identify", () => inner(reasonOf("model")), value => value === BLOCK_REASON, 10000);
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      assert.strictEqual(await inner("const a=d.querySelector('[data-studio-failure-action=verify]'),r=a.getBoundingClientRect();return String(!a.hidden&&r.top>=0&&r.left>=0&&r.bottom<=d.defaultView.innerHeight&&r.right<=d.defaultView.innerWidth);"), "true",
        "the failure's Restore previous settings must be fully visible in the viewport");
      await inner("d.querySelector('[data-studio-failure-action=verify]').click();return 'restore';");
      assert.strictEqual(await inner("return d.querySelector('[data-studio-three-page]').dataset.selectedStage;"), "verify",
        "the Restore review is shown on the Validate page");
      // The request reaches the engine (engine/routes/applicationRoute.ts): this project's journal holds no checkpoint, so the engine
      // refuses the review with its own sentence, which the Apply area shows in place of the earlier failure.
      await pollValue("the engine's refusal", () => inner("return d.querySelector('[data-studio-message=verify]').textContent;"),
        value => value !== "" && value !== "The Restore did not confirm.", 15000);
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Restore previous settings");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "false", "a refusal takes no offer away");
    }, "retry-failure");
  });

  test("the confirmed Restore's stages end the block, and a normal Apply review adds no recovery control", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(60000);
    await withStudio(async inner => {
      await inner("d.querySelector('[data-engineer-stage-tab=verify]').click();return 'validate';");
      await inner(engineResult("apply", unresolvedApply()));
      await pollValue("the block", () => inner(reasonOf("verify")), value => value === BLOCK_REASON, 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "true", "the block holds the stage actions");
      await inner(engineResult("restore", { kind: "restoreOutcome", restored: true, reason: "Previous values restored and observed after ST execution.", parameters: {},
        restoreAvailable: false, keepAvailable: false, applyAvailable: false, stages: stages(undefined, false) }));
      await pollValue("the block ended", () => inner(reasonOf("verify")), value => value === "", 10000);
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-find]').disabled);"), "false", "Identify can start again");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-verify]').disabled);"), "false", "Validate can start again");
      assert.strictEqual(await inner("return d.querySelector('[data-engineer-apply]').textContent;"), "Apply");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-apply]').disabled);"), "true", "nothing is offered after a confirmed Restore");
      // A physical target's Apply review shows the engine's approval block in the Apply area (webview/threePageApplication.ts showReview).
      await inner(engineResult("apply", { kind: "applicationReview", action: "apply", identity: "sha256:review", description: "Apply writes the validated candidate.", foundMode: "auto",
        physical: true, confirmation: "approval", parameters: [{ symbol: "Kp", unit: "", running: 2, next: 3 }],
        approval: { loopId: "heater-pid", signals: [], durationSeconds: 12, stopConditions: [], restoration: "Restore previous settings returns the record." } }));
      assert.strictEqual(await inner("return String(Boolean(d.querySelector('[data-engine-approval]')));"), "true", "The review asks for the engineer's approval");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engine-confirm=apply]').disabled);"), "true", "Confirm waits for the approval");
      assert.strictEqual(await inner("return String(d.querySelector('[data-engineer-keep]').hidden);"), "true", "Keep in project waits for a successful Apply");
      assert.strictEqual(await inner("return String(!d.querySelector('[data-studio-recovery-action]'));"), "true", "the page projects no recovery of its own");
    }, "resolved-apply-review");
  });
});

/** The engine's reason of the block of an Apply or a Restore that stopped before the runtime confirmed its end (trust-control `checkpoint_block`). */
const BLOCK_REASON = "An Apply or a Restore of this loop stopped before the runtime confirmed its end. Restore previous settings first: until the runtime confirms it, the loop's other actions wait.";

/**
 * The statuses of the three pages' stages and of Apply, held by `reason` when one is given, as the engine answers them (B9.4). The Apply
 * stage can start exactly when the answer offers Apply (`apply`, B7).
 */
function stages(reason: string | undefined, apply = reason === undefined): Array<Record<string, unknown>> {
  return ["Identify", "Tune", "Validate", "Apply"].map(stage => ({ stage, status: "ready", label: "Ready",
    available: stage === "Apply" ? apply : reason === undefined, ...(reason === undefined ? {} : { reason }) }));
}

/**
 * The end of an Apply whose own Restore the runtime did not confirm (B6.3, B6.7): its failure names Restore as the next action, the engine
 * offers Restore and names it in the position, and its stages are held by the checkpoint that waits.
 */
function unresolvedApply(): Record<string, unknown> {
  return { kind: "applyOutcome", checkpointId: "commissioning-test", validationIdentity: "sha256:applied", restored: false, restoreAvailable: true, keepAvailable: false,
    applyAvailable: false, primaryAction: "restore", stages: stages(BLOCK_REASON),
    result: { status: "failed", reason: "The Restore did not confirm.",
      failure: { message: "The Restore did not confirm.", cause: "runtime", nextAction: "restore", nextActionLabel: "Restore previous settings" } } };
}

/** The webview script that answers the text of the stage reason at the top of `page`, or an empty text when it shows none. */
function reasonOf(page: string): string {
  return `return d.querySelector('[data-studio-page=${page}] > [data-engineer-stage-reason]')?.textContent ?? '';`;
}

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

/** Posts the engine's answer of a method as the host does (engine/enginePanelRoute.ts): its result, in the loop's scope. */
function engineResult(method: string, result: Record<string, unknown>): string {
  return `const root=d.querySelector('[data-studio-three-page]');d.defaultView.postMessage({schemaVersion:1,kind:'control-studio.engine.result',projectId:root.dataset.projectId,editorSessionId:root.dataset.editorSessionId,loopId:root.dataset.loopId,expectedLoopRevision:root.dataset.loopRevision,method:${JSON.stringify(method)},result:${JSON.stringify(result)}},'*');await new Promise(resolve=>setTimeout(resolve,100));return 'received';`;
}
