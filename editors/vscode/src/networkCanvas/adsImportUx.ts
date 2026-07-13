export const OPEN_RUN_ACTION = "Open truST sidebar";

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
        "Open the truST sidebar and start the selected target, then import again.",
      modal: true,
      actions: [OPEN_RUN_ACTION],
    };
  }
  return {
    message: "Could not add ADS variables.",
    detail:
      "The selected runtime could not complete the ADS import. Reconnect or update it, then try again.",
    modal: false,
    actions: [],
  };
}
