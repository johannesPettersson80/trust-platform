import * as vscode from "vscode";
import type { ProposalModel, ProposalStatus } from "./proposalModel";

export const PROPOSAL_SCHEME = "trust-hmi-proposal";
export interface ProposalPresentation {
  uri: string;
  activate(): void;
  close(): void;
}
export type ReviewPresenter = (
  model: ProposalModel,
  files: Record<string, string>,
  signal: AbortSignal,
) => Promise<ProposalPresentation>;
interface ProposalSession {
  uri: vscode.Uri;
  project: vscode.Uri;
  model: ProposalModel;
  text: string;
  apply(): Promise<void>;
  discard(): Promise<void>;
  active: boolean;
  busy: boolean;
  status?: ProposalStatus;
  panel?: vscode.WebviewPanel;
  ready: Promise<void>;
  started(): void;
  cancelled(): void;
}
const sessions = new Map<string, ProposalSession>();
export const proposalFor = (uri: vscode.Uri): ProposalSession | undefined => sessions.get(uri.toString());

function setStatus(session: ProposalSession, status: ProposalStatus, message?: string): void {
  session.status = status;
  session.active = status === "ready" || status === "error";
  void session.panel?.webview.postMessage({ type: "proposalStatus", status, message });
}

/** Both the agent adapter and the review controls publish their committed draft outcomes. */
export function updateProposalState(
  workspaceId: string,
  draftId: string,
  status: ProposalStatus,
  revision?: string,
): void {
  for (const session of sessions.values()) {
    if (session.uri.path.split("/")[1] !== workspaceId || session.model.draftId !== draftId) continue;
    if (status === "stale" && revision !== undefined && Number(session.model.revision) >= Number(revision)) continue;
    setStatus(session, status);
  }
}

/** Virtual documents cannot write source files and carry no caller-selected project authority. */
export function registerProposalDocuments(): vscode.Disposable {
  return vscode.workspace.registerTextDocumentContentProvider(PROPOSAL_SCHEME, {
    provideTextDocumentContent(uri) {
      const session = proposalFor(uri);
      if (!session) throw new Error("EXPIRED_DRAFT: this proposal view is no longer available.");
      return session.text;
    },
  });
}

export async function openProposal(options: {
  workspaceId: string;
  project: vscode.Uri;
  model: ProposalModel;
  files: Record<string, string>;
  apply(): Promise<void>;
  discard(): Promise<void>;
  signal: AbortSignal;
}): Promise<ProposalPresentation> {
  const { model, signal } = options;
  if (signal.aborted) throw new Error("CANCELLED: proposal review cancelled.");
  const uri = vscode.Uri.from({
    scheme: PROPOSAL_SCHEME,
    path: `/${options.workspaceId}/${model.draftId}/hmi/pages/proposal-${model.revision}.toml`,
  });
  const key = uri.toString();
  let session = sessions.get(key);
  if (!session) {
    let started!: () => void, cancelled!: () => void;
    const ready = new Promise<void>((resolve, reject) => {
      started = resolve;
      cancelled = () => reject(new Error("CANCELLED: proposal view closed."));
    });
    // The view may fail to open before anybody awaits readiness.
    void ready.catch(() => undefined);
    session = {
      uri,
      project: options.project,
      model,
      text: options.files[model.files[0].file],
      apply: options.apply,
      discard: options.discard,
      active: false,
      busy: false,
      ready,
      started,
      cancelled,
    };
    sessions.set(key, session);
  } else {
    session.model = model;
    session.active = false;
    session.apply = options.apply;
    session.discard = options.discard;
  }
  const owned = session;
  const close = () => {
    owned.panel?.dispose();
    sessions.delete(key);
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  let abort: (() => void) | undefined;
  try {
    await vscode.commands.executeCommand("vscode.openWith", uri, "trust-lsp.hmiBuilder", { preview: true });
    await Promise.race([
      owned.ready,
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error("RENDER_FAILED: proposal view did not become ready.")), 15000);
        abort = () => reject(new Error("CANCELLED: proposal review cancelled."));
        signal.addEventListener("abort", abort, { once: true });
        if (signal.aborted) abort();
      }),
    ]);
    void owned.panel?.webview.postMessage({ type: "proposal", proposal: model, active: false });
    return {
      uri: key,
      close,
      activate() {
        owned.active = true;
        owned.status = "ready";
        void owned.panel?.webview.postMessage({ type: "proposalStatus", status: "ready" });
      },
    };
  } catch (error) {
    close();
    throw error;
  } finally {
    if (timer) clearTimeout(timer);
    if (abort) signal.removeEventListener("abort", abort);
  }
}

/** This mode accepts review actions only; normal editor, deployment and runtime messages have no route. */
export function resolveProposal(session: ProposalSession, panel: vscode.WebviewPanel): void {
  session.panel = panel;
  panel.title = `Proposal · ${session.model.files[0].title}`;
  const postStatus = (status: ProposalStatus, message?: string) => setStatus(session, status, message);
  const receive = panel.webview.onDidReceiveMessage(async (message: { type?: string; index?: number }) => {
    if (message.type === "ready") {
      void panel.webview.postMessage({ type: "proposal", proposal: session.model, active: session.active });
      session.started();
      return;
    }
    if (message.type === "proposalOpenSource" && session.status === "applied") {
      const index = message.index ?? 0;
      if (!Number.isInteger(index) || !session.model.files[index]) return;
      await vscode.commands.executeCommand(
        "vscode.openWith",
        vscode.Uri.joinPath(session.project, session.model.files[index].file),
        "trust-lsp.hmiBuilder",
      );
      return;
    }
    if (!session.active || session.busy || !["proposalApply", "proposalDiscard"].includes(message.type ?? "")) return;
    if (message.type === "proposalApply" && !session.model.engineeringValid) return;
    session.busy = true;
    try {
      if (message.type === "proposalApply") {
        await session.apply();
        postStatus("applied");
      } else {
        await session.discard();
        postStatus("discarded");
        panel.dispose();
      }
    } catch (error) {
      postStatus("error", error instanceof Error ? error.message : String(error));
    } finally {
      session.busy = false;
    }
  });
  panel.onDidDispose(() => {
    receive.dispose();
    session.panel = undefined;
    session.cancelled();
    sessions.delete(session.uri.toString());
  });
}
