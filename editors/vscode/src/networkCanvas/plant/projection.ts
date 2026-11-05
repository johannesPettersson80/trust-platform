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

/** Rust's projections of one load: `plants` for the canvas, and `saved`, the first projection, of the
 * saved configuration alone, whose plants show Status unavailable and carry no live data. */
export interface PlantTopology {
  plants: NCPlant[];
  saved: NCPlant[];
}

/**
 * The project's plants as the canvas shows them (specification 47 §20.6): Rust's `plant.topology`
 * names the project and projects its saved plants; when the owning runtime answers `plant.status`,
 * Rust projects them again with that answer.
 */
export async function loadPlantTopology(service: PlantTopologyService): Promise<PlantTopology> {
  const saved = await service.topology(null);
  const savedPlants = saved.plants as NCPlant[];
  // A refusal carries the runtime's answer (a stopped project's says that no runtime runs the plants); only no answer at
  // all leaves the first projection, and so does a second projection that fails.
  const status = await service
    .status(saved.project_id)
    .catch((error: unknown) => (error instanceof RuntimeControlError && error.result !== undefined ? error.result : null));
  if (status == null) {
    return { plants: savedPlants, saved: savedPlants };
  }
  const plants = await service.topology(status).then(
    (topology) => topology.plants as NCPlant[],
    () => savedPlants
  );
  return { plants, saved: savedPlants };
}

/**
 * Each project's last confirmed saved configuration, as Rust's first projection of its last load that
 * succeeded (specification 47 §20.6): a load that fails leaves it on the canvas, with Status
 * unavailable and no live data, instead of dropping every plant.
 */
export class ConfirmedPlants {
  private readonly saved = new Map<string, NCPlant[]>();

  /** The plants for the canvas: this load's, or after a failure the project's last confirmed ones
   * (undefined when it has none). */
  async load(
    projectDir: string,
    service: PlantTopologyService,
    onFailure: (error: unknown) => void
  ): Promise<NCPlant[] | undefined> {
    try {
      const topology = await loadPlantTopology(service);
      this.saved.set(projectDir, topology.saved);
      return topology.plants;
    } catch (error) {
      onFailure(error);
      return this.saved.get(projectDir);
    }
  }
}
