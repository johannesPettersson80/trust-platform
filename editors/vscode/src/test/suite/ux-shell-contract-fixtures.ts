import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  exampleQuickPickItems,
  hardwareBadge,
  parseManifest,
} from "../../examples/model";
import {
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
} from "../../networkCanvas/webview/setUpRuntime";
import { pickAuthToken } from "../../runtimeAuthModel";
import { CHECK_PROGRAM_COMMAND } from "../../checkProgram";
import { summarizeCheck } from "../../checkProgramModel";

// v5 "complete PLC IDE shell" contract guards (vscode-ux-overhaul-plan.md §0.5/§0.6/§9). This file holds
// the package.json + source invariants for the shell: palette cleanup, no user-facing Communication
// panel, the two sidebar states, no "Network Canvas" jargon, examples manifest, etc. The Run-card MODEL
// assertions live in runtime-controls-contract.test.ts.

type MenuItem = { command?: string; when?: string; group?: string };
type Pkg = {
  activationEvents?: string[];
  contributes?: {
    commands?: Array<{ command?: string; title?: string; category?: string }>;
    configuration?: unknown;
    languageModelTools?: Array<{ name?: string; displayName?: string }>;
    menus?: {
      commandPalette?: MenuItem[];
      "editor/title"?: MenuItem[];
      "view/title"?: MenuItem[];
      "view/item/context"?: MenuItem[];
    };
    viewsContainers?: { activitybar?: Array<{ id?: string }> };
    views?: Record<string, Array<{ id?: string; type?: string }>>;
    viewsWelcome?: Array<{ view?: string; contents?: string }>;
    debuggers?: Array<{
      type?: string;
      label?: string;
      initialConfigurations?: Array<{ name?: string; request?: string }>;
    }>;
  };
  scripts?: Record<string, string>;
};

type ConfigurationContribution = {
  title?: string;
  properties?: Record<
    string,
    {
      title?: string;
      description?: string;
      markdownDescription?: string;
    }
  >;
};

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function workspaceRoot(): string {
  return path.resolve(extensionRoot(), "..", "..");
}

function loadPackageJson(): Pkg {
  return JSON.parse(
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8")
  ) as Pkg;
}

function readSrc(file: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", file), "utf8");
}

function readSrcSet(...files: string[]): string {
  return files.map((file) => readSrc(file)).join("\n");
}

function readIoPanelDocumentSource(): string {
  return readSrcSet(
    "io-panel/html.ts",
    "io-panel/styles/foundation.ts",
    "io-panel/styles/tree.ts",
    "io-panel/styles/valueRows.ts",
    "io-panel/styles/feedbackAndSettings.ts"
  );
}

function paletteHidden(pkg: Pkg, command: string): boolean {
  const entries = pkg.contributes?.menus?.commandPalette ?? [];
  return entries.some((item) => item.command === command && item.when === "false");
}

function commandTitles(pkg: Pkg): Map<string, string> {
  const map = new Map<string, string>();
  for (const command of pkg.contributes?.commands ?? []) {
    if (command.command) {
      map.set(command.command, command.title ?? "");
    }
  }
  return map;
}

// The leaked palette commands the v5 cleanup hides (§0.5.6). Each stays REGISTERED (escape hatch) but is
// hidden from Ctrl+Shift+P so core flows route through visible surfaces, not the palette.
const HIDDEN_FROM_PALETTE = [
  "trust-lsp.debug.openIoPanel", // "Open Live Values" — reached via the Live Values launcher
  "trust-lsp.debug.start", // "Start Debugging" — F5 uses the debugger, not the palette
  "trust-lsp.debug.attach", // "Attach Debugger"
  "trust-lsp.debug.ensureConfiguration", // target selection lives in the sidebar, not the palette
  "trust-lsp.debug.reload", // Update running simulation drives this internal command
  "trust-lsp.test.runAll", // tests live in VS Code's native Testing view
  "trust-lsp.test.runOne", // tests live in VS Code's native Testing view
  "trust-lsp.hmi.init", // raw HMI init — reached via the adaptive HMI launcher
  "trust-lsp.hmi.refreshFromDescriptor", // raw HMI refresh
];

const RETIRED_COMMUNICATION_COMMANDS = [
  "trust-lsp.communication.openPanel",
  "trust-lsp.ads.openPanel",
  "trust-lsp.ads.server.openPanel",
  "trust-lsp.ads.addDevice",
  "trust-lsp.ads.diagnose",
  "trust-lsp.ads.importSymbols",
  "trust-lsp.ads.addRoute",
];

export {
  assert,
  fs,
  path,
  exampleQuickPickItems,
  hardwareBadge,
  parseManifest,
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
  pickAuthToken,
  CHECK_PROGRAM_COMMAND,
  summarizeCheck,
  extensionRoot,
  workspaceRoot,
  loadPackageJson,
  readSrc,
  readSrcSet,
  readIoPanelDocumentSource,
  paletteHidden,
  commandTitles,
  HIDDEN_FROM_PALETTE,
  RETIRED_COMMUNICATION_COMMANDS,
};
export type {
  MenuItem,
  Pkg,
  ConfigurationContribution,
};
