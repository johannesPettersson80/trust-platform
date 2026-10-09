import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import type { ControlLoopDefinition } from "../../controlStudio/loopDefinition";
import type { LoopRecording } from "../../controlStudio/loopMonitor";
import { RuntimeRecordingSession } from "../../controlStudio/runtimeRecording";
import type { WorkflowResultSample } from "../../controlStudio/workflowResult";
import { inspectAttachedLoop, prepareAttachedProject, startAttachedRuntime } from "./control-studio-attached-runtime";
import { requiredEnvironment, sleep } from "./plant-rendered-harness";

/** Both runs record this much process time (the water heater's task is 100 ms, so 300 scans). */
const RECORDING_PROCESS_MS = 30_000;
/**
 * Each run starts its recording as soon as the runtime reaches this resource scan. Scan numbers count from the runtime's
 * start, so without this both 300-scan recordings could start far apart in process time at 10x and share few scans.
 */
const START_SCAN = 20;
const MINIMUM_COMMON_SCANS = 250;

interface RunRecording {
  readonly timeScale: number;
  readonly endpoint: string;
  readonly startScan: number;
  readonly recording: LoopRecording;
  readonly samples: readonly WorkflowResultSample[];
}

interface Comparison {
  readonly commonScans: number;
  readonly firstDifference?: { readonly scan: number; readonly timeScale1: unknown; readonly timeScale10: unknown };
}

suite("Control Studio speed equivalence", () => {
  test("records the same scans at time scale 1 and 10", async function () {
    // A rendered run of this journey sets both flags; the main run never has the fixture project open.
    if (process.env.TRUST_CONTROL_STUDIO_SPEED_EQUIVALENCE !== "1" || process.env.TRUST_PLANT_RENDERED_TEST !== "1") this.skip();
    this.timeout(240000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const evidence = path.dirname(requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT"));
    const slow = await recordRun(project, 1);
    const fast = await recordRun(project, 10);
    const comparison = compareRecordings(slow.samples, fast.samples);
    writeEvidence(evidence, [slow, fast], comparison);
    assert.ok(comparison.commonScans >= MINIMUM_COMMON_SCANS,
      `Only ${comparison.commonScans} scans are common to both recordings; at least ${MINIMUM_COMMON_SCANS} are required `
      + `(the 1x run starts at scan ${slow.startScan}, the 10x run at scan ${fast.startScan}).`);
    if (comparison.firstDifference) {
      assert.fail(`The recordings first differ at scan ${comparison.firstDifference.scan}: `
        + `1x ${JSON.stringify(comparison.firstDifference.timeScale1)} vs 10x ${JSON.stringify(comparison.firstDifference.timeScale10)}`);
    }
  });
});

/** Starts the project's runtime at one time scale, records RECORDING_PROCESS_MS of process time, and stops the runtime. */
async function recordRun(project: string, timeScale: number): Promise<RunRecording> {
  prepareAttachedProject(project);
  const runtime = await startAttachedRuntime(project, { timeScale, label: `Speed equivalence at ${timeScale}x` });
  try {
    const loop = JSON.parse(fs.readFileSync(path.join(project, "control-loop.json"), "utf8")) as ControlLoopDefinition;
    const startScan = await waitForScan(runtime.endpoint, loop, START_SCAN);
    const session = await RuntimeRecordingSession.start(loop, undefined, RECORDING_PROCESS_MS);
    await followUntilProcessTime(session, RECORDING_PROCESS_MS);
    const recording = await session.stop();
    await session.release();
    return { timeScale, endpoint: runtime.endpoint, startScan, recording, samples: recording.rawSamples ?? [] };
  } finally {
    await runtime.stop();
  }
}

async function waitForScan(endpoint: string, loop: ControlLoopDefinition, target: number): Promise<number> {
  const deadline = Date.now() + 60_000;
  let latest = -1;
  while (Date.now() < deadline) {
    const sample = await inspectAttachedLoop(endpoint, loop);
    if (sample) {
      latest = sample.scan;
      if (latest >= target) return latest;
    }
    await sleep(20);
  }
  throw new Error(`The runtime reached scan ${latest} only; scan ${target} was not reached within 60 s.`);
}

/** Follows the recording until its process time reaches the planned length, or the runtime has stopped it. */
async function followUntilProcessTime(session: RuntimeRecordingSession, processMs: number): Promise<void> {
  const deadline = Date.now() + 120_000;
  for (;;) {
    const page = await session.follow();
    if (page.state !== "recording" || page.elapsedMs >= processMs) return;
    if (Date.now() >= deadline) throw new Error(`The recording reached only ${page.elapsedMs} ms of process time in 120 s.`);
    await sleep(100);
  }
}

/** The recorded values of one scan: numbers, qualities, controller mode and the extra channels. Wall-clock fields are left out. */
function recordedValues(sample: WorkflowResultSample): unknown {
  return {
    sp: sample.sp,
    pv: sample.pv,
    requestedMv: sample.requestedMv,
    appliedMv: sample.appliedMv,
    quality: sample.quality,
    controllerMode: sample.controllerMode,
    additional: sample.additional ?? [],
  };
}

function compareRecordings(slow: readonly WorkflowResultSample[], fast: readonly WorkflowResultSample[]): Comparison {
  const fastByScan = new Map(fast.map((sample) => [sample.scan, sample] as const));
  const shared = slow.flatMap((sample) => {
    const other = fastByScan.get(sample.scan);
    return other ? [{ slow: sample, fast: other }] : [];
  });
  const difference = shared.find((pair) => JSON.stringify(recordedValues(pair.slow)) !== JSON.stringify(recordedValues(pair.fast)));
  return {
    commonScans: shared.length,
    ...(difference
      ? { firstDifference: { scan: difference.slow.scan, timeScale1: recordedValues(difference.slow), timeScale10: recordedValues(difference.fast) } }
      : {}),
  };
}

/** Writes both recordings and the comparison next to the screenshots, before the assertions run. */
function writeEvidence(folder: string, runs: readonly RunRecording[], comparison: Comparison): void {
  fs.mkdirSync(folder, { recursive: true });
  for (const run of runs) {
    fs.writeFileSync(path.join(folder, `speed-equivalence-${run.timeScale}x-recording.json`), JSON.stringify({
      timeScale: run.timeScale,
      endpoint: run.endpoint,
      startScan: run.startScan,
      evidence: run.recording.evidence,
      samples: run.samples,
    }, null, 2));
  }
  fs.writeFileSync(path.join(folder, "speed-equivalence-summary.json"), JSON.stringify({
    journey: "Control Studio speed equivalence",
    specification: "docs/specs/33-simulation-plant-models.md section 6.1 (results do not depend on the speed)",
    processDurationMs: RECORDING_PROCESS_MS,
    minimumCommonScans: MINIMUM_COMMON_SCANS,
    commonScans: comparison.commonScans,
    identical: comparison.firstDifference === undefined,
    firstDifference: comparison.firstDifference ?? null,
    runs: runs.map((run) => ({
      timeScale: run.timeScale,
      startScan: run.startScan,
      sampleCount: run.samples.length,
      firstScan: run.samples[0]?.scan ?? null,
      lastScan: run.samples[run.samples.length - 1]?.scan ?? null,
      completeness: run.recording.evidence.completeness,
    })),
  }, null, 2));
}
