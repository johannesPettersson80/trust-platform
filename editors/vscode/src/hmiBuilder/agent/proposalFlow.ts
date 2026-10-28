// The proposal flows of `hmi_apply_draft` (specification 43, AT-09). A review is prepared by the authoring command,
// each page is rendered, the proposal is presented, and the review is recorded. An application runs its preflight,
// writes the files in one editor transaction and records the receipt. The commands decide; these flows only sequence
// the calls and the rendering.

import type { ProposalFile, ProposalModel } from "../editor/proposalModel";
import type { ReviewPresenter } from "../editor/proposals";
import { NoSourceChangeError } from "./applicationState";
import { applyEditorFiles, PreparedFile } from "./apply";
import type { ExplorationModel } from "../editor/explorationModel";
import {
  apply,
  ApplyReceipt,
  context,
  discard,
  EngineProject,
  envelopeFor,
  Envelope,
  FlowResult,
  inputOf,
  review,
  unwrap,
} from "./engineCommands";
import { prepareExploration, ProjectSnapshot } from "./exploration";
import { appendImages, PreviewSession, previewSession, renderState } from "./previews";

/** `hmi_apply_draft` with `review`: the review is prepared, each page rendered and the proposal presented, and the
 * review recorded with the visual review the request carries. */
export async function reviewDraft(
  project: EngineProject,
  fontFile: string,
  request: Envelope,
  present: ReviewPresenter | undefined,
  signal: AbortSignal,
): Promise<FlowResult> {
  const prepared = unwrap(await review(project, "prepare", request));
  const session = await previewSession(project, fontFile, inputOf(request), signal);
  const { draft } = session;
  const exploration = inputOf(request).explore === true ? await explorationOf(session) : undefined;
  const changes = new Map<string, ProposalFile["changes"]>(
    (prepared.data.objectChanges as Array<Record<string, unknown>>).map((change): [string, ProposalFile["changes"]] => [
      String(change.file),
      change as unknown as ProposalFile["changes"],
    ]),
  );
  const titles = new Map<string, string>(
    (prepared.data.pages as Array<{ file: string; title: string }>).map((page): [string, string] => [
      page.file,
      page.title,
    ]),
  );
  const images: string[] = [];
  const previews: Array<Record<string, unknown>> = [];
  const reviewFiles: ProposalFile[] = [];
  for (const file of Object.keys(draft.files)) {
    const rendered = await renderState(session, { file }, "sample");
    const offset = appendImages(images, rendered);
    previews.push({
      file,
      kind: "after",
      imageIndex: offset,
      revision: draft.revision,
      origin: "synthetic",
      manifest: rendered.data.manifest,
    });
    reviewFiles.push({
      file,
      title: titles.get(file) ?? file,
      changes: changes.get(file) as ProposalFile["changes"],
      width: Number(rendered.data.width),
      height: Number(rendered.data.height),
      after: rendered.images[0],
    });
  }
  if (signal.aborted) throw new Error("CANCELLED: proposal review cancelled.");
  const presentation = await present?.(
    {
      draftId: draft.draftId,
      revision: draft.revision,
      intent: prepared.data.intent as ProposalModel["intent"],
      engineeringValid: prepared.data.engineeringValid === true,
      exploration,
      files: reviewFiles,
      findings: prepared.data.findings as ProposalModel["findings"],
    },
    draft.files,
    signal,
  );
  try {
    const recorded = unwrap(await review(project, "record", request));
    presentation?.activate();
    return {
      data: {
        ...prepared.data,
        ...recorded.data,
        previews,
        state: "proposal",
        origin: "synthetic",
        ...(presentation ? { reviewUri: presentation.uri } : {}),
      },
      revisions: recorded.revisions,
      warnings: recorded.warnings,
      images,
    };
  } catch (error) {
    presentation?.close();
    throw error;
  }
}

/** The read-only exploration of a review with `explore: true` (specification 43): the project's pages and popups as the
 * engine observes them (the context's `sources`), with the draft's pages over them. */
async function explorationOf(session: PreviewSession): Promise<ExplorationModel> {
  const { project, draft } = session;
  const answer = unwrap(await context(project, envelopeFor(project.workspaceId, { sections: ["sources"] })));
  return prepareExploration(
    project,
    { id: draft.draftId, revision: draft.revision, files: draft.files },
    session.projectContext,
    answer.data.sources as ProjectSnapshot[],
    session.signal,
  );
}

/** `hmi_apply_draft` with `apply`: the preflight records the application, the files are written in one editor
 * transaction, and the outcome is recorded as the receipt. A replayed key answers its committed outcome, without a
 * second write. */
export async function applyDraft(project: EngineProject, request: Envelope, signal: AbortSignal): Promise<FlowResult> {
  const prepared = unwrap(await apply(project, "prepare", request));
  if (prepared.data.committed === true) return { ...prepared, images: [] };
  const files = prepared.data.files as PreparedFile[];
  const save = prepared.data.save === true;
  let receipt: ApplyReceipt;
  try {
    const outcome = await applyEditorFiles(project.root, files, save, signal);
    receipt = { status: "committed", outcome: { ...outcome } };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    receipt =
      error instanceof NoSourceChangeError
        ? { status: "no-source-change", code: message.split(":")[0], message }
        : { status: "uncertain", message };
  }
  return { ...unwrap(await apply(project, "receipt", request, receipt)), images: [] };
}

/** `hmi_apply_draft` with `discard`: the draft's pages are removed and its identity kept (AT-09). */
export async function discardDraft(project: EngineProject, request: Envelope): Promise<FlowResult> {
  return { ...unwrap(await discard(project, request)), images: [] };
}
