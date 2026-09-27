import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import type { ControlStudioWorkflowResult, WorkflowResultSample } from "../../controlStudio/workflowResult";
import { revealControlForCapture } from "./control-studio-capture-visibility";
import { pollValue, sleep } from "./plant-rendered-harness";

export type HotHeaterInner = (body: string) => Promise<string>;

export async function clickHotHeaterControl(inner: HotHeaterInner, selector: string): Promise<void> {
  await revealControlForCapture(inner, selector);
  assert.strictEqual(await inner(`const control=d.querySelector(${JSON.stringify(selector)});if(control.disabled)return 'disabled';control.click();return 'clicked';`), "clicked", selector);
}

export async function chooseHotHeaterSignal(inner: HotHeaterInner, symbol: string, selected: boolean): Promise<void> {
  const index = Number(await inner(`return String([...d.querySelectorAll('[data-control-signals] .signal-row')].findIndex(row=>[...row.querySelectorAll('.signal-secondary')].some(label=>label.textContent.trim().endsWith(${JSON.stringify(symbol)}))));`));
  assert.ok(index >= 0, `the visible Signals list must contain ${symbol}`);
  const selector = `[data-control-signals] .signal-row:nth-child(${index + 1}) input[type=checkbox]`;
  if (await inner(`return String(d.querySelector(${JSON.stringify(selector)}).checked);`) !== String(selected)) {
    await clickHotHeaterControl(inner, selector);
  }
  await pollValue(`selection ${symbol}`, () => inner(`return String(d.querySelector(${JSON.stringify(selector)})?.checked);`), value => value === String(selected), 5000);
}

export async function hotHeaterSelectedSignals(inner: HotHeaterInner): Promise<string[]> {
  return JSON.parse(await inner("return JSON.stringify(JSON.parse(d.querySelector('[data-control-studio]').dataset.controlChartSession).selectedSignals);"));
}

/** Reads actual plotted pixels. It neither replaces canvas APIs nor changes pixels.
 * The interior excludes the legend and axis labels so a changed checkbox/legend
 * alone cannot satisfy the rendered-curve assertion. */
export async function hotHeaterResponseRaster(inner: HotHeaterInner): Promise<string> {
  await sleep(250);
  const raster = await inner("const host=d.querySelector('[data-control-studio-chart]');if(!host.checkVisibility())return 'hidden';const canvases=[...host.querySelectorAll('canvas')];return JSON.stringify(canvases.map(canvas=>{const x=Math.floor(canvas.width*.08),y=Math.floor(canvas.height*.20),width=Math.floor(canvas.width*.84),height=Math.floor(canvas.height*.68);if(width<100||height<50)return {width,height};const pixels=canvas.getContext('2d').getImageData(x,y,width,height).data;let hash=2166136261;for(const value of pixels)hash=Math.imul(hash^value,16777619)>>>0;return {width,height,hash};}));");
  const layers = JSON.parse(raster);
  assert.ok(layers.length > 0 && layers.every((layer: { hash?: number }) => typeof layer.hash === "number"), "the actual Response canvas must be visible and readable");
  return raster;
}

/** Saves the untouched real chart canvas layers alongside workbench screenshots. */
export async function captureHotHeaterPlot(inner: HotHeaterInner, prefix: string): Promise<void> {
  const layers: string[] = JSON.parse(await inner("const host=d.querySelector('[data-control-studio-chart]');return JSON.stringify(host.checkVisibility()?[...host.querySelectorAll('canvas')].map(canvas=>canvas.toDataURL('image/png')):[]);"));
  layers.forEach((data, index) => fs.writeFileSync(`${prefix}-plot-${index}.png`, Buffer.from(data.split(",", 2)[1], "base64")));
}

function channel(sample: WorkflowResultSample, id: string): number {
  const found = sample.additional?.find(signal => signal.id === id);
  assert.ok(found && typeof found.value === "number" && Number.isFinite(found.value), `native record is missing finite ${id} at scan ${sample.scan}`);
  assert.strictEqual(found.quality, "good", `${id} must retain its acquisition quality`);
  return found.value;
}

function close(actual: unknown, expected: number, label: string): void {
  assert.ok(typeof actual === "number" && Math.abs(actual - expected) < 1e-8,
    `${label}: expected ${expected.toFixed(10)}, got ${String(actual)}`);
}

