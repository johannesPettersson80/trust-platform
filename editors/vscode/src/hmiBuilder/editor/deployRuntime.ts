// Deploy and Roll back from the editor (specification 39, §16.2 item 8): the language server carries out the
// transaction (`trust-lsp.hmiDeploy`); the extension passes the project, the selected run target and its control
// token, and shows the outcome.

import * as vscode from "vscode";

import { projectRequest } from "../../projectRequest";
import { runtimeLifecycleService } from "../../runtimeLifecycle";
import type { DeployDeps, DeployOutcome } from "./deploy";
import { remoteVariableContext } from "./remoteVariables";
import { resolveHmiRunTarget } from "./runTarget";

export const runtimeDeployDeps: DeployDeps = {
  async target() {
    const target = await resolveHmiRunTarget(await runtimeLifecycleService.snapshot());
    return target && { ...target, token: await runtimeLifecycleService.controlAuthToken(target.endpoint) };
  },
};

/** Deploys the bundle of the project at `root` to the selected run target, or rolls the target back (§16.2). */
export async function deployAction(
  action: "deploy" | "rollback",
  root: string,
  deps: DeployDeps,
): Promise<DeployOutcome> {
  const target = await deps.target();
  const project =
    action === "deploy"
      ? { remote: (await remoteVariableContext(root)).catalogue, root_uri: vscode.Uri.file(root).toString() }
      : undefined;
  const answer = await projectRequest<{ outcome: DeployOutcome }>("trust-lsp.hmiDeploy", {
    action,
    project,
    target: target && { endpoint: target.endpoint, label: target.label, simulator: target.simulator },
    authToken: target?.token,
  });
  return answer.ok ? answer.outcome : { ok: false, reason: answer.error };
}
