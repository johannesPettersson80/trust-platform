import * as fs from "fs";
import * as vscode from "vscode";
import type { SetupQuestion } from "./generated/SetupStepResult";
import type { SetupPromptPresenter, SetupQuestionContext, SetupQuestionResult } from "./setupPromptTypes";

/*
 * The step-by-step setup's questions as VS Code shows them (specification 35, B1.3). The engine names each question, its choices and its
 * kind, and checks each answer. This presenter shows the question and the engine's refusal of the last answer, and returns the answer as the
 * engineer gave it: the text typed, the value of the chosen item, or the text of the file chosen. It validates nothing.
 */

export const nativeSetupPrompts: SetupPromptPresenter = {
  ask: (question, context) => {
    if (question.kind === "choice") return pick(question, context);
    if (question.kind === "file") return chooseFile(question, context);
    return typeText(question, context);
  },
};

const titleOf = (question: SetupQuestion): string => `${question.phaseName} · ${question.title}`;

function disposeAll(control: { dispose(): void }, subscriptions: vscode.Disposable[]): void {
  for (const subscription of subscriptions) subscription.dispose();
  control.dispose();
}

/** A question that takes text: a name, a unit, a number or a rate limit. The refusal of the last answer shows under the box. */
function typeText(question: SetupQuestion, context: SetupQuestionContext): Promise<SetupQuestionResult> {
  const control = vscode.window.createInputBox();
  const subscriptions: vscode.Disposable[] = [];
  let settled = false;
  let resolver!: (result: SetupQuestionResult) => void;
  const result = new Promise<SetupQuestionResult>(resolve => { resolver = resolve; });
  const settle = (value: SetupQuestionResult): void => {
    if (settled) return;
    settled = true;
    disposeAll(control, subscriptions);
    resolver(value);
  };
  control.title = titleOf(question);
  control.step = question.phase;
  control.totalSteps = 4;
  control.prompt = question.prompt ?? undefined;
  control.value = typeof context.previous === "string" ? context.previous : "";
  control.validationMessage = context.refusal;
  control.ignoreFocusOut = true;
  control.buttons = context.canGoBack ? [vscode.QuickInputButtons.Back] : [];
  subscriptions.push(control.onDidAccept(() => settle({ kind: "answered", answer: control.value })));
  subscriptions.push(control.onDidTriggerButton(button => {
    if (button === vscode.QuickInputButtons.Back) settle({ kind: "back", text: control.value });
  }));
  subscriptions.push(control.onDidHide(() => settle({ kind: "cancelled" })));
  control.show();
  return result;
}

/** A question that takes one of the items the engine offers: the item's value is the answer. The refusal of the last answer is shown as a message. */
function pick(question: SetupQuestion, context: SetupQuestionContext): Promise<SetupQuestionResult> {
  const control = vscode.window.createQuickPick<vscode.QuickPickItem & { value: unknown }>();
  const subscriptions: vscode.Disposable[] = [];
  let settled = false;
  let resolver!: (result: SetupQuestionResult) => void;
  const result = new Promise<SetupQuestionResult>(resolve => { resolver = resolve; });
  const settle = (value: SetupQuestionResult): void => {
    if (settled) return;
    settled = true;
    disposeAll(control, subscriptions);
    resolver(value);
  };
  const items = question.items.map(item => ({ label: item.label, description: item.description ?? undefined, value: item.value }));
  control.title = titleOf(question);
  control.step = question.phase;
  control.totalSteps = 4;
  control.items = items;
  const previous = items.find(item => item.value === context.previous);
  if (previous) control.activeItems = [previous];
  control.buttons = context.canGoBack ? [vscode.QuickInputButtons.Back] : [];
  if (context.refusal) void vscode.window.showErrorMessage(context.refusal);
  subscriptions.push(control.onDidAccept(() => {
    const selected = control.selectedItems[0];
    if (selected) settle({ kind: "answered", answer: selected.value });
  }));
  subscriptions.push(control.onDidTriggerButton(button => {
    if (button === vscode.QuickInputButtons.Back) settle({ kind: "back" });
  }));
  subscriptions.push(control.onDidHide(() => settle({ kind: "cancelled" })));
  control.show();
  return result;
}

/** The controller adapter mapping: the file the engineer picks is read, and the engine reads and checks its text. */
async function chooseFile(question: SetupQuestion, context: SetupQuestionContext): Promise<SetupQuestionResult> {
  if (context.refusal) void vscode.window.showErrorMessage(context.refusal);
  const selected = await vscode.window.showOpenDialog({ title: titleOf(question), canSelectMany: false,
    filters: { "Controller adapter JSON": ["json"] } });
  if (!selected?.[0]) return { kind: "cancelled" };
  return { kind: "answered", answer: await fs.promises.readFile(selected[0].fsPath, "utf8") };
}
