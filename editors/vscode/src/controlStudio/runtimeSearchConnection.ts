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
  if (connection.source !== "simulation") throw new Error("Use the local simulator for isolated ST search.");
  return connection;
}
