import { Worker } from "worker_threads";
/** Owns one cancellable numerical worker. Sessions retain their own scope/revision guards. */
export class BoundedAnalysisJob {
  private cancelJob: (() => void) | undefined;
  run<T>(modulePath: string, workerData: unknown): Promise<T | undefined> {
    this.cancel();
    return new Promise((resolve, reject) => {
      const worker = new Worker(modulePath, { workerData, resourceLimits: { maxOldGenerationSizeMb: 128 } });
      let settled = false;
      const finish = (value?: T, error?: Error) => {
        if (settled) return;
        settled = true; clearTimeout(timer); this.cancelJob = undefined; void worker.terminate();
        if (error) reject(error); else resolve(value);
      };
      const timer = setTimeout(() => finish(undefined, new Error("Analysis exceeded the 10 second budget.")), 10000);
      this.cancelJob = () => finish();
      worker.once("message", value => finish(value));
      worker.once("error", error => finish(undefined, error instanceof Error ? error : new Error(String(error))));
      worker.once("exit", code => { if (!settled) finish(undefined, new Error(`Analysis worker exited without a result (${code}).`)); });
    });
  }
  cancel(): void { this.cancelJob?.(); this.cancelJob = undefined; }
}
