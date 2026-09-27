import * as assert from "assert";
import * as vscode from "vscode";
import { ControlSetupSession } from "../../controlStudio/setupNavigation";
import type { SetupPromptContext, SetupPromptPresenter, SetupPromptResult } from "../../controlStudio/setupPromptTypes";

suite("Control Studio setup navigation", () => {
  test("Back revisits the prior answer while retaining input drafts and the imported file", async () => {
    const calls: { title?: string; context: SetupPromptContext }[] = [];
    const choices = [{ label: "Controller A" }, { label: "Controller B" }];
    let opens = 0, units = 0;
    const presenter: SetupPromptPresenter = {
      async input(options, context) {
        calls.push({ title: options.title, context });
        if (options.title === "Name") return { kind: "accepted", value: "Heater" };
        return ++units === 1 ? { kind: "back", value: "draft unit" } : { kind: "accepted", value: "°C" };
      },
      async pick<T extends string | vscode.QuickPickItem>(items: readonly T[], options: vscode.QuickPickOptions, context: SetupPromptContext) {
        calls.push({ title: options.title, context });
        return { kind: "accepted", value: items[0] } as SetupPromptResult<T | T[]>;
      },
      async open() { opens++; return [vscode.Uri.file("/adapter.json")]; },
    };
    const session = new ControlSetupSession(presenter);
    const result = await session.run(async () => {
      const name = await session.showInputBox({ title: "Name" });
      const controller = await session.showQuickPick(choices, { title: "Controller" });
      await session.showOpenDialog({ title: "Adapter" });
      session.phase = 2;
      const unit = await session.showInputBox({ title: "Unit" });
      return { name, controller, unit };
    });
    assert.deepStrictEqual(result, { name: "Heater", controller: choices[0], unit: "°C" });
    assert.deepStrictEqual(calls.map(call => call.title), ["Name", "Controller", "Unit", "Controller", "Unit"]);
    assert.strictEqual(calls[0].context.canGoBack, false);
    assert.deepStrictEqual(calls[3].context.previous, choices[0]);
    assert.strictEqual(calls[4].context.previous, "draft unit");
    assert.strictEqual(calls[3].context.phase, 1);
    assert.strictEqual(calls[4].context.phase, 2);
    assert.strictEqual(opens, 1, "going Back must reuse an unchanged adapter selection");
  });

  test("changing an earlier choice discards dependent answers and reselects its adapter", async () => {
    let picks = 0, units = 0, opens = 0;
    const presenter: SetupPromptPresenter = {
      async input() { return ++units === 1 ? { kind: "back", value: "old unit" } : { kind: "cancelled" }; },
      async pick<T extends string | vscode.QuickPickItem>(items: readonly T[]) {
        return { kind: "accepted", value: items[picks++] } as SetupPromptResult<T | T[]>;
      },
      async open() { opens++; return [vscode.Uri.file(`/adapter-${picks}.json`)]; },
    };
    const session = new ControlSetupSession(presenter);
    const unit = await session.run(async () => {
      await session.showQuickPick(["A", "B"], { title: "Controller" });
      await session.showOpenDialog({ title: "Adapter" });
      return session.showInputBox({ title: "Unit" });
    });
    assert.strictEqual(unit, undefined, "Cancel still cancels the setup");
    assert.strictEqual(opens, 2, "a different controller needs its own adapter selection");
  });
});
