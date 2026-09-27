import type { ControlLoopDefinition } from "./loopDefinition";
import type { CommissioningResult } from "./proportionalCommissioning";
import { connectRuntimeParameters } from "./runtimeParameterConnection";
import { sameParameterHistory } from "./parameterApplication";

/** Reopening inspects the existing runtime. A saved receipt cannot authorize a replay. */
export async function observeStudioApplication(loop: ControlLoopDefinition,
  receipt: CommissioningResult | undefined): Promise<Record<string, unknown> | undefined> {
  if (!receipt || receipt.status !== "passed" || !receipt.recoveryConfirmed || !receipt.parameters) return undefined;
  const saved = receipt.parameters;
  try {
    const current = await (await connectRuntimeParameters(loop)).client.status();
    const matches = current.status === "applied" && current.transaction_id === saved.transaction_id
      && Boolean(saved.previous_revision && saved.previous_values) && sameParameterHistory(current, saved);
    return { status: "application-observed", applicationAvailability: matches ? "restore" : "unavailable",
      ...(matches ? {} : { reason: "The active controller settings changed. Tune and validate again before applying." }) };
  } catch {
    return { status: "application-observed", applicationAvailability: "unavailable",
      reason: "Reconnect the controller to review its previous settings." };
  }
}
