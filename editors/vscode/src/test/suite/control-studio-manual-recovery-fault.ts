import { pollValue } from "./plant-rendered-harness";

/** Drops one acknowledgement after the real commit, then makes status temporarily unreachable. */
export function installManualRecoveryFault() {
  const connections = require("../../controlStudio/runtimeConnection");
  const original = connections.bindRuntimeConnection;
  let blocked = false;
  let dropped = false;
  let writes = 0;
  connections.bindRuntimeConnection = async (...args: unknown[]) => {
    const bound = await original(...args);
    return { ...bound, request: async (operation: string, params: any) => {
      if (operation === "control.parameters.status" && blocked) throw new Error("Controlled temporary status disconnection");
      if (operation === "control.parameters.apply") writes++;
      const result = await bound.request(operation, params);
      if (operation === "control.parameters.apply" && !dropped) {
        dropped = true;
        await pollValue("real Manual gain commit", async () => JSON.stringify(await bound.request("control.parameters.status", { binding: params.binding })),
          value => JSON.parse(value).status === "applied", 5000);
        blocked = true;
        throw new Error("Controlled lost Manual application acknowledgement");
      }
      return result;
    } };
  };
  return { unblock: () => { blocked = false; }, writes: () => writes,
    restore: () => { blocked = false; connections.bindRuntimeConnection = original; } };
}
