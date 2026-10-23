import * as vscode from "vscode";

import {
  applyCommSetup,
  testCommSetup,
} from "../communication/runtimeComm";
import type {
  CommApplyResponse,
  CommSchemaResponse,
} from "../communication/schemaForm";
import { resolveRuntimeTarget } from "../runtimeTarget";
import type { FleetTopologyResponse } from "./fleetTopology";
import {
  offlineCommApply,
  offlineCommTopology,
} from "./offlineComm";

export interface ConfigurationActionDependencies {
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly schema: () => CommSchemaResponse | undefined;
  readonly commit: (protocol: string, result: CommApplyResponse) => void;
  readonly refresh: () => Promise<void>;
}

export class NetworkCanvasConfigurationActions {
  constructor(private readonly dependencies: ConfigurationActionDependencies) {}

  async apply(message: Record<string, unknown>): Promise<void> {
    const runtime = await resolveRuntimeTarget();
    const result = await applyCommSetup(
      runtime,
      message,
      this.dependencies.schema()
    );
    if (result) {
      this.dependencies.commit(result.protocol, result.applyResult);
      await this.dependencies.refresh();
    }
  }

  async save(
    message: Record<string, unknown>,
    action: "upsert" | "remove" | "disable"
  ): Promise<void> {
    const projectDir = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const protocol =
      typeof message.protocol === "string" ? message.protocol : undefined;
    const context = this.dependencies.extensionContext();
    if (!context || !projectDir || !protocol) {
      return;
    }
    const params = isRecord(message.params) ? message.params : {};
    const result = await offlineCommApply(
      context,
      projectDir,
      protocol,
      params,
      action
    );
    if (result) {
      const topology = result.applied
        ? await offlineCommTopology(context, projectDir)
        : undefined;
      this.dependencies.commit(protocol, {
        ...result,
        instance_id:
          result.instance_id ??
          (topology && action !== "remove"
            ? findSavedEndpointId(topology, protocol, params)
            : undefined),
      });
    }
    await this.dependencies.refresh();
  }

  async test(message: Record<string, unknown>): Promise<void> {
    const runtime = await resolveRuntimeTarget();
    const result = await testCommSetup(
      runtime,
      message,
      this.dependencies.schema()
    );
    if (result) {
      this.dependencies.commit(result.protocol, result.applyResult);
      await this.dependencies.refresh();
    }
  }
}

function findSavedEndpointId(
  topology: FleetTopologyResponse,
  protocol: string,
  submittedParams: Record<string, unknown>
): string | undefined {
  const matches: Array<{
    id: string;
    params?: Record<string, unknown>;
  }> = [];
  for (const host of topology.hosts ?? []) {
    const runtimes = [
      ...(host.runtimes ?? []),
      ...(host.containers ?? []).flatMap(
        (container) => container.runtimes ?? []
      ),
    ];
    for (const runtime of runtimes) {
      for (const endpoint of runtime.endpoints ?? []) {
        if (endpoint.protocol === protocol) {
          matches.push({ id: endpoint.id, params: endpoint.params });
        }
      }
    }
  }
  const exact = matches.filter((endpoint) =>
    endpoint.params ? paramsMatch(endpoint.params, submittedParams) : false
  );
  return (lastItem(exact) ?? lastItem(matches))?.id;
}

function paramsMatch(
  endpointParams: Record<string, unknown>,
  submittedParams: Record<string, unknown>
): boolean {
  return Object.entries(submittedParams).every(([key, value]) => {
    if (!(key in endpointParams)) {
      return true;
    }
    return stableParamValue(endpointParams[key]) === stableParamValue(value);
  });
}

function stableParamValue(value: unknown): string {
  return JSON.stringify(normalizeParamValue(value)) ?? "undefined";
}

function normalizeParamValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(normalizeParamValue);
  }
  if (isRecord(value)) {
    return Object.keys(value)
      .sort()
      .reduce<Record<string, unknown>>((normalized, key) => {
        normalized[key] = normalizeParamValue(value[key]);
        return normalized;
      }, {});
  }
  return value;
}

function lastItem<T>(items: readonly T[]): T | undefined {
  return items.length > 0 ? items[items.length - 1] : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
