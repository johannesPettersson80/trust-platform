import * as vscode from "vscode";

import { sendRuntimeControlRequest } from "../runtimeControlClient";
import type { RuntimeTarget } from "../runtimeTarget";
import { getBinaryPath } from "../binary";
import {
  createPlantSessionRegistry,
  PlantSessionRegistry,
  ProjectPlantTopologyService,
  removePlantCandidate,
  savePlantCandidate,
} from "./plant/service";
import { loadPlantTopology } from "./plant/projection";
import type { NCPlant } from "./webview/types";

const plantSessions = new WeakMap<vscode.ExtensionContext, PlantSessionRegistry>();

/**
 * The project's simulated plants for the canvas (specification 47): their saved configuration through the
 * project's command session and, while the runtime is reachable, their live status. Undefined when the plant
 * configuration is unavailable, so the ordinary canvas stays usable.
 */
export async function loadProjectPlantTopology(
  context: vscode.ExtensionContext,
  projectDir: string,
  runtime: RuntimeTarget
): Promise<NCPlant[] | undefined> {
  const stopped = plantSessionRegistry(context).forProject(projectDir);
  const liveRequest =
    runtime.status === "online_reachable" && runtime.endpoint
      ? async (operation: string, params: Record<string, unknown>) =>
          await sendRuntimeControlRequest(
            runtime.endpoint as string,
            runtime.authToken,
            operation,
            params,
            { timeoutMs: 2_000 }
          )
      : async (operation: string, params: Record<string, unknown>) =>
          await stopped.request(operation, params);
  try {
    return await loadPlantTopology(new ProjectPlantTopologyService(stopped, liveRequest));
  } catch (error) {
    // Keep the ordinary canvas usable, but do not make a missing plant
    // capability invisible during extension-host diagnosis.
    console.warn(
      "[truST] Simulated plant configuration is unavailable:",
      error instanceof Error ? error.message : String(error)
    );
    return undefined;
  }
}

function plantSessionRegistry(context: vscode.ExtensionContext): PlantSessionRegistry {
  const existing = plantSessions.get(context);
  if (existing) return existing;
  const registry = createPlantSessionRegistry(
    getBinaryPath(context, "trust-runtime", "runtime.cli.path")
  );
  plantSessions.set(context, registry);
  context.subscriptions.push(registry);
  return registry;
}

/** Saves through the same per-project session already used to populate the canvas. */
export async function saveProjectPlant(
  context: vscode.ExtensionContext,
  projectDir: string,
  candidate: Record<string, unknown>,
  operationId: string,
  expectedRevision?: string
) {
  return await savePlantCandidate(
    plantSessionRegistry(context).forProject(projectDir),
    candidate,
    operationId,
    expectedRevision
  );
}

/** Removes through the same per-project session already used to populate and save the canvas. */
export async function removeProjectPlant(
  context: vscode.ExtensionContext,
  projectDir: string,
  plantId: string,
  operationId: string,
  expectedRevision?: string
) {
  return await removePlantCandidate(
    plantSessionRegistry(context).forProject(projectDir),
    plantId,
    operationId,
    expectedRevision
  );
}

/** Reads plant configuration through the canvas' existing per-project command session. */
export async function readProjectPlants(
  context: vscode.ExtensionContext,
  projectDir: string
) {
  return await plantSessionRegistry(context).forProject(projectDir).request("plant.read", {
    contract: "trust.plant",
    schema_version: 1,
  });
}
