import * as vscode from "vscode";
import type { SetupDialogs, SetupPhase, SetupPromptPresenter, SetupPromptResult } from "./setupPromptTypes";

interface Answer { key: string; value: unknown; editable: boolean; accepted: boolean }
class BackNavigation { constructor(readonly index: number) {} }
const fingerprint = (value: unknown): string => JSON.stringify(value) ?? "undefined";

/** Rebuilds only an in-memory draft on Back; the caller owns the final save. */
export class ControlSetupSession implements SetupDialogs {
  phase: SetupPhase = 1;
  private answers: Answer[] = [];
  private cursor = 0;
  private replayUntil = 0;
  constructor(private readonly presenter: SetupPromptPresenter) {}

  async run<T>(build: () => Promise<T>): Promise<T> {
    for (;;) {
      this.cursor = 0;
      this.phase = 1;
      try { return await build(); }
      catch (error) {
        if (!(error instanceof BackNavigation)) throw error;
        this.replayUntil = error.index;
      }
    }
  }

  showInputBox(options: vscode.InputBoxOptions = {}): Promise<string | undefined> {
    return this.answer<string>(`input:${options.title}`, context => this.presenter.input(options, context),
      async value => {
        if (typeof value !== "string") return false;
        const message = await options.validateInput?.(value);
        return !message || typeof message !== "string" && message.severity !== vscode.InputBoxValidationSeverity.Error;
      });
  }

  showQuickPick<T extends vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions & { canPickMany: true }): Promise<T[] | undefined>;
  showQuickPick<T extends vscode.QuickPickItem>(items: readonly T[], options?: vscode.QuickPickOptions): Promise<T | undefined>;
  showQuickPick(items: readonly string[], options: vscode.QuickPickOptions & { canPickMany: true }): Promise<string[] | undefined>;
  showQuickPick(items: readonly string[], options?: vscode.QuickPickOptions): Promise<string | undefined>;
  showQuickPick<T extends string | vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions = {}): Promise<T | T[] | undefined> {
    return this.answer<T | T[]>(`pick:${options.title}:${Boolean(options.canPickMany)}:${fingerprint(items)}`,
      context => this.presenter.pick(items, options, context), async () => true);
  }

  async showOpenDialog(options: vscode.OpenDialogOptions): Promise<vscode.Uri[] | undefined> {
    const index = this.cursor++;
    const key = `file:${this.phase}:${fingerprint(options)}`;
    const previous = this.answers[index];
    if (previous?.key === key && previous.accepted) return previous.value as vscode.Uri[];
    const selected = await this.presenter.open(options);
    if (selected) this.answers[index] = { key, value: selected, accepted: true, editable: false };
    return selected;
  }

  private async answer<T>(identity: string,
    show: (context: { phase: SetupPhase; canGoBack: boolean; previous?: unknown }) => Promise<SetupPromptResult<T>>,
    valid: (value: unknown) => Promise<boolean>): Promise<T | undefined> {
    const index = this.cursor++;
    const key = `${this.phase}:${identity}`;
    let previous: Answer | undefined = this.answers[index];
    if (previous?.key !== key) {
      this.answers.splice(index);
      previous = undefined;
    }
    if (index < this.replayUntil && previous?.accepted && await valid(previous.value)) return previous.value as T;
    this.replayUntil = Math.min(this.replayUntil, index);
    let backIndex = index - 1;
    while (backIndex >= 0 && !this.answers[backIndex]?.editable) backIndex--;
    const result = await show({ phase: this.phase, canGoBack: backIndex >= 0, previous: previous?.value });
    if (result.kind === "cancelled") return undefined;
    if (result.kind === "back") {
      this.answers[index] = { key, value: result.value, editable: true, accepted: false };
      if (backIndex < 0) return undefined;
      throw new BackNavigation(backIndex);
    }
    if (previous && fingerprint(previous.value) !== fingerprint(result.value)) this.answers.splice(index + 1);
    this.answers[index] = { key, value: result.value, editable: true, accepted: true };
    return result.value;
  }
}
