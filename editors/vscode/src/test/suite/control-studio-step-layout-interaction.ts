import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { execFile } from "child_process";
import { promisify } from "util";
import { pollValue, sleep } from "./plant-rendered-harness";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { exerciseVisiblePlotToolbar } from "./control-studio-toolbar-interactions";
import { exerciseNativePlotExport } from "./control-studio-native-export-interaction";

/** Reuses a fitted/imported study: changing presentation must not require another acquisition. */
export async function exerciseStepLayout(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>, captures: string): Promise<void> {
  const driver = process.env.TRUST_CONTROL_STUDIO_NATIVE_WINDOW_DRIVER;
  assert.ok(driver, "layout review requires a native window driver on the isolated display");
  const nativeWindow = async (bounds?: Record<string, number>) => JSON.parse((await promisify(execFile)("python3",
    [driver, ...(bounds ? [JSON.stringify(bounds)] : [])], { timeout: 10000 })).stdout);
  const originalBounds = await nativeWindow();
  const workbench = vscode.workspace.getConfiguration("workbench"), window = vscode.workspace.getConfiguration("window");
  const originalTheme = workbench.inspect<string>("colorTheme")?.globalValue;
  const originalZoom = window.inspect<number>("zoomLevel")?.globalValue;
  const observations: unknown[] = [], failures: string[] = [];
  const click = (selector: string) => inner(`var b=d.querySelector(${JSON.stringify(selector)});if(!b||b.disabled)throw Error('Unavailable: '+${JSON.stringify(selector)});b.click();return 'clicked';`);
  const stage = (name: string) => click(`[data-control-studio-mode][aria-label^="${name},"]`);
  const geometry = () => inner(`var plot=d.querySelector('.plot-view:not([hidden])');var canvas=plot?.querySelector('canvas');
    return JSON.stringify({width:d.documentElement.clientWidth,height:d.documentElement.clientHeight,scrollWidth:d.documentElement.scrollWidth,
      theme:d.body.dataset.vscodeThemeKind,ratio:d.defaultView.devicePixelRatio,plot:canvas?.getBoundingClientRect().toJSON(),
      fields:[...d.querySelectorAll('[data-step-field]')].map(f=>({name:f.dataset.stepField,rect:f.getBoundingClientRect().toJSON()}))});`);
  try {
    for (const variant of [
      { name: "dark", theme: "Default Dark Modern", kind: "vscode-dark", width: 1600, height: 1180, zoom: 0 },
      { name: "light", theme: "Default Light Modern", kind: "vscode-light", width: 1600, height: 1180, zoom: 0 },
      { name: "contrast", theme: "Default High Contrast", kind: "vscode-high-contrast", width: 1600, height: 1180, zoom: 0 },
      { name: "narrow", theme: "Default Dark Modern", kind: "vscode-dark", width: 640, height: 1000, zoom: 0 },
      { name: "zoom", theme: "Default Dark Modern", kind: "vscode-dark", width: 1600, height: 1180, zoom: Math.log(2) / Math.log(1.2) },
    ]) {
      const actualBounds = await nativeWindow({ ...originalBounds, x: 0, y: 0, width: variant.width, height: variant.height });
      assert.strictEqual(actualBounds.width, variant.width);
      assert.strictEqual(actualBounds.height, variant.height);
      await workbench.update("colorTheme", variant.theme, vscode.ConfigurationTarget.Global);
      await window.update("zoomLevel", variant.zoom, vscode.ConfigurationTarget.Global);
      await pollValue("rendered theme", () => inner("return d.body.dataset.vscodeThemeKind??'';"), value => value === variant.kind, 10000);
      await sleep(400);
      await stage("Identify");
      await click('[data-plot-tab="fit"]');
      await inner("d.querySelector('[data-step-response-settings]').open=false;d.querySelector('[data-step-response-evidence]').open=false;d.querySelector('.workspace').scrollTop=0;d.querySelector('[data-control-studio]').scrollTop=0;return 'overview';");
      const overview = JSON.parse(await geometry());
      if (overview.scrollWidth > overview.width + 1) failures.push(`${variant.name}: the page overflows horizontally`);
      if (variant.name === "narrow") assert.ok(overview.width <= 640, "the actual native window must reach narrow layout");
      if (variant.name === "narrow" && (overview.plot?.height ?? 0) < 300) failures.push("narrow: reference labels need a readable plot, at least 300px high for this fixture");
      if (variant.name === "zoom") assert.ok(Math.abs(overview.ratio - 2) < .05, "the editor must actually render at 200% zoom");
      if (variant.zoom === 0 && variant.width === 1600) {
        assert.ok(overview.height >= 1080, "wide review must reach the specification's 1080px viewport");
        if ((overview.plot?.height ?? 0) < 420) failures.push(`${variant.name}: the fitted plot must be at least 420px high`);
      }
      await capture(`layout-${variant.name}-fit`);
      await inner("d.querySelector('[data-step-response-settings]').open=true;d.querySelector('[data-step-field]').closest('label').scrollIntoView({block:'center'});return 'settings';");
      const settings = JSON.parse(await geometry());
      for (const field of settings.fields) {
        if (field.rect.width <= 0 || field.rect.left < 0 || field.rect.right > settings.width + 1) failures.push(`${variant.name}: ${field.name} must fit the available width`);
      }
      const reachable = JSON.parse(await inner("var f=d.querySelector('[data-step-field]').closest('label').getBoundingClientRect();var w=d.querySelector('.workspace').getBoundingClientRect();var n=d.querySelector('.modes').getBoundingClientRect();return JSON.stringify({field:f.toJSON(),workspace:w.toJSON(),navigation:n.toJSON(),height:d.documentElement.clientHeight,visible:f.top>=Math.max(0,w.top,n.bottom)&&f.bottom<=Math.min(d.documentElement.clientHeight,w.bottom)});"));
      if (!reachable.visible) failures.push(`${variant.name}: the first labelled setting must scroll fully into view`);
      await capture(`layout-${variant.name}-settings`);
      if (variant.name === "narrow" || variant.name === "zoom") {
        await inner("d.querySelector('[data-step-field=\"timeConstant.step\"]').closest('label').scrollIntoView({block:'center'});return 'last settings';");
        await capture(`layout-${variant.name}-settings-end`);
      }
      await inner("d.querySelector('[data-step-response-settings]').open=false;d.querySelector('[data-step-response-evidence]').open=true;d.querySelector('[data-step-response-evidence]').scrollIntoView({block:'end'});return 'measurements';");
      await capture(`layout-${variant.name}-measurements`);
      await stage("Validate");
      await click('[data-plot-tab="recorded-response"]');
      await inner("d.querySelector('.workspace').scrollTop=0;d.querySelector('[data-control-studio]').scrollTop=0;return 'recorded response';");
      const recorded = JSON.parse(await geometry());
      if (recorded.scrollWidth > recorded.width + 1) failures.push(`${variant.name}: retained response overflows horizontally`);
      await capture(`layout-${variant.name}-recording`);
      observations.push({ variant, overview, settings, reachable, recorded });
    }
    fs.writeFileSync(path.join(captures, "layout-interactions.json"), JSON.stringify({ observations, failures }, null, 2));
    assert.deepStrictEqual(failures, [], "each reviewed theme and layout must preserve usable plots and fields");
  } finally {
    await workbench.update("colorTheme", originalTheme, vscode.ConfigurationTarget.Global);
    await window.update("zoomLevel", originalZoom, vscode.ConfigurationTarget.Global);
    await nativeWindow(originalBounds);
  }
}

