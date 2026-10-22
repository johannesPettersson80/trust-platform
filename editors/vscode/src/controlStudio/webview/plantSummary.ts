import type { PlantMonitorPayload } from "../messages";

/**
 * The plant summary of a plant handoff (specification 35, B1.1). The panel asks the engine for it (`controlStudio/plantMonitor`) and
 * posts the answer as `plant.monitor.snapshot`; this shows the answer in the summary line under the target label. The engine decides
 * whether the plant is available and why it is not, so the page only words the line.
 */
export function bindPlantSummary(root: HTMLElement): void {
  const line = root.querySelector<HTMLElement>("[data-studio-plant-summary]");
  const view = root.ownerDocument.defaultView;
  if (!line || !view) return;
  view.addEventListener("message", event => {
    const result = plantSnapshotOf(event.data, root);
    if (!result) return;
    line.textContent = plantSummaryText(result);
    line.hidden = false;
  });
}

/** The summary of a `plant.monitor.snapshot` message that belongs to this page's project, editor session and plant revision. */
function plantSnapshotOf(message: unknown, root: HTMLElement): PlantMonitorPayload | undefined {
  if (!record(message) || message.schemaVersion !== 1 || message.kind !== "plant.monitor.snapshot") return undefined;
  if (message.projectId !== root.dataset.projectId || message.editorSessionId !== root.dataset.editorSessionId
    || message.expectedPlantRevision !== root.dataset.plantRevision) return undefined;
  return record(message.payload) ? message.payload as PlantMonitorPayload : undefined;
}

/**
 * One line for the summary: the plant and the source the engine gave, then its model and operating state, or the engine's reason. A
 * summary that is a failure's sentence names no source, and the line names none.
 */
export function plantSummaryText(result: PlantMonitorPayload): string {
  const plant = "source" in result ? `${result.plantId} · ${result.source}` : result.plantId;
  if ("source" in result && result.availability === "available" && result.status) {
    const status = result.status;
    return `${plant} · ${status.model} · ${status.operatingState}${status.currentFault ? ` (${status.currentFault})` : ""}`;
  }
  return `${plant} · unavailable${result.reason ? `: ${result.reason}` : ""}`;
}

function record(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
