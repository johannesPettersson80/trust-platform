import { RuntimeControlError } from "../../runtimeControlClient";
import type { NCPlant } from "../webview/types";
import type { PlantContractMessage } from "./contract";

export interface PlantTopologyService {
  /** Rust's projection of the saved plants (`plant.topology`), with the owning runtime's
   * `plant.status` answer as it came, or with none. */
  topology(status: unknown): Promise<PlantContractMessage>;
  /** The owning runtime's `plant.status` answer, as it came: Rust decides whether it is current. */
  status(projectId: string): Promise<unknown>;
}

/**
 * The project's plants as the canvas shows them (specification 47 §20.6): Rust's `plant.topology`
 * names the project and projects its saved plants; when the owning runtime answers `plant.status`,
 * Rust projects them again with that answer.
 */
export async function loadPlantTopology(service: PlantTopologyService): Promise<NCPlant[]> {
  const saved = await service.topology(null);
  // A refusal carries the runtime's answer (a stopped project's says that no runtime runs the plants); only no answer at
  // all leaves the first projection.
  const status = await service
    .status(saved.project_id)
    .catch((error: unknown) => (error instanceof RuntimeControlError && error.result !== undefined ? error.result : null));
  const topology = status == null ? saved : await service.topology(status);
  return topology.plants as NCPlant[];
}
