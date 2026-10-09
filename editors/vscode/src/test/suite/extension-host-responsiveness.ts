import * as fs from "fs";
import * as inspector from "inspector";
import { monitorEventLoopDelay, type IntervalHistogram } from "perf_hooks";

/** Specification 35, A10.7: Control Studio never holds the extension host for more than this at a time. */
export const LONGEST_EXTENSION_HOST_BLOCK_MS = 1000;
/** The event-loop delay is sampled this often, so a block is measured to within this many milliseconds. */
const RESOLUTION_MS = 20;
/** The CPU profile samples the extension host's thread this often, in microseconds. */
const PROFILE_INTERVAL_US = 5000;

/**
 * Measures how long the extension host's thread is held, from a journey that runs inside it: the longest event-loop delay
 * since the last phase mark and over the whole probe. A CPU profile of the same span names what held it.
 */
export class ExtensionHostResponsiveness {
  private readonly overall: IntervalHistogram = monitorEventLoopDelay({ resolution: RESOLUTION_MS });
  private readonly phase: IntervalHistogram = monitorEventLoopDelay({ resolution: RESOLUTION_MS });
  private profiler?: inspector.Session;
  private profileFailure?: string;

  async start(): Promise<void> {
    this.overall.enable();
    this.phase.enable();
    try {
      const session = new inspector.Session();
      session.connect();
      this.profiler = session;
      await this.post("Profiler.enable");
      await this.post("Profiler.setSamplingInterval", { interval: PROFILE_INTERVAL_US });
      await this.post("Profiler.start");
    } catch (error) {
      this.profileFailure = error instanceof Error ? error.message : String(error);
    }
  }

  /** Starts a new phase: phaseMaxMs then covers only what follows. */
  markPhase(): void {
    this.phase.reset();
  }

  /** The longest block since the last phase mark, in milliseconds. */
  phaseMaxMs(): number {
    return blockMs(this.phase);
  }

  /** The longest block since the probe started, in milliseconds. */
  overallMaxMs(): number {
    return blockMs(this.overall);
  }

  /** Stops measuring and writes the CPU profile; returns why no profile was written, if none was. */
  async stop(profilePath: string): Promise<string | undefined> {
    this.overall.disable();
    this.phase.disable();
    if (!this.profiler) return this.profileFailure ?? "the profiler did not start";
    try {
      const result = await this.post("Profiler.stop") as { profile: unknown };
      fs.writeFileSync(profilePath, JSON.stringify(result.profile));
      return undefined;
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    } finally {
      this.profiler.disconnect();
      this.profiler = undefined;
    }
  }

  private post(method: string, params: object = {}): Promise<unknown> {
    return new Promise((resolve, reject) => {
      this.profiler!.post(method, params, (error, result) => (error ? reject(error) : resolve(result)));
    });
  }
}

/** A histogram's longest delay, less the sampling interval it waits anyway. */
function blockMs(histogram: IntervalHistogram): number {
  return Math.max(0, histogram.max / 1e6 - RESOLUTION_MS);
}
