import type { StudioAnalysisSessions } from "./studioAnalysisSessions";
import type { LoopMessageScope } from "./messages";

/** Clears the working study through its owners; runtime adoption/recovery journals remain authoritative. */
export class StudioStudyReset {
  private readonly handled = new Set<string>();
  async handle(message: unknown, scope: LoopMessageScope, sessions: StudioAnalysisSessions, busy: boolean,
    post: (stage: string, payload: Record<string, unknown>) => Promise<void>): Promise<boolean> {
    if (!message || typeof message !== "object") return false;
    const request = message as Record<string, unknown>;
    if (request.kind !== "control-studio.study.reset" || request.schemaVersion !== 1
      || request.projectId !== scope.projectId || request.editorSessionId !== scope.editorSessionId
      || request.loopId !== scope.loopId || request.expectedLoopRevision !== scope.loopRevision
      || typeof request.messageId !== "string") return false;
    if (this.handled.has(request.messageId)) return true;
    this.handled.add(request.messageId);
    try {
      if (busy) throw new Error("Finish or abort the current operation before resetting the study.");
      sessions.engineer?.assertStudyRestoreAllowed();
      const previous = sessions.engineer?.snapshotStudy();
      sessions.cancelIdentification();
      sessions.identification.restore(undefined);
      sessions.model.restore(undefined);
      sessions.engineer?.restoreStudy({ campaigns: [],
        ...(previous?.channelMapping ? { channelMapping: previous.channelMapping } : {}),
        ...(previous?.acquisitionSettings ? { acquisitionSettings: previous.acquisitionSettings } : {}) });
      sessions.validation.restore(undefined);
      sessions.search?.restore(undefined);
      sessions.leadLag.restore(undefined);
      sessions.routh?.invalidate();
      sessions.mimo.clear();
      await post("engineer", { status: "reset", stageEvidence: { hasModel: false, hasCandidate: false, validationPassed: false } });
    } catch (error) {
      await post("engineer", { status: "failed", reason: error instanceof Error ? error.message : String(error) });
    }
    return true;
  }
}
