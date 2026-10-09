import { AcquisitionLimitsRequired } from "./acquisitionSettings";
import { classifyControlExperimentStatus, type ControlExperimentClient, type ControlExperimentPlan } from "./experiment";
import { CONNECTION_LOSS_TIMEOUT_MS, MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS, RECORDING_DURATION_LIMIT_MS, recordPeriodExecutions } from "./controlStudioLimits";
import type { ControlLoopDefinition } from "./loopDefinition";
import type { LoopRecording } from "./loopMonitor";
import type { RecordingFollowPage, RecordingRecoveryOwner } from "./runtimeRecording";
import {
  hasSettled, isSteady, leftNoiseBand, mean, medianSpacing, MINIMUM_WINDOW_SAMPLES, noiseSigma, observedTimeScale,
  trailingWindow, trendSlope, baselineWindowSeconds, type TimedValue,
} from "./settleDetection";

/** Process time comes from the recording's samples. The wall clock only supervises and paces, and is injected for tests. */
export interface CampaignClock {
  now(): number;
  wait(milliseconds: number): Promise<void>;
}

export const systemClock: CampaignClock = {
  now: () => Date.now(),
  wait: milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds)),
};

/** What a campaign reads from its recording; the runtime's recording session provides it. */
export interface RecordingFollower {
  follow(): Promise<RecordingFollowPage>;
  stop(): Promise<LoopRecording>;
  recoveryOwner(): RecordingRecoveryOwner;
}

/** The steady baseline that the step is judged against (B3.4 step 2). */
export interface BaselineSummary {
  readonly mean: number;
  readonly noise: number;
  /** Half the baseline's peak-to-peak range: the size of a relay's switching band and of the mode's response allowance. */
  readonly halfRange: number;
  readonly spacingSeconds: number;
  readonly windowSeconds: number;
}

/** What resuming a campaign needs (A8.5). It is journalled before each dispatch, beside the loop revision and mode checkpoint. */
export interface IdentificationCampaignJournal {
  readonly schemaVersion: 1;
  readonly loopRevision: string;
  readonly phase: "baseline" | "step";
  readonly recording: RecordingRecoveryOwner;
  readonly plan?: ControlExperimentPlan;
  readonly baseline?: BaselineSummary;
  readonly stepStartSeconds?: number;
  /** Plant inputs that still follow this one. A resumed campaign finishes only the input it was stepping. */
  readonly channelsRemaining: number;
}

export interface IdentificationCampaignPersistence {
  read(): IdentificationCampaignJournal | undefined;
  write(value: IdentificationCampaignJournal | undefined): Promise<void>;
}

/** The runtime's view of an experiment as the campaign reads it. */
export interface ExperimentState {
  status: string;
  restoration: string;
  stop_reason?: string | null;
}

/** Stop reasons of an experiment that reached its planned end: the runtime's duration end, or the early finish of B3.4. */
const NORMAL_STOP_REASONS = ["duration_reached", "duration_complete", "finished_early"];
const STOP_REASON_TEXT: Readonly<Record<string, string>> = {
  user_abort: "it was aborted",
  mode_changed: "the loop left the mode the experiment requires",
  physical_approval_required: "physical approval was not given",
  stale_scan: "runtime data went stale",
};
const POLL_MILLISECONDS = 100;
const PROGRESS_INTERVAL_SECONDS = 10;
const RATE_WINDOW_SAMPLES = 200;
/** The baseline may take up to five minutes, or a fifth of the maximum duration when that is longer (B3.4 step 2). */
const BASELINE_BUDGET_MINIMUM_SECONDS = 300;

/** The maximum duration of a campaign in milliseconds: the declared maximum, within the limits of A7. */
export function campaignMaximumMs(loop: ControlLoopDefinition): number {
  const seconds = loop.experiment?.durationSeconds;
  if (seconds === undefined || !Number.isFinite(seconds) || seconds <= 0) throw new AcquisitionLimitsRequired();
  const milliseconds = Math.round(seconds * 1_000);
  if (milliseconds < MINIMUM_EXPERIMENT_DURATION_MS || milliseconds > MAXIMUM_EXPERIMENT_DURATION_MS) {
    throw new Error("The maximum experiment duration must be between 1 s and 24 h.");
  }
  return milliseconds;
}

/** How long Identify waits for a steady baseline: the longer of five minutes and a fifth of the maximum duration (B3.4 step 2). */
export function baselineBudgetMs(maximumMs: number): number {
  return Math.max(BASELINE_BUDGET_MINIMUM_SECONDS * 1_000, 0.2 * maximumMs);
}

/** The planned duration of a campaign's recording: the baseline wait, then the maximum duration of the steps (B3.4), within the
 * recording limit (A7). The record period follows from it (A4.2). */
export function campaignRecordingMs(loop: ControlLoopDefinition): number {
  const maximumMs = campaignMaximumMs(loop);
  return Math.min(RECORDING_DURATION_LIMIT_MS, baselineBudgetMs(maximumMs) + maximumMs);
}

