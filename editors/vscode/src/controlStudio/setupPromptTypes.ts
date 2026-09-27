import type * as vscode from "vscode";

export type SetupPhase = 1 | 2 | 3 | 4;
export const SETUP_PHASE_NAMES: Record<SetupPhase, string> = {
  1: "Loop and signals", 2: "Units and bounds",
  3: "Controller and experiment", 4: "Channels and actuators",
};
export interface SetupPromptContext {
  phase: SetupPhase;
  canGoBack: boolean;
  previous?: unknown;
}
export type SetupPromptResult<T> = { kind: "accepted"; value: T }
  | { kind: "cancelled" } | { kind: "back"; value?: T };

/** Native prompt presentation is separate from answer history and loop construction. */
export interface SetupPromptPresenter {
  input(options: vscode.InputBoxOptions, context: SetupPromptContext): Promise<SetupPromptResult<string>>;
  pick<T extends string | vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions,
    context: SetupPromptContext): Promise<SetupPromptResult<T | T[]>>;
  open(options: vscode.OpenDialogOptions): Promise<vscode.Uri[] | undefined>;
}

/** The same prompt port is passed to existing controller/channel setup owners. */
export interface SetupDialogs {
  showInputBox(options?: vscode.InputBoxOptions): PromiseLike<string | undefined>;
  showQuickPick<T extends vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions & { canPickMany: true }): PromiseLike<T[] | undefined>;
  showQuickPick<T extends vscode.QuickPickItem>(items: readonly T[], options?: vscode.QuickPickOptions): PromiseLike<T | undefined>;
  showQuickPick(items: readonly string[], options: vscode.QuickPickOptions & { canPickMany: true }): PromiseLike<string[] | undefined>;
  showQuickPick(items: readonly string[], options?: vscode.QuickPickOptions): PromiseLike<string | undefined>;
  showOpenDialog(options: vscode.OpenDialogOptions): PromiseLike<vscode.Uri[] | undefined>;
}
