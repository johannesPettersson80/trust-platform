import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";
import { nativeSetupPrompts } from "../../controlStudio/nativeSetupPrompts";
import type { ChoiceItem, SetupQuestion } from "../../controlStudio/generated/SetupStepResult";

/** A control stub: the properties the presenter sets, and the events a test fires. */
function control(properties: Record<string, unknown>) {
  const listeners = new Map<string, Array<(value?: unknown) => void>>();
  const on = (name: string) => (listener: (value?: unknown) => void) => {
    listeners.set(name, [...(listeners.get(name) ?? []), listener]);
    return { dispose: () => undefined };
  };
  const fire = (name: string, value?: unknown) => (listeners.get(name) ?? []).forEach(listener => listener(value));
  return {
    value: "", buttons: [] as unknown[], selectedItems: [] as unknown[], show: () => undefined, dispose: () => undefined,
    onDidAccept: on("accept"), onDidTriggerButton: on("button"), onDidHide: on("hide"),
    fireAccept: () => fire("accept"), fireButton: (button: unknown) => fire("button", button), ...properties,
  } as any;
}

function question(properties: Partial<SetupQuestion>): SetupQuestion {
  return { key: "name", phase: 1, phaseName: "Loop and signals", title: "Loop name", kind: "text", items: [], ...properties } as SetupQuestion;
}

suite("Control Studio native setup prompts", () => {
  test("a text question returns the text the engineer typed, with the engine's refusal of the last answer under the box", async () => {
    const window = vscode.window as any;
    const original = window.createInputBox;
    const box = control({});
    window.createInputBox = () => box;
    try {
      const pending = nativeSetupPrompts.ask(question({}), { previous: "Tank", refusal: "Enter a value.", canGoBack: false });
      assert.strictEqual(box.value, "Tank", "the previous answer is shown again");
      assert.strictEqual(box.validationMessage, "Enter a value.");
      box.value = "  Tank 2 ";
      box.fireAccept();
      assert.deepStrictEqual(await pending, { kind: "answered", answer: "  Tank 2 " }, "the presenter sends the text as typed; the engine checks it");
    } finally { window.createInputBox = original; }
  });

  test("a choice returns the value of the item the engineer picks, and Back returns to the question before", async () => {
    const window = vscode.window as any;
    const original = window.createQuickPick;
    // The engine's choice values are any JSON (setup/prompts.rs, ChoiceItem.value): null stands for no setpoint, a text for a signal's address.
    const items: ChoiceItem[] = [{ index: 0, label: "No setpoint (open loop)", value: null }, { index: 1, label: "P1.Ref", description: "%IL1 · LREAL", value: "%IL1" }];
    const pick = control({});
    window.createQuickPick = () => pick;
    try {
      const pending = nativeSetupPrompts.ask(question({ kind: "choice", key: "setpoint", title: "Setpoint", items }), { previous: "%IL1", canGoBack: true });
      assert.deepStrictEqual(pick.items.map((item: any) => item.label), ["No setpoint (open loop)", "P1.Ref"]);
      assert.deepStrictEqual(pick.activeItems.map((item: any) => item.value), ["%IL1"], "the previous choice is active");
      assert.strictEqual(pick.buttons.length, 1, "Back is offered");
      pick.selectedItems = [pick.items[0]];
      pick.fireAccept();
      assert.deepStrictEqual(await pending, { kind: "answered", answer: null }, "the item's value is the answer, so no setpoint is null");
    } finally { window.createQuickPick = original; }
    const backPick = control({});
    window.createQuickPick = () => backPick;
    try {
      const pending = nativeSetupPrompts.ask(question({ kind: "choice", key: "setpoint", title: "Setpoint", items }), { canGoBack: true });
      backPick.fireButton(vscode.QuickInputButtons.Back);
      assert.deepStrictEqual(await pending, { kind: "back" });
    } finally { window.createQuickPick = original; }
  });

  test("a file question returns the text of the file the engineer chooses", async () => {
    const window = vscode.window as any;
    const original = window.showOpenDialog;
    const file = path.join(os.tmpdir(), `control-studio-adapter-${process.pid}.json`);
    fs.writeFileSync(file, "{\"kind\":\"adapter\"}");
    window.showOpenDialog = async () => [vscode.Uri.file(file)];
    try {
      const answer = await nativeSetupPrompts.ask(question({ kind: "file", key: "adapterMapping", title: "Controller adapter mapping" }), { canGoBack: false });
      assert.deepStrictEqual(answer, { kind: "answered", answer: "{\"kind\":\"adapter\"}" });
    } finally {
      window.showOpenDialog = original;
      fs.rmSync(file, { force: true });
    }
  });
});
