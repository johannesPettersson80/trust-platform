export const START_RUNTIME_ACTION = "Start runtime";

export interface AdsImportFailurePrompt {
  readonly message: string;
  readonly detail?: string;
  readonly modal: boolean;
  readonly actions: readonly string[];
}

export function adsImportFailurePrompt(reason: string): AdsImportFailurePrompt {
  const detail = reason.trim();
  if (/write-enabled ADS imports need a running runtime/i.test(detail)) {
    return {
      message: "Write-enabled ADS imports need a running runtime.",
      detail:
        "truST must verify the explicit write acknowledgement before importing writable tags. Start the runtime, then import again.",
      modal: true,
      actions: [START_RUNTIME_ACTION],
    };
  }
  return {
    message: `Could not add ADS tags: ${detail || "the import was rejected."}`,
    modal: false,
    actions: [],
  };
}
