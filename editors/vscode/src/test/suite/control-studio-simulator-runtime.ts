import { runtimeLifecycleService } from "../../runtimeLifecycle";
import { sendRuntimeControlRequest } from "../../runtimeControlClient";
import { getSelectedRuntimeId } from "../../selectedRuntime";

/** The fields of a loop that the control requests below read: its runtime and revision, its two signals' addresses and its ST parameter binding. */
export interface SimulatorLoop {
  readonly runtimeId: string;
  readonly revision: string;
  readonly parameterBinding?: unknown;
  readonly signals: {
    readonly processVariable: { readonly address?: string | null; readonly iecType: string };
    readonly manipulatedVariable: { readonly address?: string | null; readonly iecType: string };
  };
}

/** One coherent reading of the loop's measurement and of the output the plant receives, taken from the runtime's own scan. */
export interface ProcessReading {
  readonly pv: number;
  readonly mv: number;
}

/**
 * A control request to the started local simulator. The journeys read the runtime directly to check what the engine and the page
 * claim, so this sends the request as the deleted runtime connection of the extension sent it (`bindRuntimeConnection` at HEAD): the
 * selected runtime, then the lifecycle's endpoint and its control token, with the same five-second timeout. Start the simulator first.
 */
export async function simulatorRequest(runtimeId: string, operation: string, params: Record<string, unknown>): Promise<any> {
  if (getSelectedRuntimeId() !== runtimeId) throw new Error("The selected runtime does not match this loop.");
  const lifecycle = await runtimeLifecycleService.snapshot();
  if (!lifecycle.status.running) throw new Error("Start the bound runtime before reviewing this operation.");
  const endpoint = lifecycle.status.endpoint;
  const token = await runtimeLifecycleService.controlAuthToken(endpoint);
  return sendRuntimeControlRequest(endpoint, token, operation, params, { timeoutMs: 5000 });
}

/**
 * The runtime's answer to `control.parameters.status` for the loop's ST parameter binding: the values the controller holds, the
 * transaction state and the revisions (runtime protocol annex, E7). It is the runtime's own record, not the page's.
 */
export async function parameterStatus(loop: SimulatorLoop): Promise<any> {
  if (!loop.parameterBinding) throw new Error("Declare the ST parameter binding before application.");
  return simulatorRequest(loop.runtimeId, "control.parameters.status", { binding: loop.parameterBinding });
}

/**
 * The loop's measurement and output in the simulator's last coherent scan (`control.signals.inspect`), or undefined while the
 * runtime holds none. The request names the signals by their declared addresses.
 *
 * Read the runtime only while no engine operation runs: the runtime keeps one inspection watch for every reader of a loop, and a
 * request with another selection replaces it (crates/trust-runtime/src/host/debug/control/signals.rs). The first request after a
 * replacement answers that no coherent scan is held yet, so a reader that polls at the engine's pace starves the engine's own sampler,
 * recording start and monitor. A journey reads the plant at rest before a test and after it, and takes the rest from the kept recording.
 */
export async function processReading(loop: SimulatorLoop): Promise<ProcessReading | undefined> {
  const { processVariable, manipulatedVariable } = loop.signals;
  if (!processVariable.address || !manipulatedVariable.address) throw new Error("The loop declares no addresses for its measurement and output.");
  const answer = await inspectOrNothing(loop.runtimeId, {
    loop_revision: loop.revision,
    bindings: [
      { id: "pv", address: processVariable.address, iec_type: processVariable.iecType },
      { id: "mv", address: manipulatedVariable.address, iec_type: manipulatedVariable.iecType },
    ],
  });
  if (!answer || answer.available !== true || !Array.isArray(answer.values)) return undefined;
  const [pv, mv] = answer.values;
  return typeof pv === "number" && typeof mv === "number" ? { pv, mv } : undefined;
}

/** `control.signals.inspect`, or undefined while the runtime holds no scan: just after the simulator starts that is a reading not there yet, not a failure. */
async function inspectOrNothing(runtimeId: string, params: Record<string, unknown>): Promise<any> {
  try {
    return await simulatorRequest(runtimeId, "control.signals.inspect", params);
  } catch (error) {
    if (error instanceof Error && /current runtime scan unavailable/i.test(error.message)) return undefined;
    throw error;
  }
}
