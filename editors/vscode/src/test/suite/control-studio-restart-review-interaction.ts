import * as assert from "assert";
import * as vscode from "vscode";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { attachControlStudio } from "./control-studio-rendered-harness";
import { evaluateInWebview, pollValue, type CdpConnection } from "./plant-rendered-harness";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

export function installRestartRecoveryFault() {
  const module = require("../../controlStudio/runtimeConnection");
  const original = module.bindRuntimeConnection;
  let blocked = false;
  let writes = 0;
  module.bindRuntimeConnection = async (...args: unknown[]) => {
    const bound = await original(...args);
    return { ...bound, request: async (operation: string, params: any) => {
      if (blocked && operation === "control.transfer.apply" && params.manual) throw new Error("Controlled interrupted Manual recovery");
      if (operation === "control.parameters.apply") writes++;
      return bound.request(operation, params);
    } };
  };
  return { block: () => { blocked = true; }, writes: () => writes,
    restore: () => { module.bindRuntimeConnection = original; } };
}

export async function reviewRestartedRuntime(connection: CdpConnection, port: number, inner: EvaluateWebview,
  loop: any, fault: ReturnType<typeof installRestartRecoveryFault>, capture: (name: string) => Promise<void>) {
  await pollValue("Auto before interrupted close", () => inner("return String(d.querySelector('[data-control-studio-controller-mode]').textContent==='Auto'&&Number(d.querySelector('[data-control-studio-pv]').textContent.split(' ')[0])>0.1);"), value => value === "true", 10000);
  const previous = await inner("return d.querySelector('[data-control-studio]').dataset.editorSessionId;");
  fault.block();
  await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
  await runtimeLifecycleService.stopRuntime();
  assert.strictEqual((await runtimeLifecycleService.startLocalSimulator()).ok, true);
  await vscode.commands.executeCommand("trust-lsp.controlStudio.open", { schemaVersion: 1, source: "ConfiguredLoop", scope: { projectId: loop.projectId, runtimeId: loop.runtimeId }, loopDefinition: loop });
  const session = await attachControlStudio(connection, port, previous, loop.id);
  const current = (body: string) => evaluateInWebview(connection, session, body);
  const parameters = require("../../controlStudio/runtimeParameterConnection");
  const status = async () => (await parameters.connectRuntimeParameters(loop)).client.status();
  const before = await status();
  const writes = fault.writes();
  await pollValue("restarted runtime review action", () => current("const b=d.querySelector('[data-engineer-review-runtime]');const r=b.getBoundingClientRect();return String(!b.disabled&&r.width>0&&r.height>0);"), value => value === "true", 15000);
  await capture("restart-review-needed");
  await current("d.querySelector('[data-engineer-review-runtime]').click();return 'review';");
  await pollValue("replacement runtime review", () => current("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("new runtime epoch"), 10000);
  await capture("restart-review");
  await current("d.querySelector('[data-engineer-confirm]').click();return 'confirm retirement';");
  await pollValue("retained unresolved history", () => current("return d.querySelector('[data-engineer-status]').textContent;"), value => value.includes("retained as unresolved history"), 10000);
  const after = await status();
  assert.deepStrictEqual(after.values, before.values);
  assert.strictEqual(after.active_revision, before.active_revision);
  assert.strictEqual(fault.writes(), writes, "reviewing a new runtime cannot replay an old gain write");
  await current("d.querySelector('[data-engineer-stage-tab=model]').click();return 'model';");
  assert.strictEqual(await current("return String(d.querySelector('[data-engineer-find]').disabled);"), "false", "completed runtime review must unlock the next study");
  await capture("restart-new-study");
}
