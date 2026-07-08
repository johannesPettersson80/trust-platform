import { getTrustConfiguration } from "../configuration";
import * as vscode from "vscode";

import { SettingsPayload } from "./types";

export function collectSettingsSnapshot(): SettingsPayload {
  const config = getTrustConfiguration();
  return {
    serverPath: config.get<string>("server.path") ?? "",
    traceServer: config.get<string>("trace.server") ?? "off",
    debugAdapterPath: config.get<string>("debug.adapter.path") ?? "",
    debugAdapterArgs: config.get<string[]>("debug.adapter.args") ?? [],
    debugAdapterEnv: config.get<Record<string, string>>("debug.adapter.env") ?? {},
    runtimeControlEndpoint: config.get<string>("runtime.controlEndpoint") ?? "",
    runtimeControlAuthToken: config.get<string>("runtime.controlAuthToken") ?? "",
    runtimeIncludeGlobs: config.get<string[]>("runtime.includeGlobs") ?? [],
    runtimeExcludeGlobs: config.get<string[]>("runtime.excludeGlobs") ?? [],
    runtimeIgnorePragmas: config.get<string[]>("runtime.ignorePragmas") ?? [],
    runtimeInlineValuesEnabled:
      config.get<boolean>("runtime.inlineValuesEnabled") ?? true,
  };
}

export async function applySettingsUpdate(
  payload: SettingsPayload | undefined
): Promise<void> {
  if (!payload) {
    return;
  }
  const config = getTrustConfiguration();
  const settingsUpdates: Array<{ key: string; value: unknown }> = [
    { key: "server.path", value: payload.serverPath?.trim() || undefined },
    { key: "trace.server", value: payload.traceServer?.trim() || "off" },
    {
      key: "debug.adapter.path",
      value: payload.debugAdapterPath?.trim() || undefined,
    },
    { key: "debug.adapter.args", value: payload.debugAdapterArgs ?? [] },
    { key: "debug.adapter.env", value: payload.debugAdapterEnv ?? {} },
    {
      key: "runtime.controlEndpoint",
      value: payload.runtimeControlEndpoint?.trim() || undefined,
    },
    {
      key: "runtime.controlAuthToken",
      value: payload.runtimeControlAuthToken?.trim() || undefined,
    },
    { key: "runtime.includeGlobs", value: payload.runtimeIncludeGlobs ?? [] },
    { key: "runtime.excludeGlobs", value: payload.runtimeExcludeGlobs ?? [] },
    { key: "runtime.ignorePragmas", value: payload.runtimeIgnorePragmas ?? [] },
    {
      key: "runtime.inlineValuesEnabled",
      value: payload.runtimeInlineValuesEnabled ?? true,
    },
  ];
  for (const update of settingsUpdates) {
    await config.update(
      update.key,
      update.value,
      vscode.ConfigurationTarget.Workspace
    );
  }
}
