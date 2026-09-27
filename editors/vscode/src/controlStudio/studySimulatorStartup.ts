interface SimulatorLifecycle {
  snapshot(): Promise<{ status: { running: boolean; runtimeMode: string } }>;
  startLocalSimulator(): Promise<{ ok: boolean; failure?: { message?: string } }>;
}

/** One shared simulator start; each waiting study retains its own cancellation and target checks. */
export class StudySimulatorStartup {
  private starting: Promise<void> | undefined;
  constructor(private readonly lifecycle: SimulatorLifecycle, private readonly selected: () => string | undefined) {}

  async ensure(runtimeId: string, cancelled: () => boolean): Promise<void> {
    const current = () => {
      if (cancelled()) throw new Error("Study cancelled before simulator readiness.");
      if (this.selected() !== runtimeId) throw new Error("The selected target changed before simulator readiness.");
    };
    current();
    const snapshot = await this.lifecycle.snapshot();
    current();
    if (snapshot.status.runtimeMode !== "simulate") throw new Error("Select the local simulator before starting an isolated study.");
    if (!snapshot.status.running) {
      if (!this.starting) {
        const start = this.lifecycle.startLocalSimulator().then(result => {
          if (!result.ok) throw new Error(result.failure?.message ?? "Simulator startup failed.");
        });
        const pending = start.finally(() => { if (this.starting === pending) this.starting = undefined; });
        this.starting = pending;
      }
      await this.starting;
    }
    current();
  }
}
