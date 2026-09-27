import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

/** Selects a destination at the native dialog boundary; the actual host must write it. */
export async function exportRecordedComparisonThroughHost(inner: EvaluateWebview, project: string): Promise<Buffer> {
  const target = path.join(project, "recorded-comparison.png");
  const original = vscode.window.showSaveDialog;
  let dialogs = 0;
  vscode.window.showSaveDialog = async options => {
    dialogs++;
    assert.ok(options?.defaultUri?.path.endsWith(".png"));
    return vscode.Uri.file(target);
  };
  try {
    await inner("d.querySelector('[data-chart-action=export-figure]').click();return 'export';");
    for (let attempt = 0; attempt < 100; attempt++) {
      const status = await inner("return d.querySelector('[data-engineer-response-status]').textContent;");
      if (/saved|exported as PNG/i.test(status) && fs.existsSync(target)) break;
      if (/could not|failed/i.test(status)) assert.fail(status);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.strictEqual(dialogs, 1, "Export must open exactly one destination dialog");
    assert.ok(fs.existsSync(target), "The real host must write the named PNG, without anchor interception");
    console.log("Recorded comparison exported through host:", target);
    return fs.readFileSync(target);
  } finally { vscode.window.showSaveDialog = original; }
}
