export interface ChartExportHost {
  showSaveDialog(options: { defaultFileName: string; mimeType: string }): Promise<{ fsPath: string } | undefined>;
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

export type ChartExportResult =
  | { readonly ok: true; readonly filename: string; readonly path: string }
  | { readonly ok: false; readonly reason: string };

export async function saveControlStudioChartExport(
  message: unknown,
  scope: ChartExportScope,
  host: ChartExportHost,
): Promise<ChartExportResult> {
  const request = decodeRequest(message, scope);
  if (!request.ok) return request;
  const parsed = decodeDataUrl(request.payload.data);
  if (!parsed.ok) return parsed;
  if (parsed.mimeType !== request.payload.mimeType) return { ok: false, reason: "Export MIME type does not match its data." };
  const filename = exportFilename(request.payload.filename, parsed.mimeType);
  if (!filename) return { ok: false, reason: "Choose a filename with the correct export extension." };
  if (parsed.bytes.byteLength > 16 * 1024 * 1024) return { ok: false, reason: "The chart export is larger than 16 MiB." };
  try {
    const uri = await host.showSaveDialog({ defaultFileName: filename, mimeType: parsed.mimeType });
    if (!uri) return { ok: false, reason: "Export cancelled." };
    await host.writeFile(uri, parsed.bytes);
    const uriToString = (uri as { toString?: unknown }).toString;
    return { ok: true, filename: uri.fsPath.split(/[\\/]/).pop() || filename, path: typeof uriToString === "function" && uriToString !== Object.prototype.toString
      ? (uri as { toString(): string }).toString() : uri.fsPath };
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
}

function decodeRequest(value: unknown, scope: ChartExportScope):
  { ok: true; requestId: string; payload: { data: string; mimeType: string; filename?: string } } | { ok: false; reason: string } {
  if (!value || typeof value !== "object") return { ok: false, reason: "Invalid chart export message." };
  const request = value as Record<string, unknown>;
  const payload = request.payload as Record<string, unknown> | undefined;
  const loopScope = Boolean(scope.loopId && scope.loopRevision);
  const scopeMatches = loopScope
    ? request.loopId === scope.loopId && request.expectedLoopRevision === scope.loopRevision
    : Boolean(scope.plantId && scope.plantRevision) && request.plantId === scope.plantId && request.expectedPlantRevision === scope.plantRevision;
  if (request.schemaVersion !== 1 || request.kind !== "control-studio.chart.export" || typeof payload?.requestId !== "string" || !payload.requestId ||
      request.projectId !== scope.projectId || request.editorSessionId !== scope.editorSessionId || !scopeMatches ||
      !payload || typeof payload.data !== "string" || typeof payload.mimeType !== "string" ||
      (payload.filename !== undefined && typeof payload.filename !== "string")) {
    return { ok: false, reason: "The chart export session is stale or invalid." };
  }
  return { ok: true, requestId: payload.requestId as string, payload: payload as { data: string; mimeType: string; filename?: string } };
}

function decodeDataUrl(value: string): { ok: true; mimeType: string; bytes: Uint8Array } | { ok: false; reason: string } {
  if (value.length > 64 * 1024 * 1024) return { ok: false, reason: "The chart export is too large." };
  const match = /^data:([^;,]+)((?:;[^,]+)*),(.*)$/s.exec(value);
  if (!match || !["image/png", "text/csv", "application/json"].includes(match[1])) return { ok: false, reason: "Unsupported chart export data." };
  try {
    if (match[2].split(";").includes("base64")) {
      const binary = atob(match[3]);
      return { ok: true, mimeType: match[1], bytes: Uint8Array.from(binary, character => character.charCodeAt(0)) };
    }
    return { ok: true, mimeType: match[1], bytes: new TextEncoder().encode(decodeURIComponent(match[3])) };
  } catch { return { ok: false, reason: "Chart export data is not valid." }; }
}

function exportFilename(value: string | undefined, mimeType: string): string | undefined {
  const extension = mimeType === "image/png" ? ".png" : mimeType === "text/csv" ? ".csv" : ".json";
  const fallback = `control-studio-export${extension}`;
  const filename = value?.trim() || fallback;
  if (filename !== filename.split(/[\\/]/).pop() || !filename.toLowerCase().endsWith(extension)) return undefined;
  return filename;
}