/** A missing limit throws an error that names the field (B3.4 step 1). */
export function requireCampaignLimits(loop: ControlLoopDefinition): void {
  const { outputMinimum, outputMaximum } = loop.limits;
  if (!Number.isFinite(outputMinimum) || !Number.isFinite(outputMaximum) || outputMinimum >= outputMaximum) {
    throw new Error("The plant input limits are missing: limits.outputMinimum and limits.outputMaximum must be set, with the minimum below the maximum.");
  }
  const range = loop.signals.processVariable.range;
  if (!range || !Number.isFinite(range.minimum) || !Number.isFinite(range.maximum) || range.minimum >= range.maximum) {
    throw new Error(`The permitted range of ${loop.signals.processVariable.symbol} is missing: signals.processVariable.range must be set.`);
  }
}

/** Why a finished experiment does not qualify its recording, or undefined for a normal end (B3.4 step 6). */
export function experimentStopFailure(state: ExperimentState): string | undefined {
  if (state.status === "completed" && NORMAL_STOP_REASONS.includes(String(state.stop_reason))) return undefined;
  const reason = state.stop_reason ?? "unknown";
  return `The experiment stopped before its hold ended: ${STOP_REASON_TEXT[reason] ?? `stop reason ${reason}`}. The recording is kept and marked not qualified.`;
}

/** A restoration is confirmed only by the runtime's restored (or not required) state. */
export function restorationConfirmed(state: ExperimentState): boolean {
  return ["restored", "not_required"].includes(state.restoration);
}

/** A duration in plain words: "45 s", "12 min", "3 h 50 min". */
export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
}

/** A rate of change per minute with two significant figures, with its unit: "0.52 degC/min". */
export function formatRate(perSecond: number, unit: string): string {
  const perMinute = Number((Math.abs(perSecond) * 60).toPrecision(2));
  return `${perMinute}${unit ? ` ${unit}` : ""}/min`;
}

export interface CampaignConfig {
  readonly loop: ControlLoopDefinition;
  readonly recorder: RecordingFollower;
  readonly experiment: ControlExperimentClient;
  readonly clock: CampaignClock;
  readonly progress: (text: string) => Promise<void>;
  readonly cancelled: () => boolean;
  /** A loop that was in Auto with a declared setpoint is offered a setpoint step when its output keeps moving (B3.4 step 2). */
  readonly offerSetpointStep: boolean;
  readonly expectedSettlingSeconds?: number;
}

type Output = CampaignConfig["loop"]["signals"]["processVariable"];

/** One live campaign on process time. The recording's samples decide every phase; the wall clock only supervises (A3.2). */
export class IdentificationCampaign {
  private lastSampleAt: number;
  private first: { readonly wall: number; readonly seconds: number } | undefined;
  private newestSeconds = 0;
  private reportedSeconds = Number.NEGATIVE_INFINITY;

  constructor(private readonly config: CampaignConfig) {
    this.lastSampleAt = config.clock.now();
  }

  /** Process time of the newest sample followed so far: the step's reference and the campaign's elapsed time. */
  get seconds(): number { return this.newestSeconds; }

  /** Follows the recording until the output is steady over one window, or refuses it with its rate (B3.4 step 2). */
  async baseline(maximumMs: number): Promise<BaselineSummary> {
    const maximumSeconds = maximumMs / 1_000;
    const budgetSeconds = baselineBudgetMs(maximumMs) / 1_000;
    const output = this.config.loop.signals.processVariable;
    const range = output.range.maximum - output.range.minimum;
    const nominalSeconds = this.config.loop.controller.taskPeriodMs / 1_000;
    const points: TimedValue[] = [];
    this.lastSampleAt = this.config.clock.now();
    for (;;) {
      this.checkCancelled();
      points.push(...this.observe((await this.follow()).rawSamples));
      const spacing = medianSpacing(points.slice(-RATE_WINDOW_SAMPLES), nominalSeconds);
      const windowSeconds = baselineWindowSeconds(maximumSeconds, spacing, this.config.expectedSettlingSeconds);
      const recent = trailingWindow(points, windowSeconds);
      const span = points.length > 1 ? points[points.length - 1].seconds - points[0].seconds : 0;
      if (span >= windowSeconds && recent.length >= MINIMUM_WINDOW_SAMPLES) {
        const values = recent.map(point => point.value);
        const noise = noiseSigma(values);
        if (isSteady(recent, noise, range)) {
          const halfRange = (values.reduce((high, value) => Math.max(high, value), -Infinity)
            - values.reduce((low, value) => Math.min(low, value), Infinity)) / 2;
          return { mean: mean(values), noise, halfRange, spacingSeconds: spacing, windowSeconds };
        }
      }
      const rateWindow = recent.length >= 2 ? recent : points;
      if (span >= budgetSeconds) {
        const offer = this.config.offerSetpointStep ? ", or identify with a setpoint step" : "";
        throw new Error(`${output.symbol} is still changing by ${formatRate(trendSlope(rateWindow), output.unit)}. Wait for it to settle${offer}.`);
      }
      await this.reportBaseline(output, rateWindow);
      await this.pace();
    }
  }

