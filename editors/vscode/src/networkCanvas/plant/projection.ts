import type { NCPlant } from "../webview/types";
import type { PlantContractMessage } from "./contract";

export interface PlantTopologyService {
  read(): Promise<PlantContractMessage>;
  status(projectId: string): Promise<PlantContractMessage>;
  topology(projectId: string, status: PlantContractMessage | null): Promise<PlantContractMessage>;
}

/**
 * The project's plants as the canvas shows them (specification 47 §20.6): `plant.read` names the
 * project, the owning runtime answers its status when it can, and Rust's `plant.topology` projects
 * the saved plants with that status.
 */
export async function loadPlantTopology(service: PlantTopologyService): Promise<NCPlant[]> {
  const saved = await service.read();
  // A status the runtime could not give goes to Rust as none, which marks the plants unavailable.
  const status = await service.status(saved.project_id).catch(() => null);
  const topology = await service.topology(saved.project_id, status);
  return topology.plants as NCPlant[];
}
