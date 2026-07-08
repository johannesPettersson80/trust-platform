import * as vscode from "vscode";

const CANONICAL_SECTION = "trust";
const LEGACY_SECTION = "trust-lsp";

const CANONICAL_KEYS: Record<string, string> = {
  "server.path": "languageServer.executablePath",
  "trace.server": "languageServer.trace",
  "diagnostics.showIecReferences": "diagnostics.showIecReferences",
  "debug.adapter.path": "debugAdapter.executablePath",
  "debug.adapter.args": "debugAdapter.arguments",
  "debug.adapter.env": "debugAdapter.environment",
  "runtime.includeGlobs": "runtime.sourceIncludePatterns",
  "runtime.excludeGlobs": "runtime.sourceExcludePatterns",
  "runtime.mode": "runtime.mode",
  "runtime.controlEndpoint": "runtime.controlEndpoint",
  "runtime.fleetEndpoints": "runtime.additionalEndpoints",
  "runtime.controlAuthToken": "runtime.authTokenFallback",
  "runtime.setupUrl": "runtime.setupUrl",
  "runtime.controlEndpointEnabled": "runtime.useControlEndpoint",
  "runtime.inlineValuesEnabled": "runtime.showInlineValues",
  "runtime.ignorePragmas": "runtime.ignorePragmas",
  "runtime.cli.path": "runtime.executablePath",
  "dev.cli.path": "testRunner.executablePath",
  "visual.autoGenerateStCompanion": "visual.generateStructuredTextCompanions",
  "visual.openStCompanionOnCreate":
    "visual.openStructuredTextCompanionOnCreate",
  "visual.autoOpenCustomEditors": "visual.openVisualEditorsAutomatically",
  "hmi.pollIntervalMs": "hmi.refreshIntervalMs",
};

interface ConfigurationInspect<T> {
  key: string;
  defaultValue?: T;
  globalValue?: T;
  workspaceValue?: T;
  workspaceFolderValue?: T;
  defaultLanguageValue?: T;
  globalLanguageValue?: T;
  workspaceLanguageValue?: T;
  workspaceFolderLanguageValue?: T;
  languageIds?: string[];
}

function canonicalKey(key: string): string {
  return CANONICAL_KEYS[key] ?? key;
}

function configuredValueExists(
  inspected: ConfigurationInspect<unknown> | undefined
): boolean {
  if (!inspected) {
    return false;
  }
  return (
    inspected.globalValue !== undefined ||
    inspected.workspaceValue !== undefined ||
    inspected.workspaceFolderValue !== undefined ||
    inspected.globalLanguageValue !== undefined ||
    inspected.workspaceLanguageValue !== undefined ||
    inspected.workspaceFolderLanguageValue !== undefined
  );
}

export function affectsTrustConfiguration(
  event: vscode.ConfigurationChangeEvent,
  key?: string
): boolean {
  if (!key) {
    return (
      event.affectsConfiguration(CANONICAL_SECTION) ||
      event.affectsConfiguration(LEGACY_SECTION)
    );
  }
  return (
    event.affectsConfiguration(`${CANONICAL_SECTION}.${canonicalKey(key)}`) ||
    event.affectsConfiguration(`${LEGACY_SECTION}.${key}`)
  );
}

export function getTrustConfiguration(
  scope?: vscode.ConfigurationScope | null
): vscode.WorkspaceConfiguration {
  const canonical = vscode.workspace.getConfiguration(CANONICAL_SECTION, scope);
  const legacy = vscode.workspace.getConfiguration(LEGACY_SECTION, scope);

  return new Proxy({}, {
    get(target, property, receiver) {
      if (property === "get") {
        return <T>(key: string, defaultValue?: T): T | undefined => {
          const mapped = canonicalKey(key);
          const canonicalInspect = canonical.inspect<T>(mapped);
          if (configuredValueExists(canonicalInspect)) {
            return canonical.get<T>(mapped, defaultValue as T);
          }
          const legacyInspect = legacy.inspect<T>(key);
          if (configuredValueExists(legacyInspect)) {
            return legacy.get<T>(key, defaultValue as T);
          }
          return canonical.get<T>(mapped, defaultValue as T);
        };
      }

      if (property === "update") {
        return (
          key: string,
          value: unknown,
          configurationTarget?: boolean | vscode.ConfigurationTarget | null,
          overrideInLanguage?: boolean
        ): Thenable<void> =>
          canonical.update(
            canonicalKey(key),
            value,
            configurationTarget,
            overrideInLanguage
          );
      }

      if (property === "inspect") {
        return <T>(
          key: string
        ): ConfigurationInspect<T> | undefined => {
          const mapped = canonicalKey(key);
          const canonicalInspect = canonical.inspect<T>(mapped);
          const legacyInspect = legacy.inspect<T>(key);
          return configuredValueExists(legacyInspect) &&
            !configuredValueExists(canonicalInspect)
            ? legacyInspect
            : canonicalInspect;
        };
      }

      if (property === "has") {
        return (key: string): boolean =>
          canonical.has(canonicalKey(key)) || legacy.has(key);
      }

      const value = Reflect.get(canonical, property, receiver);
      return typeof value === "function" ? value.bind(canonical) : value;
    },
  }) as vscode.WorkspaceConfiguration;
}
