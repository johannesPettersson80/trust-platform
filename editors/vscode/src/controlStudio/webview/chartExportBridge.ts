export interface ChartExportResultEvent {
  requestId: string;
  result: { ok: boolean; filename?: string; path?: string; reason?: string };
}

type ExportScope = { projectId?: string; editorSessionId?: string; loopId?: string; loopRevision?: string; plantId?: string; plantRevision?: string };

/** Uses DOM events because chart and workflow code are separate browser bundles. */
export function initializeChartExportBridge(post: (kind: string, payload: Record<string, unknown>) => void, owner: Document = document): void {
  const win = owner.defaultView!;
  const root = owner.querySelector<HTMLElement>("[data-control-studio]");
  const pending = new Map<string, ExportScope>();
  const status = owner.createElement("span");
  status.dataset.chartExportStatus = ""; status.setAttribute("role", "status"); status.hidden = true;
  status.style.cssText = "font-size:11px;overflow-wrap:anywhere";
  owner.querySelector(".plot-toolbar")?.append(status);
  const scope = (): ExportScope => ({ projectId: root?.dataset.projectId, editorSessionId: root?.dataset.editorSessionId,
    loopId: root?.dataset.loopId, loopRevision: root?.dataset.loopRevision,
    plantId: root?.dataset.plantId, plantRevision: root?.dataset.plantRevision });
  const show = (text: string) => { status.hidden = false; status.textContent = text; };
  win.addEventListener("trust-control-studio-export-request", event => {
    const detail = (event as CustomEvent<{ url?: string; filename?: string; mimeType?: string; requestId?: string }>).detail;
    if (!detail?.url || !detail.filename || !detail.mimeType || !detail.requestId || pending.has(detail.requestId)) return;
    event.preventDefault();
    pending.set(detail.requestId, scope());
    show("Choose where to save…");
    post("control-studio.chart.export", { requestId: detail.requestId, data: detail.url, filename: detail.filename, mimeType: detail.mimeType });
  });
  win.addEventListener("message", event => {
    const message = event.data;
    if (message?.kind !== "control-studio.chart.export.result" || message.schemaVersion !== 1 || typeof message.requestId !== "string") return;
    const expected = pending.get(message.requestId);
    if (!expected || message.projectId !== expected.projectId || message.editorSessionId !== expected.editorSessionId ||
      (expected.loopId ? message.loopId !== expected.loopId || message.expectedLoopRevision !== expected.loopRevision
        : message.plantId !== expected.plantId || message.expectedPlantRevision !== expected.plantRevision)) return;
    pending.delete(message.requestId);
    const result = message.result ?? { ok: false, reason: "Export failed." };
    show(result.ok ? `Saved ${result.filename ?? "file"}` : String(result.reason ?? "Export failed."));
    win.dispatchEvent(new win.CustomEvent<ChartExportResultEvent>("trust-control-studio-export-result", { detail: { requestId: message.requestId, result } }));
  });
  win.addEventListener("pagehide", () => pending.clear());
}
