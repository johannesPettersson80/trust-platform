import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { execFile } from "child_process";
import { promisify } from "util";
import { pollValue } from "./plant-rendered-harness";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

export async function exerciseNativeCandidateExport(inner: EvaluateWebview, project: string,
  capture: (name: string) => Promise<void>): Promise<void> {
  const driver = process.env.TRUST_CONTROL_STUDIO_NATIVE_DIALOG_DRIVER;
  if (!driver) return;
  const original = vscode.window.showSaveDialog;
  let calls = 0;
  vscode.window.showSaveDialog = options => { calls++; return original(options); };
  const target = path.join(project, "reviewed-controller.json");
  try {
    assert.strictEqual(await inner("const b=d.querySelector('[data-auto-tune-export]');b.scrollIntoView({block:'center'});const r=b.getBoundingClientRect();if(b.disabled||!r.width||!r.height)return 'unavailable';b.click();return 'clicked';"), "clicked");
    await pollValue("native candidate Save invocation", async () => String(calls), value => value === "1", 10000);
    await promisify(execFile)("python3", [driver, target, path.join(path.dirname(project), "native-candidate-save.png")], { timeout: 25000 });
    await pollValue("candidate JSON file", async () => String(fs.existsSync(target)), value => value === "true", 10000);
    const result = JSON.parse(fs.readFileSync(target, "utf8"));
    assert.ok(Number.isFinite(result.candidate?.parameters?.proportionalGain), "Export retains the actual controller candidate");
    await capture("candidate-export-saved");
  } finally { vscode.window.showSaveDialog = original; }
}

/** Uses the real OS chooser; the optional driver operates its visible native controls. */
export async function exerciseNativePlotExport(inner: EvaluateWebview, project: string,
  capture: (name: string) => Promise<void>): Promise<void> {
  const driver = process.env.TRUST_CONTROL_STUDIO_NATIVE_DIALOG_DRIVER;
  if (!driver) return;
  const original = vscode.window.showSaveDialog;
  let calls = 0;
  vscode.window.showSaveDialog = options => { calls++; return original(options); };
  const target = path.join(project, "renamed-live-response.png");
  try {
    for (const cancel of [false, true]) {
      const previousCalls = calls;
      assert.strictEqual(await inner("const b=d.querySelector('[data-chart-action=export-figure]');const r=b.getBoundingClientRect();return String(!b.disabled&&r.width>0&&r.height>0);"), "true");
      await inner("d.querySelector('[data-chart-action=export-figure]').click();return 'export';");
      await pollValue("native Save invocation", async () => String(calls), value => Number(value) === previousCalls + 1, 10000);
      await promisify(execFile)("python3", [driver, cancel ? "" : target,
        path.join(path.dirname(project), cancel ? "native-cancel-dialog.png" : "native-save-dialog.png")], { timeout: 25000 });
      await pollValue(cancel ? "export cancellation" : "export saved", () => inner("return d.querySelector('[data-chart-export-status]')?.textContent??d.querySelector('[data-chart-status]')?.textContent??'';"),
        value => cancel ? /cancelled/i.test(value) : /saved|exported/i.test(value), 10000);
      if (!cancel) {
        assert.ok(fs.existsSync(target), "The user-selected named file must exist");
        assert.strictEqual(fs.readFileSync(target).subarray(1, 4).toString(), "PNG");
      }
      await capture(cancel ? "11-export-cancelled" : "10-export-saved");
    }
  } finally { vscode.window.showSaveDialog = original; }
}
