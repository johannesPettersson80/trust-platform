import {
  decodePlantContractResponse,
  type PlantContractMessage,
} from "./contract";
import { attachPlantsToRuntime } from "../graphData";
import { loadPlantTopology, type PlantTopologyService } from "./projection";
import { OfflinePlantCommandSession, type PlantCommandProcess } from "./offlineSession";
import { spawn } from "child_process";
import * as path from "path";
import type { NCGraph } from "../webview/types";

export interface PlantCommandClient {
  request(
    operation: string,
    params: Record<string, unknown>
  ): Promise<PlantContractMessage>;
}

export interface DisposablePlantCommandClient extends PlantCommandClient {
  dispose(): void;
}

export function plantSaveResultLabel(
  result: PlantContractMessage
): "Saved" | "Saved · restart required" {
  return "restart_required" in result && result.restart_required === true
    ? "Saved · restart required"
    : "Saved";
}

/** Uses the established project command session for the complete read/validate/save transaction. */
export async function savePlantCandidate(
  client: PlantCommandClient,
  plant: Record<string, unknown>,
  operationId: string,
  expectedRevision?: string
): Promise<PlantContractMessage> {
  const read = await client.request("plant.read", {
    contract: "trust.plant",
    schema_version: 1,
  });
  const common = {
    contract: "trust.plant",
    schema_version: 1,
    project_id: read.project_id,
    plant,
  };
  const validation = await client.request("plant.validate", common);
  if (validation.operation !== "plant.validate" || validation.valid !== true) {
    throw new Error("The simulated plant configuration is not valid.");
  }
  const saved = await client.request("plant.save", {
    ...common,
    operation_id: operationId,
    // Validation uses the latest document, but an existing drawer remains
    // bound to the revision it opened so newer project edits are not lost.
    expected_revision: expectedRevision ?? read.configuration_revision,
  });
  if (saved.operation !== "plant.save") {
    throw new Error("The simulated plant was not saved.");
  }
  return saved;
}

/** Removes through the same established project command session used by reads and saves. */
export async function removePlantCandidate(
  client: PlantCommandClient,
  plantId: string,
  operationId: string,
  expectedRevision?: string
): Promise<PlantContractMessage> {
  const read = await client.request("plant.read", {
    contract: "trust.plant",
    schema_version: 1,
  });
  const removed = await client.request("plant.remove", {
    contract: "trust.plant",
    schema_version: 1,
    project_id: read.project_id,
    operation_id: operationId,
    expected_revision: expectedRevision ?? read.configuration_revision,
    plant_id: plantId,
  });
  if (removed.operation !== "plant.remove") {
    throw new Error("The simulated plant was not removed.");
  }
  return removed;
}

export class PlantSessionRegistry {
  private readonly sessions = new Map<string, DisposablePlantCommandClient>();

  constructor(
    private readonly factory: (projectRoot: string) => DisposablePlantCommandClient
  ) {}

  forProject(projectRoot: string): DisposablePlantCommandClient {
    const resolved = path.resolve(projectRoot);
    const existing = this.sessions.get(resolved);
    if (existing) return existing;
    const created = this.factory(resolved);
    this.sessions.set(resolved, created);
    return created;
  }

  dispose(): void {
    for (const session of this.sessions.values()) session.dispose();
    this.sessions.clear();
  }
}

export type PlantProcessSpawner = (
  binary: string,
  args: string[],
  options: { cwd: string; stdio: ["pipe", "pipe", "pipe"] }
) => PlantCommandProcess;

export function createPlantSessionRegistry(
  runtimeBinary: string,
  spawner: PlantProcessSpawner = spawn as unknown as PlantProcessSpawner
): PlantSessionRegistry {
  return new PlantSessionRegistry((projectRoot) =>
    new OfflinePlantCommandSession(
      spawner(runtimeBinary, ["plant", "serve", "--project", projectRoot], {
        cwd: projectRoot,
        stdio: ["pipe", "pipe", "pipe"],
      })
    )
  );
}

export async function populatePlantTopology(
  graph: NCGraph,
  runtimeId: string,
  service: PlantTopologyService
): Promise<void> {
  attachPlantsToRuntime(graph, runtimeId, await loadPlantTopology(service));
}

export type LivePlantRequest = (
  operation: string,
  params: Record<string, unknown>
) => Promise<unknown>;

/** Keeps transport details on the extension-host side of the typed plant port. */
export class ProjectPlantTopologyService implements PlantTopologyService {
  constructor(
    private readonly stopped: PlantCommandClient,
    private readonly live: LivePlantRequest
  ) {}

  async read(): Promise<PlantContractMessage> {
    return await this.stopped.request("plant.read", {
      contract: "trust.plant",
      schema_version: 1,
    });
  }

  async status(projectId: string): Promise<PlantContractMessage> {
    const value = await this.live("plant.status", {
      contract: "trust.plant",
      schema_version: 1,
      project_id: projectId,
    });
    const decoded = decodePlantContractResponse(value, projectId);
    if (decoded.kind !== "accepted") {
      throw new Error(`plant status unavailable: ${decoded.reason}`);
    }
    if (decoded.message.operation !== "plant.status") {
      throw new Error("plant status unavailable: unexpected operation");
    }
    return decoded.message;
  }
}
