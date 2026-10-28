// Live mode on the editor's host (specification 39, §11, item 6): reads the page's bound paths
// from the selected run target with the control request `hmi.read` every 250 ms and passes the
// values and the state of the connection to the webview. A value is never sent the other way.

import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import type { HostToWebview, LiveState, LiveUpdate } from "./messages";
import { runTargetOf } from "./runTarget";

/** How often Live mode reads, as often as the runtime display's stream (§11.1). */
export const LIVE_PERIOD_MS = 250;

/** The run target Live mode reads from: its control endpoint, and a request to it. */
export interface LiveConnection {
  target(): Promise<{ endpoint: string; label: string; token?: string } | undefined>;
  request(endpoint: string, token: string | undefined, operation: string, params: unknown): Promise<unknown>;
}

/** The run target the extension has selected and started: the simulator or a runtime. */
export const runtimeLiveConnection: LiveConnection = {
  async target() {
    const target = runTargetOf((await runtimeLifecycleService.snapshot()).status);
    return target && { ...target, token: await runtimeLifecycleService.controlAuthToken(target.endpoint) };
  },
  request: (endpoint, token, operation, params) =>
    sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 2_000 }),
};

/** What a poller tells the webview: Live mode's state and values, or the current values of the
 * variables the pickers show (§3.3 item 8), which leave Live mode's state alone. */
export type PollerOutput = "live" | "picker";

export class LivePoller {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private paths: string[] = [];
  private on = false;
  private reported: string | undefined;

  constructor(
    private readonly connection: LiveConnection,
    private readonly post: (message: HostToWebview) => void,
    private readonly output: PollerOutput = "live",
  ) {}

  /** Live mode on with the page's bound paths, or off. */
  set(on: boolean, paths: string[] = []): void {
    this.paths = paths;
    if (!on) {
      this.stop();
      return;
    }
    if (this.on) return;
    this.on = true;
    this.reported = undefined;
    void this.tick();
  }

  stop(): void {
    this.on = false;
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
  }

  private async tick(): Promise<void> {
    if (!this.on) return;
    const started = Date.now();
    try {
      const target = await this.connection.target();
      if (!target) {
        this.state("none");
      } else {
        try {
          const update = (await this.connection.request(target.endpoint, target.token, "hmi.read", {
            paths: this.paths,
          })) as LiveUpdate;
          if (!this.on) return;
          this.state("connected", target.label);
          this.post(this.output === "live" ? { type: "liveValues", update } : { type: "pickerValues", update });
        } catch (error) {
          this.state("lost", target.label, error instanceof Error ? error.message : String(error));
        }
      }
    } catch (error) {
      this.state("lost", undefined, error instanceof Error ? error.message : String(error));
    } finally {
      if (this.on)
        this.timer = setTimeout(() => void this.tick(), Math.max(0, LIVE_PERIOD_MS - (Date.now() - started)));
    }
  }

  /** Reports the connection's state when it changes; the pickers only learn that values are gone. */
  private state(state: LiveState, target?: string, reason?: string): void {
    if (this.output === "picker") {
      if (state !== "connected" && this.reported !== state && this.on) this.post({ type: "pickerValues" });
      this.reported = state;
      return;
    }
    const message: HostToWebview = {
      type: "live",
      state,
      ...(target ? { target } : {}),
      ...(reason ? { reason } : {}),
    };
    const key = JSON.stringify(message);
    if (key === this.reported || !this.on) return;
    this.reported = key;
    this.post(message);
  }
}
