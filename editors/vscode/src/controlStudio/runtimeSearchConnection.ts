import { runtimeLifecycleService } from "../runtimeLifecycle";
import { getSelectedRuntimeId } from "../selectedRuntime";
import { StudySimulatorStartup } from "./studySimulatorStartup";
import type { ControlLoopDefinition } from "./loopDefinition";
import { bindRuntimeConnection } from "./runtimeConnection";
import type { SearchConnection } from "./finiteSearchSession";

const simulatorStartup = new StudySimulatorStartup(runtimeLifecycleService, getSelectedRuntimeId);

export async function connectRuntimeSearch(loop: ControlLoopDefinition, startup?: { cancelled: () => boolean }): Promise<SearchConnection> {
  if (startup) await simulatorStartup.ensure(loop.runtimeId, startup.cancelled);
  const connection = await bindRuntimeConnection(loop.runtimeId);
  // Isolated simulation never runs on a physical target's host (A2.6), so an approval cannot make this connection usable.
  if (connection.source !== "simulation") throw new Error("Isolated ST search needs the local simulator or a simulated runtime; it never runs on a physical target's host (A2.6).");
  return connection;
}
