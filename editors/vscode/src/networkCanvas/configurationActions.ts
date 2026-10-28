import * as vscode from "vscode";

import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import { resolveRuntimeTarget } from "../runtimeTarget";
import {
  presentCanvasReply,
  requestCanvasAct,
  type CanvasLiveRuntime,
  type CanvasReply,
} from "./canvasCommands";

export interface ConfigurationActionDependencies {
  readonly schema: () => CommSchemaResponse | undefined;
  readonly commit: (protocol: string, result: CommApplyResponse) => void;
  readonly refresh: () => Promise<void>;
}

/**
 * The setup forms' actions. Each one forwards the form to `trust-lsp.devicesCanvasAct`, which
 * converts its values by the schema, checks them and applies, tests or saves them
 * (specification 25 §5.4, §5.9 and §5.10); the form shows the server's result.
 */
export class NetworkCanvasConfigurationActions {
  constructor(private readonly dependencies: ConfigurationActionDependencies) {}

  /** `Apply to running runtime`: the form applied at the runtime target. */
  async apply(message: Record<string, unknown>): Promise<void> {
    await this.show(
      await requestCanvasAct("comm_apply", this.form(message), {
        runtime: await liveRuntime(),
      })
    );
  }

  /** `Save`, `Remove` and `Disable`: the form written into the open project's files. */
  async save(
    message: Record<string, unknown>,
    action: "upsert" | "remove" | "disable"
  ): Promise<void> {
    const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!project) {
      return;
    }
    await this.show(
      await requestCanvasAct("comm_save", { ...this.form(message), action }, { project })
    );
  }

  /** `Test`: the form tested at the runtime target. */
  async test(message: Record<string, unknown>): Promise<void> {
    await this.show(
      await requestCanvasAct("comm_test", this.form(message), {
        runtime: await liveRuntime(),
      })
    );
  }

  /** The form as the server reads it: the values typed into it, or saved parameters. */
  private form(message: Record<string, unknown>): Record<string, unknown> {
    return {
      protocol: typeof message.protocol === "string" ? message.protocol : "",
      action: typeof message.action === "string" ? message.action : "",
      instance_id: typeof message.instanceId === "string" ? message.instanceId : undefined,
      runtime_id: typeof message.runtimeId === "string" ? message.runtimeId : undefined,
      values: isRecord(message.values) ? message.values : undefined,
      params: isRecord(message.params) ? message.params : undefined,
      schema: this.dependencies.schema(),
    };
  }

  private async show(reply: CanvasReply): Promise<void> {
    const result = isRecord(reply.result) ? (reply.result as unknown as CommApplyResponse) : undefined;
    if (result) {
      this.dependencies.commit(result.protocol, result);
    }
    await presentCanvasReply(reply);
    if (reply.refresh) {
      await this.dependencies.refresh();
    }
  }
}

/** The runtime target while it answers: the runtime a form is applied and tested at. */
async function liveRuntime(): Promise<CanvasLiveRuntime | undefined> {
  const runtime = await resolveRuntimeTarget();
  return runtime.status === "online_reachable" && runtime.endpoint
    ? { endpoint: runtime.endpoint, token: runtime.authToken }
    : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