export function assertHotHeaterRecording(
  result: ControlStudioWorkflowResult, project: string, sequence: number, selected: readonly string[]
): Record<string, unknown> {
  assert.strictEqual(result.controller.instance, "P1.Controller");
  assert.strictEqual(result.source, "Simulation");
  assert.strictEqual(result.recording.acquisition?.method, "runtime_scan_buffer");
  assert.strictEqual(result.recording.acquisition?.completeness, "complete");
  assert.deepStrictEqual(result.recording.acquisition?.missingScanRanges, []);
  for (const id of selected) assert.ok(result.chartSession?.selectedSignals.includes(id), `saved chart session retains ${id}`);
  const active = result.recording.rawSamples.filter(sample => sample.additional?.some(signal => signal.id === "lesson-experiment" && signal.value === 1));
  const reference = fs.readFileSync(path.join(project, "tests/fixtures/hot-response-reference.csv"), "utf8")
    .split(/\r?\n/).filter(line => line && !line.startsWith("#")).map(line => line.split(",").map(Number));
  assert.strictEqual(reference.length, 700);
  assert.ok(active.length >= 700, `the complete 70-second active interval needs at least 700 native scans, got ${active.length}`);
  let previousRequest = 75;
  let previousApplied = 75;
  for (let index = 0; index < 700; index += 1) {
    const sample = active[index];
    const row = reference[index];
    assert.strictEqual(sample.quality, "good");
    if (index > 0) {
      assert.strictEqual(sample.scan, active[index - 1].scan + 1, "retained native scans must be consecutive");
      close(sample.elapsedSeconds - active[0].elapsedSeconds, index * 0.1, "native sample timing");
    }
    close(sample.sp, index < 100 ? 45 : 35, `setpoint event at sample ${index}`);
    close(sample.pv, row[0], `frozen PV sample ${index}`);
    close(channel(sample, "requested-power"), row[1], `current request sample ${index}`);
    close(channel(sample, "stored-correction"), row[2], `retained I sample ${index}`);
    if (sequence === 8) close(channel(sample, "tracking-correction"), row[3], `back-calculation sample ${index}`);
    close(channel(sample, "controller-command"), sequence === 7 ? row[1] : row[4], `actual PID command sample ${index}`);
    close(sample.appliedMv, previousApplied, `preceding applied feedback sample ${index}`);
    assert.ok(sample.appliedMv! >= 0 && sample.appliedMv! <= 100, "physical feedback stays within 0–100%");
    close(channel(sample, "paired-requested-power"), previousRequest, `paired request sample ${index}`);
    close(channel(sample, "unavailable-power"), previousRequest - previousApplied, `phase-aligned mismatch sample ${index}`);
    previousRequest = row[1]; previousApplied = row[4];
  }
  // The 70.1 s host envelope may include one more active PLC scan. Preserve the
  // fixed 700-row reference, then pair later observations with their own actual
  // preceding command instead of reusing reference row 699 at shutdown.
  const appliedCommand = (sample: WorkflowResultSample) => Math.max(0, Math.min(100, channel(sample, "controller-command")));
  previousRequest = channel(active[699], "requested-power");
  previousApplied = appliedCommand(active[699]);
  for (const sample of active.slice(700)) {
    close(sample.appliedMv, previousApplied, "extra active scan uses preceding applied command");
    close(channel(sample, "paired-requested-power"), previousRequest, "extra active scan request pairing");
    close(channel(sample, "unavailable-power"), previousRequest - previousApplied, "extra active scan mismatch");
    previousRequest = channel(sample, "requested-power");
    previousApplied = appliedCommand(sample);
  }
  const terminal = result.recording.rawSamples.filter(sample => sample.scan > active[active.length - 1].scan);
  assert.ok(terminal.some(sample => channel(sample, "controller-command") === 0), "record native terminal zero command");
  const firstStop = terminal.find(sample => channel(sample, "controller-command") === 0)!;
  close(firstStop.appliedMv, previousApplied, "terminal command precedes the following applied feedback publication");
  close(channel(firstStop, "paired-requested-power"), previousRequest, "terminal request pairing");
  return { sequence, nativeSamples: result.recording.rawSamples.length, activeSamples: active.length,
    firstScan: active[0].scan, lastActiveScan: active[active.length - 1].scan,
    maximumRequest: Math.max(...active.map(sample => channel(sample, "requested-power"))),
    maximumApplied: Math.max(...active.map(sample => sample.appliedMv!)),
    maximumPv: Math.max(...active.map(sample => sample.pv!)),
    retainedIntegralBeforeReturn: channel(active[99], "stored-correction"), terminalAppliedAtZeroCommand: firstStop.appliedMv,
    selectedSignals: result.chartSession?.selectedSignals, referenceTolerance: 1e-8 };
}
