// The `connectors.status` request of a runtime (specification 11, "Connector status and discovery
// truth contract"). The canvas sends each runtime's answer with its `fleet.topology` report to
// `trust-lsp.devicesTopology`, which attaches the reports to their endpoints and checks their
// vocabularies (specification 25 §5.6); nothing here matches or judges a report.
import type { RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";

export async function fetchConnectorStatus(
  runtime: RuntimeTarget,
  timeoutMs = 2000
): Promise<unknown> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return undefined;
  }
  return await sendRuntimeControlRequest<unknown>(
    runtime.endpoint,
    runtime.authToken,
    "connectors.status",
    undefined,
    { timeoutMs }
  );
}
