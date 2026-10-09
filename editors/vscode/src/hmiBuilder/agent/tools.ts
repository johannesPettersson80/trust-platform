import { AuthoringLifetime, cancellableNativeRead } from "./lifetime";
import { openProposal, updateProposalState } from "../editor/proposals";
import { validateToolInput } from "./schema";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { renderLibrarySheets, renderPatternReference } from "./library";
import type { ComponentDefinition } from "../editor/catalogue";
import { AuthoringService } from "./service";
import { requireRetainedPages } from "./drafts";
import { createHash } from "crypto";
import * as vscode from "vscode";
import type { LspClientProvider } from "../../lm-tools/shared";
import { invokeAuthoring, AuthoringResult, AuthoringErrorCode } from "./boundary";
import { WorkspaceDocuments } from "./documents";
import { AuthoringQueries } from "./queries";

export const workspaceIdentity = (uri: vscode.Uri): string =>
  createHash("sha256").update(uri.toString()).digest("hex").slice(0, 24);
const CODES = new Set<AuthoringErrorCode>([
  "INVALID_ARGUMENT",
  "UNSUPPORTED_VERSION",
  "PROJECT_UNAVAILABLE",
  "COMPILATION_FAILED",
  "CONFLICT",
  "CANCELLED",
  "VALIDATION_FAILED",
  "RENDER_FAILED",
  "NOT_CONNECTED",
  "UNSUPPORTED_CONTEXT",
  "LIMIT_EXCEEDED",
  "EXPIRED_DRAFT",
]);
function errorResult(error: unknown, cancelled = false): AuthoringResult {
  const message = error instanceof Error ? error.message : String(error);
  const prefix = message.split(":", 1)[0] as AuthoringErrorCode;
  if (!CODES.has(prefix) && (cancelled || error instanceof vscode.CancellationError))
    return { ok: false, code: "CANCELLED", message: "The authoring request was cancelled.", retryable: false };
  return { ok: false, code: CODES.has(prefix) ? prefix : "PROJECT_UNAVAILABLE", message, retryable: false };
}

