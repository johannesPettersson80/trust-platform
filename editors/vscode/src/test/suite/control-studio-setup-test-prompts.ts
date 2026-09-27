import type * as vscode from "vscode";
import type { SetupPromptPresenter } from "../../controlStudio/setupPromptTypes";

type InputStub = (options: vscode.InputBoxOptions) => PromiseLike<string | undefined>;
type PickStub = (items: readonly (string | vscode.QuickPickItem)[], options: vscode.QuickPickOptions) => PromiseLike<unknown>;

/** Adapts deterministic focused-test answers to the explicit setup prompt port. */
export function setupTestPrompts(input: InputStub, pick: PickStub,
  open: (options: vscode.OpenDialogOptions) => PromiseLike<vscode.Uri[] | undefined> = async () => undefined): SetupPromptPresenter {
  return {
    input: async options => {
      const value = await input(options);
      return value === undefined ? { kind: "cancelled" } : { kind: "accepted", value };
    },
    pick: async (items, options) => {
      const value = await pick(items, options);
      return value === undefined ? { kind: "cancelled" } : { kind: "accepted", value: value as any };
    },
    open: async options => open(options),
  };
}