  /** Holds the step until it has settled, then finishes it early; returns the runtime's terminal state once the original command
   * is restored (B3.4 steps 4 to 6). Only a single-step experiment is finished early; a pulse, PRBS or relay runs its duration. */
  async hold(plan: ControlExperimentPlan, baseline: BaselineSummary, stepSeconds: number): Promise<ExperimentState> {
    const single = plan.trajectory.length === 1 && ["setpoint_step", "load_step", "mv_step"].includes(plan.kind);
    const points: TimedValue[] = [];
    let leftBand = false;
    let finished = false;
    this.lastSampleAt = this.config.clock.now();
    this.reportedSeconds = Number.NEGATIVE_INFINITY;
    for (;;) {
      if (this.config.cancelled()) await this.config.experiment.abort(plan.id);
      const state = await this.config.experiment.status(plan.id) as ExperimentState;
      const phase = classifyControlExperimentStatus(state);
      if (phase === "invalid") throw new Error("The runtime returned an invalid experiment status.");
      for (const value of this.observe((await this.follow()).rawSamples)) {
        if (value.seconds <= stepSeconds) continue;
        points.push(value);
        leftBand = leftBand || leftNoiseBand(value.value, baseline.mean, baseline.noise);
      }
      if (phase === "terminal") return state;
      if (single && !finished && leftBand && hasSettled(points, baseline, stepSeconds)) {
        await this.config.experiment.finish(plan.id);
        finished = true;
      }
      await this.reportHold(plan, stepSeconds);
      await this.pace();
    }
  }

  /** Keeps the output's good values at their process times and notes the newest sample time. */
  private observe(samples: RecordingFollowPage["rawSamples"]): TimedValue[] {
    const values: TimedValue[] = [];
    for (const sample of samples) {
      if (this.first === undefined) this.first = { wall: this.config.clock.now(), seconds: sample.elapsedSeconds };
      this.newestSeconds = Math.max(this.newestSeconds, sample.elapsedSeconds);
      if (sample.quality === "good" && typeof sample.pv === "number" && Number.isFinite(sample.pv)) {
        values.push({ seconds: sample.elapsedSeconds, value: sample.pv });
      }
    }
    return values;
  }

  /** A page with no new sample for the connection-loss timeout stops the campaign (A3.4, A7). */
  private async follow(): Promise<RecordingFollowPage> {
    const page = await this.config.recorder.follow();
    if (page.rawSamples.length > 0) this.lastSampleAt = this.config.clock.now();
    else if (this.config.clock.now() - this.lastSampleAt > this.silenceLimitMs()) {
      throw new Error("The runtime stopped sending samples.");
    }
    return page;
  }

  /**
   * The connection-loss timeout: 5 s, or three recorded sample intervals when those are longer (A3.4, A7). The
   * interval is taken in process time, which never runs slower than the wall clock, so the bound is safe at any time scale.
   */
  private silenceLimitMs(): number {
    const task = this.config.loop.controller.taskPeriodMs;
    return Math.max(CONNECTION_LOSS_TIMEOUT_MS, 3 * task * recordPeriodExecutions(campaignRecordingMs(this.config.loop), task));
  }

  private async pace(): Promise<void> {
    await this.config.clock.wait(POLL_MILLISECONDS);
  }

  private checkCancelled(): void {
    if (this.config.cancelled()) throw new Error("Identification cancelled.");
  }

  /** The observed time scale when it differs from 1 by more than 5 %, else undefined (A3.6). */
  private timeScale(): number | undefined {
    if (!this.first) return undefined;
    const scale = observedTimeScale(this.newestSeconds - this.first.seconds, this.config.clock.now() - this.first.wall);
    return scale !== undefined && Math.abs(scale - 1) > 0.05 ? scale : undefined;
  }

  private async reportBaseline(output: Output, rateWindow: readonly TimedValue[]): Promise<void> {
    if (this.newestSeconds - this.reportedSeconds < PROGRESS_INTERVAL_SECONDS) return;
    this.reportedSeconds = this.newestSeconds;
    await this.config.progress(`Waiting for ${output.symbol} to settle (changing ${formatRate(trendSlope(rateWindow), output.unit)})…`);
  }

  private async reportHold(plan: ControlExperimentPlan, stepSeconds: number): Promise<void> {
    const elapsed = Math.max(0, this.newestSeconds - stepSeconds);
    if (elapsed - this.reportedSeconds < PROGRESS_INTERVAL_SECONDS) return;
    this.reportedSeconds = elapsed;
    const scale = this.timeScale();
    const note = scale === undefined ? "" : ` (time scale ${scale.toFixed(1)}×)`;
    await this.config.progress(`Holding the step until the response settles: ${formatDuration(elapsed)} of at most ${formatDuration(plan.durationMs / 1_000)}${note}.`);
  }
}
