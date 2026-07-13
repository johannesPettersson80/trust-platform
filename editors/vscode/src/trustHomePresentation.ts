import {
  isConfigDiagnosticPath,
  type ValidityLine,
} from "./compileGate";
import { summarizeCheck, type CheckProgramResponse } from "./checkProgramModel";
import type { SelectedRuntime } from "./trustHomeModel";

export type CompileState =
  | { readonly kind: "unknown" }
  | { readonly kind: "dirty" }
  | { readonly kind: "clean"; readonly summary: string }
  | {
      readonly kind: "failed";
      readonly summary: string;
      readonly errors: number;
      readonly sourceErrors: number;
      readonly configErrors: number;
    };

type ButtonTone = "neutral" | "primary" | "success" | "warning" | "danger" | "disabled";
type ButtonVariant = "outline" | "filled";

export interface SidebarButtonState {
  readonly state: string;
  readonly label: string;
  readonly title: string;
  readonly icon: string;
  readonly tone: ButtonTone;
  readonly variant: ButtonVariant;
  readonly enabled: boolean;
}

export function displayProjectName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) {
    return "";
  }
  if (/^network[_-]+canvas[_-]+demo$/i.test(trimmed)) {
    return "Conveyor Demo";
  }
  if (!/[_-]/.test(trimmed)) {
    return trimmed;
  }
  return trimmed
    .split(/[_-]+/)
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function compileButtonState(
  state: CompileState,
  diagnostics: ValidityLine
): SidebarButtonState {
  if (!diagnostics.ok) {
    return {
      state: "diagnostics-failed",
      label: `Compile ${diagnostics.errors}`,
      title: diagnostics.label,
      icon: "codicon-error",
      tone: "danger",
      variant: "outline",
      enabled: true,
    };
  }
  switch (state.kind) {
    case "clean":
      return {
        state: "clean",
        label: "Compile",
        title: state.summary,
        icon: "codicon-check",
        tone: "neutral",
        variant: "outline",
        enabled: true,
      };
    case "failed":
      return {
        state: "failed",
        label: `Compile ${state.errors}`,
        title: state.summary,
        icon: "codicon-error",
        tone: "danger",
        variant: "outline",
        enabled: true,
      };
    case "dirty":
      return {
        state: "dirty",
        label: "Compile",
        title: "Source changed — compile again.",
        icon: "codicon-warning",
        tone: "warning",
        variant: "outline",
        enabled: true,
      };
    case "unknown":
    default:
      return {
        state: "unknown",
        label: "Compile",
        title: "Compile the project and show Problems if it fails.",
        icon: "codicon-tools",
        tone: "neutral",
        variant: "outline",
        enabled: true,
      };
  }
}

export function runtimeActionButtonState(selected: SelectedRuntime): SidebarButtonState {
  const action = selected.primary.action;
  const enabled = selected.primary.enabled;
  const title = selected.primary.hint || selected.statusLabel || selected.primary.label;
  switch (action) {
    case "start":
      return {
        state: "start",
        label: selected.primary.label,
        title,
        icon: "codicon-play",
        tone: enabled ? "primary" : "disabled",
        variant: enabled ? "filled" : "outline",
        enabled,
      };
    case "connect":
      return {
        state: enabled ? "connect" : "connect-disabled",
        label: selected.primary.label,
        title,
        icon: "codicon-remote",
        tone: enabled ? "primary" : "disabled",
        variant: enabled ? "filled" : "outline",
        enabled,
      };
    case "stop":
      return {
        state: "stop",
        label: "Stop",
        title,
        icon: "codicon-debug-stop",
        tone: "neutral",
        variant: "outline",
        enabled,
      };
    case "disconnect":
      return {
        state: "disconnect",
        label: selected.primary.label,
        title,
        icon: "codicon-debug-disconnect",
        tone: "neutral",
        variant: "outline",
        enabled,
      };
    case "none":
    default:
      return {
        state: selected.status === "starting" ? "busy" : "disabled",
        label: selected.primary.label,
        title,
        icon: selected.status === "starting" ? "codicon-loading codicon-modifier-spin" : "codicon-circle-slash",
        tone: "disabled",
        variant: "outline",
        enabled: false,
      };
  }
}

export function disabledButtonState(
  button: SidebarButtonState,
  reason: string | undefined
): SidebarButtonState {
  return reason
    ? {
        ...button,
        title: reason,
        tone: "disabled",
        variant: "outline",
        enabled: false,
      }
    : button;
}

export function classifyCompileIssues(response: CheckProgramResponse): {
  sourceErrors: number;
  configErrors: number;
} {
  let sourceErrors = 0;
  let configErrors = 0;
  for (const issue of response.issues ?? []) {
    if ((issue.severity ?? "").toLowerCase() !== "error") {
      continue;
    }
    const file = issue.file ?? "";
    const code = issue.code ?? "";
    if (isConfigDiagnosticPath(file) || /config/i.test(code)) {
      configErrors += 1;
    } else {
      sourceErrors += 1;
    }
  }
  return { sourceErrors, configErrors };
}

export function compileSummary(response: CheckProgramResponse): string {
  return summarizeCheck(response);
}
