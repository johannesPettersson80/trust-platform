import { createHash } from "crypto";
import * as path from "path";
import * as vscode from "vscode";
import type { ComponentDefinition } from "../editor/catalogue";
import type { CustomSymbolModel } from "../drawing/symbols/custom";
import { openProposal, ReviewPresenter, updateProposalState } from "../editor/proposals";
import type { LspClientProvider } from "../../lm-tools/shared";
import { AuthoringLifetime } from "./lifetime";
import { renderLibrarySheets } from "./library";
import {
  AuthoringErrorCode,
  AuthoringResult,
  checkInput,
  compose,
  context,
  EngineProject,
  Envelope,
  edit,
  envelopeFor,
  FlowResult,
  inputOf,
  inspect,
  library,
  validate,
} from "./engineCommands";
import { applyDraft, discardDraft, reviewDraft } from "./proposalFlow";
import { previewSession, renderState, renderStates, simulatorJourney } from "./previews";

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

/** One tool call's answer and the PNG images it carries. */
interface Outcome {
  answer: AuthoringResult;
  images: string[];
}

/** A successful answer of a flow. */
function succeeded(
  data: Record<string, unknown>,
  revisions: Record<string, string> = {},
  warnings: string[] = [],
): AuthoringResult {
  return { ok: true, data, revisions, warnings };
}

/** Native VS Code adapter. Project authority comes from the open window, never an input path. The authoring commands
 * run in `trust-lsp`; this adapter forwards each call to them, runs the renders, and presents the proposal. */
