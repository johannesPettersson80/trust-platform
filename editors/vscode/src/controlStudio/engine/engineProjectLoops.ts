import type { ProjectLoopsRequest } from "../generated/ProjectLoopsRequest";
import type { ProjectLoopsResult } from "../generated/ProjectLoopsResult";
import { engineAnswer } from "../loopSetup/setupEngine";
import type { ControlStudioEngine, EngineAnswer } from "./engineClient";

/*
 * The loop files of the workspace folders, as the engine decides them (`controlStudio/projectLoops`; specification 35, A8.3, B1.2 and
 * B1.3). The extension forwards the request and shows the answer: it validates no loop file and decides no folder itself.
 */

/** The workspace folders' loops: each file's decision, the folder that declares a loop, and an equipment's loops (A8.3, B1.2). */
export function projectLoops(engine: Pick<ControlStudioEngine, "projectLoops">, request: ProjectLoopsRequest): Promise<EngineAnswer<ProjectLoopsResult>> {
  return engineAnswer(() => engine.projectLoops(request));
}
