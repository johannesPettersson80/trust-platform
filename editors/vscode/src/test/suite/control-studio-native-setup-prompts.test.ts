import * as assert from "assert";
import * as vscode from "vscode";
import { nativeSetupPrompts } from "../../controlStudio/nativeSetupPrompts";

function signal<T>() {
  const listeners = new Set<(value: T) => void>();
  const event = ((listener: (value: T) => void) => {
    listeners.add(listener);
    return { dispose: () => listeners.delete(listener) };
  }) as vscode.Event<T>;
  return { event, fire: (value: T) => [...listeners].forEach(listener => listener(value)), size: () => listeners.size };
}

function fakeInputBox() {
  const accept = signal<void>();
  const change = signal<string>();
  const triggerButton = signal<vscode.QuickInputButton>();
  const hide = signal<void>();
  const box: any = {
    value: "",
    validationMessage: undefined,
    busy: false,
    onDidAccept: accept.event,
    onDidChangeValue: change.event,
    onDidTriggerButton: triggerButton.event,
    onDidHide: hide.event,
    show: () => undefined,
    dispose: () => { box.disposeCount += 1; },
    disposeCount: 0,
    fireAccept: () => accept.fire(undefined),
    fireChange: (value: string) => { box.value = value; change.fire(value); },
    fireHide: () => hide.fire(undefined),
  };
  return box;
}

suite("Control Studio native setup prompts", () => {
  test("revalidates the current draft while an older async acceptance is pending", async () => {
    const vscodeWindow = vscode.window as typeof vscode.window & { createInputBox: typeof vscode.window.createInputBox };
    const originalCreateInputBox = vscodeWindow.createInputBox;
    const box = fakeInputBox();
    vscodeWindow.createInputBox = () => box;
    let releaseOld!: () => void;
    const oldValidation = new Promise<string | undefined>(resolve => { releaseOld = () => resolve(undefined); });
    const validationValues: string[] = [];
    let result: ReturnType<typeof nativeSetupPrompts.input> | undefined;
    try {
      result = nativeSetupPrompts.input({ title: "Loop name", validateInput: async value => {
        validationValues.push(value);
        if (value === "old") return oldValidation;
        return value === "bad" ? "Invalid draft" : undefined;
      } }, { phase: 1, canGoBack: false, previous: "old" });
      box.fireAccept();
      await Promise.resolve();
      assert.strictEqual(box.busy, true, "the first asynchronous validation must be pending");

      box.fireChange("bad");
      await Promise.resolve();
      await Promise.resolve();
      assert.strictEqual(box.busy, false, "editing must stop the stale validation busy state");
      assert.strictEqual(box.validationMessage, "Invalid draft", "the current invalid draft must be validated immediately");
      assert.deepStrictEqual(validationValues, ["old", "bad"]);

      releaseOld();
      await Promise.resolve();
      await Promise.resolve();
      let settled = false;
      void result.then(() => { settled = true; });
      await Promise.resolve();
      assert.strictEqual(settled, false, "the obsolete valid answer must not settle the prompt");

      box.fireHide();
      const cancelled = await result;
      assert.deepStrictEqual(cancelled, { kind: "cancelled" });
      box.fireHide();
      assert.strictEqual(box.disposeCount, 1, "settlement must dispose the prompt exactly once");
    } finally {
      box.fireHide();
      releaseOld();
      await result;
      vscodeWindow.createInputBox = originalCreateInputBox;
    }
  });
});
