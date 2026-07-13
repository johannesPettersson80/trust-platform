import * as vscode from "vscode";

import type { LifecycleAction, LifecyclePhase } from "../lifecycleEntryFailure";
import {
  showManagedRuntimeLogs,
  startManagedRuntime,
  stopManagedRuntime,
} from "../localRuntime";
import {
  attachManagedRuntimeAfterStart,
  disconnectManagedRuntimeAfterStop,
} from "../managedRuntimeSession";
import {
  openSelectedRuntimeToml,
  openStructuredTextDebuggerLogs,
} from "../runtimeRecoveryActions";
import type {
  RuntimeExclusiveOperationResult,
  RuntimeLifecycleResult,
} from "../runtimeLifecycle";
import type {
  RuntimeLifecycleOperationKind,
  RuntimeLifecycleTarget,
} from "../runtimeLifecycleModel";
import { openRuntimePane } from "../runtimeTarget";
import { setSelectedRuntimeId } from "../selectedRuntime";
import { SIMULATOR_RUNTIME_ID } from "../trustHomeModel";
import {
  runtimeOperationBlockReason,
  type RuntimeLockedAction,
} from "../runtimeOperationPolicy";

export interface NetworkCanvasLifecycleActionDependencies {
  readonly extensionContext: () => vscode.ExtensionContext | undefined;
  readonly refresh: () => Promise<void>;
  readonly clearFailure: () => void;
  readonly recordResult: (
    result: RuntimeLifecycleResult,
    action: LifecycleAction,
  ) => void;
  readonly stopRuntime: () => Promise<RuntimeLifecycleResult>;
  readonly connectRemote: (
    endpoint: string,
    label?: string,
  ) => Promise<RuntimeLifecycleResult>;
  readonly runExclusiveOperation: <T>(
    kind: RuntimeLifecycleOperationKind,
    target: RuntimeLifecycleTarget,
    operation: (operationId: string) => Thenable<T>,
  ) => Promise<RuntimeExclusiveOperationResult<T>>;
  readonly lifecyclePhase: () => LifecyclePhase;
  readonly activeTarget: () => RuntimeLifecycleTarget | undefined;
  readonly managedTarget: (
    name: string,
    endpoint: string,
  ) =>
    Extract<RuntimeLifecycleTarget, { readonly kind: "managed" }> | undefined;
  readonly operationInProgress: () => boolean;
  readonly reportBlocked?: (reason: string) => void;
  readonly startManagedRuntime?: typeof startManagedRuntime;
  readonly stopManagedRuntime?: typeof stopManagedRuntime;
  readonly attachManagedRuntimeAfterStart?: typeof attachManagedRuntimeAfterStart;
  readonly disconnectManagedRuntimeAfterStop?: typeof disconnectManagedRuntimeAfterStop;
}

export class NetworkCanvasLifecycleActions {
  constructor(
    private readonly dependencies: NetworkCanvasLifecycleActionDependencies,
  ) {}

  async handleMessage(message: Record<string, unknown>): Promise<boolean> {
    const operation = lockedOperationForCanvasMessage(message);
    if (operation && !this.allowOperation(operation, message)) {
      return true;
    }
    switch (message.type) {
      case "action":
        await this.handleCanvasAction(
          typeof message.action === "string" ? message.action : "",
        );
        return true;
      case "openRuntimePane":
        await openRuntimePane();
        return true;
      case "openRuntimeSettings":
        await vscode.commands.executeCommand(
          "trust-lsp.debug.openIoPanelSettings",
        );
        return true;
      case "openRuntimeLogs":
        openStructuredTextDebuggerLogs();
        return true;
      case "openRuntimeToml":
        await openSelectedRuntimeToml();
        return true;
      case "runtimeConnect":
        await this.connectRemote(message);
        return true;
      case "runtimeDisconnect":
        this.dependencies.recordResult(
          await this.dependencies.stopRuntime(),
          "disconnect",
        );
        await this.dependencies.refresh();
        return true;
      case "setRuntimeAuthToken":
        await vscode.commands.executeCommand("trust-lsp.runtime.setAuthToken", {
          endpoint:
            typeof message.endpoint === "string" ? message.endpoint : "",
        });
        await this.dependencies.refresh();
        return true;
      case "setAsRunTarget":
        await this.setAsRunTarget(message);
        return true;
      case "runtimeManagedStart":
      case "runtimeManagedStop":
        await this.runManagedAction(message);
        return true;
      case "runtimeManagedLogs":
        await this.showManagedLogs(message);
        return true;
      default:
        return false;
    }
  }