/** Native VS Code adapter. Project authority comes from the open window, never an input path. */
export class HmiAgentTool {
  constructor(
    readonly name: string,
    private readonly getClient?: LspClientProvider,
    private readonly environment?: { storageRoot: string; fontFile: string },
    private readonly timeoutMs?: number,
  ) {}
  async invoke(options: { input: unknown }, token: vscode.CancellationToken): Promise<vscode.LanguageModelToolResult> {
    const lifetime = new AuthoringLifetime(this.name, token, this.timeoutMs);
    const controller = lifetime.controller;
    let response: AuthoringResult;
    const images: string[] = [];
    try {
      const raw = options.input as { workspaceId?: unknown } | null;
      const folders = vscode.workspace.workspaceFolders ?? [];
      const folder =
        typeof raw?.workspaceId === "string"
          ? folders.find((candidate) => workspaceIdentity(candidate.uri) === raw.workspaceId)
          : folders.length === 1
            ? folders[0]
            : undefined;
      if (!folder) {
        response = {
          ok: false,
          code: "PROJECT_UNAVAILABLE",
          message: "Select one of the connected workspace IDs.",
          retryable: false,
          details: {
            workspaces: folders.map((candidate) => ({
              name: candidate.name,
              workspaceId: workspaceIdentity(candidate.uri),
            })),
          },
        };
      } else {
        const workspaceId = workspaceIdentity(folder.uri);
        const envelope =
          raw && typeof raw === "object" && !Array.isArray(raw)
            ? { ...raw, workspaceId: raw.workspaceId ?? workspaceId }
            : raw;
        response = await invokeAuthoring(envelope, { workspaceId, signal: controller.signal }, async (input) => {
          validateToolInput(this.name, input);
          const nativeFor =
            (requestToken: vscode.CancellationToken): import("./queries").NativeCommand =>
            async (command, argument) => {
              const client = this.getClient?.();
              if (!client) throw new Error("NOT_CONNECTED: the truST language server is not ready.");
              return cancellableNativeRead(requestToken, () =>
                client.sendRequest<Record<string, unknown>>(
                  "workspace/executeCommand",
                  { command, arguments: [argument] },
                  requestToken,
                ),
              );
            };
          const native = nativeFor(lifetime.token);
          const documents = new WorkspaceDocuments(folder.uri);
          const queries = new AuthoringQueries(documents, native);
          let data: Record<string, unknown>;
          let committed = false;
          if (this.name === "hmi_get_context") {
            data = await queries.context({ ...input, partial: true });
            if (this.environment) {
              const service = new AuthoringService(
                workspaceId,
                documents,
                native,
                this.environment.storageRoot,
                this.environment.fontFile,
              );
              data.drafts = await service.store.list(workspaceId);
            }
          } else if (this.name === "hmi_browse_library") {
            if (
              Object.keys(input).some(
                (key) => !["query", "cursor", "limit", "theme", "category", "state"].includes(key),
              ) ||
              (input.query !== undefined && typeof input.query !== "string") ||
              (input.cursor !== undefined && typeof input.cursor !== "number")
            )
              throw new Error("INVALID_ARGUMENT: provide query text and an integer cursor.");
            data = await queries.library(
              input.query as string | undefined,
              input.cursor as number | undefined,
              (input.limit ?? 12) as number,
              input.category as string | undefined,
            );
            if (!this.environment) throw new Error("NOT_CONNECTED: component preview renderer unavailable.");
            const theme = input.theme ?? "dark";
            if (theme !== "dark" && theme !== "light")
              throw new Error("INVALID_ARGUMENT: theme must be dark or light.");
            const rendered = await renderLibrarySheets(
              data.components as ComponentDefinition[],
              this.environment.fontFile,
              theme,
              controller.signal,
              data.symbols as CustomSymbolModel[],
              input.state as string | undefined,
            );
            for (const sheet of rendered.sheets) sheet.manifest.catalogueRevision = data.catalogueRevision;
            images.push(...rendered.images);
            data.sheets = rendered.sheets;
            data.origin = "synthetic";
            for (const pattern of data.compositionPatterns as Array<{
              reference?: import("./patternReferences").PatternReference;
            }>) {
              if (!pattern.reference) continue;
              const preview = await renderPatternReference(
                pattern.reference,
                this.environment.fontFile,
                theme,
                controller.signal,
              );
              pattern.reference.imageIndex = images.length;
              images.push(preview.png);
            }
            data.unpictured = (data.components as ComponentDefinition[])
              .filter((component) => !rendered.sheets.some((sheet) => sheet.types.includes(component.type)))
              .map((component) => component.type);
          } else if (this.name === "hmi_inspect_page") {
            let override: { text: string; revision: string; draftId: string } | undefined;
            if (input.draftId !== undefined) {
              if (!this.environment) throw new Error("NOT_CONNECTED: authoring storage is unavailable.");
              const service = new AuthoringService(
                workspaceId,
                documents,
                native,
                this.environment.storageRoot,
                this.environment.fontFile,
              );
              const draft = await service.store.get(String(input.draftId));
              if (draft.workspaceId !== workspaceId)
                throw new Error("PROJECT_UNAVAILABLE: draft belongs to another workspace.");
              if (draft.revision !== input.revision) throw new Error("CONFLICT: draft revision changed.");
              requireRetainedPages(draft);
              if (!Object.prototype.hasOwnProperty.call(draft.files, String(input.file)))
                throw new Error("INVALID_ARGUMENT: select a file in the draft.");
              override = { text: draft.files[String(input.file)], revision: draft.revision, draftId: draft.id };
            }
            data = await queries.inspect(
              String(input.file),
              input.ids as string[] | undefined,
              input.region as import("./queries").InspectionRegion | undefined,
              override,
            );
          } else {
            if (!this.environment) throw new Error("NOT_CONNECTED: authoring storage is unavailable.");
            const environment = this.environment;
            const present: import("../editor/proposals").ReviewPresenter = (model, files, signal) => {
              const action = async (operation: "apply" | "discard") => {
                if (
                  !vscode.workspace.workspaceFolders?.some(
                    (current) => current.uri.toString() === folder.uri.toString(),
                  )
                )
                  throw new Error("PROJECT_UNAVAILABLE: reopen the proposal's workspace before applying changes.");
                const cancellation = new AuthoringLifetime("hmi_apply_draft");
                try {
                  const owner = new AuthoringService(
                    workspaceId,
                    documents,
                    nativeFor(cancellation.token),
                    environment.storageRoot,
                    environment.fontFile,
                  );
                  await owner.execute(
                    "hmi_apply_draft",
                    { draftId: model.draftId, revision: model.revision, action: operation, save: false },
                    cancellation.signal,
                  );
                  updateProposalState(workspaceId, model.draftId, operation === "apply" ? "applied" : "discarded");
                } finally {
                  cancellation.dispose();
                }
              };
              return openProposal({
                workspaceId,
                project: folder.uri,
                model,
                files,
                signal,
                apply: () => action("apply"),
                discard: () => action("discard"),
              });
            };
            const service = new AuthoringService(
              workspaceId,
              documents,
              native,
              environment.storageRoot,
              environment.fontFile,
              present,
            );
            const output = await service.execute(this.name, input, controller.signal);
            if (this.name === "hmi_edit_draft")
              updateProposalState(workspaceId, String(output.data.draftId), "stale", String(output.data.revision));
            if (this.name === "hmi_apply_draft" && ["apply", "discard"].includes(String(input.action)))
              updateProposalState(
                workspaceId,
                String(output.data.draftId),
                input.action === "apply" ? "applied" : "discarded",
              );

            data = output.data;
            committed = output.committed === true;
            images.push(...(output.images ?? []));
          }
          // Once source has committed, preserve the durable receipt even if the caller cancels.
          if (controller.signal.aborted && !committed) throw new Error("CANCELLED: authoring query cancelled.");
          return {
            ok: true,
            data: { ...data, workspaceId },
            revisions: (data.revisions as Record<string, string>) ?? {},
            warnings: [],
          };
        });
      }
    } catch (error) {
      response = errorResult(error, controller.signal.aborted);
      if (lifetime.expired && !response.ok && response.code === "CANCELLED")
        response.message =
          "The authoring request exceeded its time budget and was cancelled. Inspect its draft state before retrying.";
    } finally {
      lifetime.dispose();
    }
    return new vscode.LanguageModelToolResult([
      new vscode.LanguageModelTextPart(JSON.stringify(response)),
      ...images.map((png) => vscode.LanguageModelDataPart.image(Buffer.from(png, "base64"), "image/png")),
    ]);
  }
}
