import * as vscode from "vscode";
import { SETUP_PHASE_NAMES, type SetupPromptContext, type SetupPromptPresenter, type SetupPromptResult } from "./setupPromptTypes";

const titleFor = (options: { title?: string }, context: SetupPromptContext): string =>
  `${SETUP_PHASE_NAMES[context.phase]} · ${options.title ?? "Setup"}`;

function disposeAll(control: vscode.QuickInput, subscriptions: vscode.Disposable[]): void {
  for (const subscription of subscriptions) subscription.dispose();
  control.dispose();
}

function itemKey(item: string | vscode.QuickPickItem): string {
  return typeof item === "string" ? item : `${item.label}\u0000${item.description ?? ""}`;
}

function sameItem(left: unknown, right: string | vscode.QuickPickItem): boolean {
  if (left === right) return true;
  if (typeof left === "string") return left === itemKey(right) || left === (typeof right === "string" ? right : right.label);
  if (!left || typeof left !== "object") return false;
  return itemKey(left as vscode.QuickPickItem) === itemKey(right);
}

function validationBlocks(validation: string | vscode.InputBoxValidationMessage | undefined): boolean {
  if (typeof validation === "string") return validation.length > 0;
  return validation?.severity === vscode.InputBoxValidationSeverity.Error;
}

function input(options: vscode.InputBoxOptions, context: SetupPromptContext): Promise<SetupPromptResult<string>> {
  const control = vscode.window.createInputBox();
  const subscriptions: vscode.Disposable[] = [];
  let settled = false;
  let validationAttempt = 0;
  const settle = (result: SetupPromptResult<string>): void => {
    if (settled) return;
    settled = true;
    disposeAll(control, subscriptions);
    resolver(result);
  };
  let resolver!: (result: SetupPromptResult<string>) => void;
  const result = new Promise<SetupPromptResult<string>>(resolve => { resolver = resolve; });

  control.title = titleFor(options, context);
  control.step = context.phase;
  control.totalSteps = 4;
  control.prompt = options.prompt;
  control.placeholder = options.placeHolder;
  control.password = options.password ?? false;
  control.ignoreFocusOut = options.ignoreFocusOut ?? false;
  control.value = typeof context.previous === "string" ? context.previous : options.value ?? "";
  control.valueSelection = options.valueSelection;
  control.buttons = context.canGoBack ? [vscode.QuickInputButtons.Back] : [];
  const validate = async (value: string, accepting = false): Promise<void> => {
    const attempt = ++validationAttempt;
    if (accepting) control.busy = true;
    try {
      const validation = await options.validateInput?.(value) ?? undefined;
      if (settled || attempt !== validationAttempt) return;
      control.validationMessage = validation;
      if (validationBlocks(validation)) {
        return;
      }
      if (accepting) settle({ kind: "accepted", value });
    } catch {
      if (!settled && attempt === validationAttempt) control.validationMessage = "Enter a valid value.";
    } finally {
      if (!settled && attempt === validationAttempt) control.busy = false;
    }
  };
  subscriptions.push(control.onDidChangeValue(value => {
    control.busy = false;
    control.validationMessage = undefined;
    void validate(value);
  }));
  subscriptions.push(control.onDidAccept(() => { void validate(control.value, true); }));
  subscriptions.push(control.onDidTriggerButton(button => {
    if (button === vscode.QuickInputButtons.Back) {
      ++validationAttempt;
      settle({ kind: "back", value: control.value });
    }
  }));
  subscriptions.push(control.onDidHide(() => settle({ kind: "cancelled" })));
  control.show();
  return result;
}

function pick<T extends string | vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions,
  context: SetupPromptContext): Promise<SetupPromptResult<T | T[]>> {
  type NativeItem = vscode.QuickPickItem & { value: T };
  const nativeItems: NativeItem[] = items.map(item => ({
    ...(typeof item === "string" ? { label: item } : item as vscode.QuickPickItem), value: item,
  }));
  const control = vscode.window.createQuickPick<NativeItem>();
  const subscriptions: vscode.Disposable[] = [];
  let settled = false;
  let resolver!: (result: SetupPromptResult<T | T[]>) => void;
  const result = new Promise<SetupPromptResult<T | T[]>>(resolve => { resolver = resolve; });
  const settle = (value: SetupPromptResult<T | T[]>): void => {
    if (settled) return;
    settled = true;
    disposeAll(control, subscriptions);
    resolver(value);
  };

  control.title = titleFor(options, context);
  control.step = context.phase;
  control.totalSteps = 4;
  control.placeholder = options.placeHolder;
  control.canSelectMany = options.canPickMany ?? false;
  control.matchOnDescription = options.matchOnDescription ?? false;
  control.matchOnDetail = options.matchOnDetail ?? false;
  control.ignoreFocusOut = options.ignoreFocusOut ?? false;
  control.items = nativeItems;
  const previous = context.previous;
  if (control.canSelectMany) {
    const selected = Array.isArray(previous) ? previous : [];
    const restored = nativeItems.filter(item => selected.some(value => sameItem(value, item.value)));
    control.selectedItems = restored;
    control.activeItems = restored;
  } else {
    const selected = Array.isArray(previous) ? previous[0] : previous;
    const item = nativeItems.find(candidate => sameItem(selected, candidate.value));
    if (item) {
      control.activeItems = [item];
    }
  }
  control.buttons = context.canGoBack ? [vscode.QuickInputButtons.Back] : [];
  subscriptions.push(control.onDidAccept(() => {
    const selected = [...control.selectedItems];
    if (control.canSelectMany) settle({ kind: "accepted", value: selected.map(item => item.value) });
    else if (selected[0]) settle({ kind: "accepted", value: selected[0].value });
  }));
  subscriptions.push(control.onDidTriggerButton(button => {
    if (button === vscode.QuickInputButtons.Back) {
      settle({ kind: "back", value: control.canSelectMany ? control.selectedItems.map(item => item.value) : control.selectedItems[0]?.value });
    }
  }));
  subscriptions.push(control.onDidHide(() => settle({ kind: "cancelled" })));
  control.show();
  return result;
}

export const nativeSetupPrompts: SetupPromptPresenter = {
  input,
  pick,
  open: async options => vscode.window.showOpenDialog(options),
};