  private allowOperation(
    action: RuntimeLockedAction,
    message: Record<string, unknown>,
  ): boolean {
    const reason =
      runtimeOperationBlockReason(
        this.dependencies.lifecyclePhase(),
        action,
        this.dependencies.operationInProgress(),
      ) ?? this.ownershipBlockReason(action, message);
    if (!reason) {
      return true;
    }
    if (this.dependencies.reportBlocked) {
      this.dependencies.reportBlocked(reason);
    } else {
      void vscode.window.showInformationMessage(reason);
    }
    return false;
  }

  private ownershipBlockReason(
    action: RuntimeLockedAction,
    message: Record<string, unknown>,
  ): string | undefined {
    const phase = this.dependencies.lifecyclePhase();
    const active = this.dependencies.activeTarget();
    if (action === "managed_start" || action === "managed_stop") {
      const name = typeof message.name === "string" ? message.name.trim() : "";
      const endpoint =
        typeof message.endpoint === "string" ? message.endpoint.trim() : "";
      const managedTarget = this.dependencies.managedTarget(name, endpoint);
      if (!managedTarget) {
        return "The managed runtime list changed. Wait for Devices & Connections to refresh, then try again.";
      }
      if (phase === "stopped" || action === "managed_start") {
        return undefined;
      }
      return managedRuntimeOwnsActiveTarget(name, endpoint, active)
        ? undefined
        : runtimeOperationBlockReason(phase, "managed_start");
    }
    if (phase === "stopped" || phase === "starting") {
      return undefined;
    }
    if (action === "remote_disconnect") {
      const endpoint =
        typeof message.endpoint === "string" ? message.endpoint.trim() : "";
      return active?.kind === "remote" && endpoint === active.endpoint
        ? undefined
        : runtimeOperationBlockReason(phase, "remote_connect");
    }
    return undefined;
  }

  private async handleCanvasAction(action: string): Promise<void> {
    switch (action) {
      case "openRuntimePane":
        await openRuntimePane();
        break;
      case "openRuntimeLogs":
        openStructuredTextDebuggerLogs();
        break;
      case "openRuntimeToml":
        await openSelectedRuntimeToml();
        break;
      case "openRuntimeSettings":
        await vscode.commands.executeCommand(
          "trust-lsp.debug.openIoPanelSettings",
        );
        break;
    }
  }

  private async connectRemote(message: Record<string, unknown>): Promise<void> {
    const endpoint =
      typeof message.endpoint === "string" ? message.endpoint : "";
    const label = typeof message.label === "string" ? message.label : undefined;
    const result = await this.dependencies.connectRemote(endpoint, label);
    this.dependencies.recordResult(result, "connect");
    if (result.ok && endpoint) {
      await setSelectedRuntimeId(endpoint);
    }
    await this.dependencies.refresh();
  }

  private async setAsRunTarget(
    message: Record<string, unknown>,
  ): Promise<void> {
    const managedName =
      typeof message.managedName === "string" ? message.managedName : "";
    const endpoint =
      typeof message.endpoint === "string" ? message.endpoint : "";
    const target = managedName
      ? managedName
      : message.isLocal || !endpoint
        ? SIMULATOR_RUNTIME_ID
        : endpoint;
    await setSelectedRuntimeId(target);
    await this.dependencies.refresh();
  }