export async function exerciseStepPlots(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>, captures: string): Promise<void> {
  assert.ok(process.env.TRUST_CONTROL_STUDIO_NATIVE_DIALOG_DRIVER, "plot acceptance requires the existing native dialog driver");
  const project = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  assert.ok(project, "native export requires the opened test project");
  const findings: unknown[] = [], failures: string[] = [];
  for (const [stage, plot] of [["Identify", "fit"], ["Validate", "recorded-response"]]) {
    await inner(`d.querySelector('[data-control-studio-mode][aria-label^="${stage},"]').click();d.querySelector('[data-plot-tab="${plot}"]').click();d.querySelector('[data-control-studio]').scrollTop=0;return 'selected ${plot}';`);
    await exerciseVisiblePlotToolbar(inner, async name => {
      await capture(name);
      if (plot === "fit" && /-(inspect|zoom-in|zoom-out|fit)$/.test(name)) {
        const tooltip = await inner("return [...d.querySelectorAll('[data-identification-plot=fit] div')].filter(e=>e.textContent.includes('Measured PV')&&e.textContent.includes('Predicted PV')&&e.getBoundingClientRect().height>0&&d.defaultView.getComputedStyle(e).visibility==='visible'&&d.defaultView.getComputedStyle(e).display!=='none').map(e=>e.textContent).join(' ');");
        if (name.endsWith("-inspect")) assert.match(tooltip, /Measured PV.*Predicted PV/, "Inspect must show the actual fitted comparison values");
        else if (tooltip) failures.push(`${name}: the previous inspection must clear when its window changes`);
        findings.push({ name, tooltip });
      }
    }, plot, true);
    await exerciseNativePlotExport(inner, project, name => capture(`${plot}-${name}`));
    for (const action of ["save", "cancel"]) {
      fs.copyFileSync(path.join(path.dirname(project), `native-${action}-dialog.png`), path.join(captures, `${plot}-native-${action}-dialog.png`));
    }
    const exported = path.join(project, "renamed-live-response.png");
    const retained = path.join(captures, `${plot}-export.png`);
    fs.copyFileSync(exported, retained);
    findings.push({ plot, exported: retained, bytes: fs.statSync(retained).size });
    // The second export chooses the same name in a fresh dialog; remove only this test-created output.
    fs.unlinkSync(exported);
    if (plot === "recorded-response") {
      assert.strictEqual(await inner("var b=d.querySelector('[data-chart-action=table]');return String(!!b&&!b.hidden&&!b.disabled);"), "true", "retained response must expose its Samples table");
      await inner("d.querySelector('[data-chart-action=table]').click();return 'samples';");
      const table = JSON.parse(await inner("var t=d.querySelector('[data-chart-table]');return JSON.stringify({hidden:t.hidden,headings:[...t.querySelectorAll('th')].map(c=>c.textContent),rows:t.querySelectorAll('tbody tr').length,last:t.querySelector('tbody tr:last-child')?.textContent});"));
      assert.strictEqual(table.hidden, false);
      assert.ok(table.rows > 0);
      assert.ok(table.headings.some((heading: string) => /°C/.test(heading)) && table.headings.some((heading: string) => /%/.test(heading)), "recorded table retains output/input units");
      await capture("recorded-response-samples");
      await inner("d.querySelector('[data-plot-tab=signals]').click();return 'signals';");
      assert.strictEqual(await inner("return String(d.querySelector('[data-chart-table]').hidden);"), "true", "leaving the response closes its sample table");
      await capture("recorded-response-signals");
      await inner("d.querySelector('[data-plot-tab=recorded-response]').click();return 'return to recording';");
      findings.push({ table });
    }
  }
  fs.writeFileSync(path.join(captures, "plot-interactions.json"), JSON.stringify(findings, null, 2));
  assert.deepStrictEqual(failures, [], "plot tools must keep inspection consistent with the visible window");
}