export class HmiAgentTool {
  constructor(
    readonly name: string,
    // Kept for the callers' signature. The commands are reached through VS Code's command registry, not a client.
    private readonly getClient?: LspClientProvider,
    private readonly environment?: { storageRoot: string; fontFile: string },
    private readonly timeoutMs?: number,
  ) {}
  async invoke(options: { input: unknown }, token: vscode.CancellationToken): Promise<vscode.LanguageModelToolResult> {
    const lifetime = new AuthoringLifetime(this.name, token, this.timeoutMs);
    const controller = lifetime.controller;
    let response: AuthoringResult;
    let images: string[] = [];
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
        const request: Envelope =
          raw && typeof raw === "object" && !Array.isArray(raw)
            ? { ...raw, workspaceId: raw.workspaceId ?? workspaceId }
            : raw;
        if (!this.environment) throw new Error("NOT_CONNECTED: authoring storage is unavailable.");
        const project: EngineProject = {
          root: folder.uri,
          workspaceId,
          storage: path.join(this.environment.storageRoot, workspaceId),
          token: lifetime.token,
        };
        const outcome = await this.perform(project, folder, request, this.environment.fontFile, controller.signal);
        images = outcome.images;
        response = outcome.answer.ok
          ? { ...outcome.answer, data: { ...outcome.answer.data, workspaceId } }
          : outcome.answer;
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

  /** Forwards the call to its command, or runs the flow that renders or applies its answer. */
  private async perform(
    project: EngineProject,
    folder: vscode.WorkspaceFolder,
    request: Envelope,
    fontFile: string,
    signal: AbortSignal,
  ): Promise<Outcome> {
    const input = inputOf(request);
    switch (this.name) {
      case "hmi_get_context":
        return { answer: await context(project, request), images: [] };
      case "hmi_browse_library":
        return this.browse(project, request, input, fontFile, signal);
      case "hmi_inspect_page":
        return { answer: await inspect(project, request), images: [] };
      case "hmi_compose_page":
        return { answer: await compose(project, request), images: [] };
      case "hmi_edit_draft": {
        const answer = await edit(project, request);
        if (answer.ok)
          updateProposalState(project.workspaceId, String(answer.data.draftId), "stale", String(answer.data.revision));
        return { answer, images: [] };
      }
      case "hmi_validate_draft":
        return { answer: await validate(project, request), images: [] };
      case "hmi_render_draft": {
        const checked = await checkInput(project, this.name, request);
        if (!checked.ok) return { answer: checked, images: [] };
        const rendered = await renderState(await previewSession(project, fontFile, input, signal), input, "sample");
        return { answer: succeeded(rendered.data), images: rendered.images };
      }
      case "hmi_preview_states": {
        const checked = await checkInput(project, this.name, request);
        if (!checked.ok) return { answer: checked, images: [] };
        const session = await previewSession(project, fontFile, input, signal);
        if (input.mode === "simulator")
          return { answer: succeeded(await simulatorJourney(session, input)), images: [] };
        const rendered = await renderStates(session, input, input.states as string[]);
        return { answer: succeeded(rendered.data), images: rendered.images };
      }
      case "hmi_apply_draft":
        return this.proposal(project, folder, request, input, fontFile, signal);
      default:
        throw new Error("INVALID_ARGUMENT: unknown authoring tool.");
    }
  }

  /** The component library, with each component's sheet rendered in the requested theme. */
  private async browse(
    project: EngineProject,
    request: Envelope,
    input: Record<string, unknown>,
    fontFile: string,
    signal: AbortSignal,
  ): Promise<Outcome> {
    const answer = await library(project, request);
    if (!answer.ok) return { answer, images: [] };
    const data = answer.data;
    const rendered = await renderLibrarySheets(
      data.components as ComponentDefinition[],
      fontFile,
      (input.theme ?? "dark") as "dark" | "light",
      signal,
      data.symbols as CustomSymbolModel[],
      input.state as string | undefined,
    );
    for (const sheet of rendered.sheets) sheet.manifest.catalogueRevision = data.catalogueRevision;
    data.sheets = rendered.sheets;
    data.origin = "synthetic";
    data.unpictured = (data.components as ComponentDefinition[])
      .filter((component) => !rendered.sheets.some((sheet) => sheet.types.includes(component.type)))
      .map((component) => component.type);
    return { answer: succeeded(data, answer.revisions, answer.warnings), images: rendered.images };
  }

  /** `hmi_apply_draft`: the review, the application or the discard, by its action. */
  private async proposal(
    project: EngineProject,
    folder: vscode.WorkspaceFolder,
    request: Envelope,
    input: Record<string, unknown>,
    fontFile: string,
    signal: AbortSignal,
  ): Promise<Outcome> {
    const action = input.action;
    let flow: FlowResult;
    if (action === "review")
      flow = await reviewDraft(project, fontFile, request, this.presenter(project, folder), signal);
    else if (action === "apply") flow = await applyDraft(project, request, signal);
    else if (action === "discard") flow = await discardDraft(project, request);
    else throw new Error("INVALID_ARGUMENT: action must be review, apply or discard.");
    if (action === "apply" || action === "discard")
      updateProposalState(project.workspaceId, String(flow.data.draftId), action === "apply" ? "applied" : "discarded");
    return { answer: succeeded(flow.data, flow.revisions, flow.warnings), images: flow.images };
  }

  /** The proposal host of a review. Its Apply and Discard buttons run the same flows as the tool. */
  private presenter(project: EngineProject, folder: vscode.WorkspaceFolder): ReviewPresenter {
    return (model, files, signal) => {
      const operate = async (operation: "apply" | "discard"): Promise<void> => {
        if (!vscode.workspace.workspaceFolders?.some((current) => current.uri.toString() === folder.uri.toString()))
          throw new Error("PROJECT_UNAVAILABLE: reopen the proposal's workspace before applying changes.");
        const cancellation = new AuthoringLifetime("hmi_apply_draft");
        try {
          const owned: EngineProject = { ...project, token: cancellation.token };
          const request = envelopeFor(project.workspaceId, {
            draftId: model.draftId,
            revision: model.revision,
            action: operation,
            save: false,
          });
          if (operation === "apply") await applyDraft(owned, request, cancellation.signal);
          else await discardDraft(owned, request);
          updateProposalState(project.workspaceId, model.draftId, operation === "apply" ? "applied" : "discarded");
        } finally {
          cancellation.dispose();
        }
      };
      return openProposal({
        workspaceId: project.workspaceId,
        project: folder.uri,
        model,
        files,
        signal,
        apply: () => operate("apply"),
        discard: () => operate("discard"),
      });
    };
  }
}