  private async runManagedAction(
    message: Record<string, unknown>,
  ): Promise<void> {
    const context = this.dependencies.extensionContext();
    const name = typeof message.name === "string" ? message.name : "";
    if (context && name) {
      const starting = message.type === "runtimeManagedStart";
      const endpoint =
        typeof message.endpoint === "string" ? message.endpoint.trim() : "";
      const inventoryTarget = this.dependencies.managedTarget(name, endpoint);
      if (!inventoryTarget) {
        this.dependencies.reportBlocked?.(
          "The managed runtime list changed. Wait for Devices & Connections to refresh, then try again.",
        );
        await this.dependencies.refresh();
        return;
      }
      const validatedAuthority = this.dependencies.activeTarget();
      const operationTarget =
        !starting &&
        validatedAuthority?.kind === "managed" &&
        validatedAuthority.id === name
          ? validatedAuthority
          : inventoryTarget;
      const operation = await this.dependencies.runExclusiveOperation(
        starting ? "managed_start" : "managed_stop",
        operationTarget,
        async (operationId) => {
          const result = starting
            ? await (
                this.dependencies.startManagedRuntime ?? startManagedRuntime
              )(context, name)
            : await (
                this.dependencies.stopManagedRuntime ?? stopManagedRuntime
              )(context, name);
          if (!result.ok) {
            return { result };
          }
          if (starting) {
            return {
              result,
              attach: await (
                this.dependencies.attachManagedRuntimeAfterStart ??
                attachManagedRuntimeAfterStart
              )(name, result, operationId),
            };
          }
          return {
            result,
            disconnect: await (
              this.dependencies.disconnectManagedRuntimeAfterStop ??
              disconnectManagedRuntimeAfterStop
            )(name, result, operationId, validatedAuthority),
          };
        },
      );
      if (!operation.acquired) {
        this.dependencies.reportBlocked?.(operation.reason);
        await this.dependencies.refresh();
        return;
      }
      const { result, attach, disconnect } = operation.value;
      if (!result.ok) {
        void vscode.window.showWarningMessage(
          result.message || `Could not ${starting ? "start" : "stop"} ${name}.`,
        );
      } else if (starting) {
        this.dependencies.clearFailure();
        if (!attach?.ok) {
          void vscode.window.showWarningMessage(
            attach?.message ||
              "Runtime started, but Live Values could not connect.",
          );
        }
      } else {
        const disconnectResult = disconnect ?? {
          ok: false as const,
          failure: {
            kind: "stale_runtime" as const,
            message: `Stopped ${name}, but its Live Values session outcome was not reported.`,
          },
        };
        if (!disconnectResult.ok) {
          this.dependencies.recordResult(disconnectResult, "disconnect");
          void vscode.window.showWarningMessage(
            disconnectResult.failure.message ||
              `Stopped ${name}, but could not close its Live Values session.`,
          );
        } else {
          this.dependencies.clearFailure();
        }
      }
    }
    await this.dependencies.refresh();
  }

  private async showManagedLogs(
    message: Record<string, unknown>,
  ): Promise<void> {
    const context = this.dependencies.extensionContext();
    const name = typeof message.name === "string" ? message.name : "";
    if (context && name) {
      await showManagedRuntimeLogs(context, name);
    }
  }
}

export function managedRuntimeOwnsActiveTarget(
  name: string,
  endpoint: string,
  active: RuntimeLifecycleTarget | undefined,
): boolean {
  if (active?.kind === "managed") {
    return (
      active.id === name &&
      (!active.endpoint || !endpoint || active.endpoint === endpoint)
    );
  }
  return false;
}

export function lockedOperationForCanvasMessage(
  message: Record<string, unknown>,
): RuntimeLockedAction | undefined {
  switch (message.type) {
    case "runtimeConnect":
      return "remote_connect";
    case "runtimeDisconnect":
      return "remote_disconnect";
    case "setAsRunTarget":
      return "set_run_target";
    case "runtimeManagedStart":
      return "managed_start";
    case "runtimeManagedStop":
      return "managed_stop";
    default:
      return undefined;
  }
}
