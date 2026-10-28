import type { ChartExportAnswer } from "./generated/ChartExportAnswer";
import type { ChartExportRequest } from "./generated/ChartExportRequest";
import type { EngineAnswer } from "./engine/engineClient";

export interface ChartExportHost {
  /** The save dialog takes the name, the filter and the extension the engine names; the host shows them. */
  showSaveDialog(options: { defaultFileName: string; filterLabel: string; extension: string }): Promise<{ fsPath: string } | undefined>;
  writeFile(uri: { fsPath: string }, bytes: Uint8Array): Promise<void>;
}

export interface ChartExportScope {
  projectId: string;
  editorSessionId: string;
  loopId?: string;
  loopRevision?: string;
  plantId?: string;
  plantRevision?: string;
}

/** The engine's side of an export: its answer for the page's message (B10.5). */
export interface ChartExportEngine {
  chartExport(request: ChartExportRequest): Promise<EngineAnswer<ChartExportAnswer>>;
}

export type ChartExportResult =
  | { readonly ok: true; readonly filename: string; readonly path: string }
  | { readonly ok: false; readonly reason: string };

/**
 * Saves a chart export (B10.5). The engine checks the page's message against the session the panel shows, decodes its data, and
 * names the file; the host shows the save dialog with the engine's name and filter, and writes the bytes the engine produced.
 */
export async function saveControlStudioChartExport(
  engine: ChartExportEngine,
  message: unknown,
  scope: ChartExportScope,
  host: ChartExportHost,
): Promise<ChartExportResult> {
  try {
    const answer = await engine.chartExport({ message: message as ChartExportRequest["message"], session: scope });
    if ("failure" in answer) return { ok: false, reason: answer.failure.detail || answer.failure.message };
    const file = answer.result;
    const uri = await host.showSaveDialog({ defaultFileName: file.fileName, filterLabel: file.filterLabel, extension: file.extension });
    if (!uri) return { ok: false, reason: "Export cancelled." };
    await host.writeFile(uri, Buffer.from(file.bytesBase64, "base64"));
    const uriToString = (uri as { toString?: unknown }).toString;
    return { ok: true, filename: uri.fsPath.split(/[\\/]/).pop() || file.fileName, path: typeof uriToString === "function" && uriToString !== Object.prototype.toString
      ? (uri as { toString(): string }).toString() : uri.fsPath };
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
}
