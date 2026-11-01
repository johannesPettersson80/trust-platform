// The run target the HMI Builder talks to (specification 39, §11 item 6, §16.2 item 8), as the language server
// answers it (`trust-lsp.runTargets`, action `hmiTarget`): the simulator or runtime the extension has started or
// attached to, or the configured online runtime when it answers.

import { projectRequest } from "../../projectRequest";
import type { RuntimeLifecycleSnapshot } from "../../runtimeLifecycle";
import { toModelSnapshot } from "../../trustHomeModel";

export interface RunTarget {
  endpoint: string;
  label: string;
  simulator: boolean;
}

/** The selected run target of `snapshot`, or none when nothing runs or answers. Throws the language server's reason
 * when it gives no answer; `wait` waits a bounded time for a server that is still starting. */
export async function resolveHmiRunTarget(
  snapshot: RuntimeLifecycleSnapshot,
  options: { readonly wait?: boolean } = {},
): Promise<RunTarget | undefined> {
  const answer = await projectRequest<{ target: RunTarget | null }>(
    "trust-lsp.runTargets",
    { action: "hmiTarget", snapshot: toModelSnapshot(snapshot) },
    options,
  );
  if (!answer.ok) throw new Error(answer.error);
  return answer.target ?? undefined;
}
