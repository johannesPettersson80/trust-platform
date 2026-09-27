import type { RecordingStateNotice, RecordingUpdate } from "../recordingCoordinator";

interface RecordingControlsDependencies {
  post(kind: string): void;
  setText(selector: string, value: unknown, title?: string): void;
  setWorkflowActionAvailability(update: { recording?: boolean; result?: boolean }): void;
}

function testOwnsAcquisition(): boolean {
  return Boolean(document.querySelector<HTMLElement>("[data-control-studio]")?.dataset.experimentId);
}

/** Live observations cannot expose manual controls over an active Test owner. */
export function renderRecordingActions(recording: boolean): void {
  const test = testOwnsAcquisition();
  const start = document.querySelector<HTMLButtonElement>("[data-control-studio-record-start]");
  const stop = document.querySelector<HTMLButtonElement>("[data-control-studio-record-stop]");
  if (start) { start.hidden = test; start.disabled = test || recording; }
  if (stop) { stop.hidden = test; stop.disabled = test || !recording; }
}

/** Owns the manual Record/Stop controls and their acknowledged lifecycle status. */
export function initializeRecordingControls(dependencies: RecordingControlsDependencies): {
  receive(update: RecordingUpdate): void;
  showRetained(): void;
} {
  const start = document.querySelector<HTMLButtonElement>("[data-control-studio-record-start]");
  const stop = document.querySelector<HTMLButtonElement>("[data-control-studio-record-stop]");
  let pending = false;
  const status = (text: string, startDisabled: boolean, stopDisabled: boolean): void => {
    pending = true;
    dependencies.setText("[data-control-studio-recording-evidence]", text);
    if (start) start.disabled = startDisabled;
    if (stop) stop.disabled = stopDisabled;
    dependencies.setWorkflowActionAvailability({ recording: false });
  };
  start?.addEventListener("click", () => {
    if (testOwnsAcquisition()) return;
    dependencies.post("control-studio.recording.start");
    status("Starting recording…", true, false);
  });
  stop?.addEventListener("click", () => {
    if (testOwnsAcquisition()) return;
    dependencies.post("control-studio.recording.stop");
    status("Stopping recording… retaining runtime scans…", true, true);
  });
  return {
    showRetained(): void {
      if (!pending && !testOwnsAcquisition()) dependencies.setText("[data-control-studio-recording-evidence]", "Retained recording available in Validate.");
    },
    receive(update: RecordingUpdate): void {
      if (testOwnsAcquisition()) { renderRecordingActions(true); return; }
      if (isRecordingStateNotice(update)) {
        if (update.state === "recording") status(update.reason, true, false);
        else status(update.reason, true, false);
        return;
      }
      const startUnconfirmed = /Start acknowledgement unconfirmed/i.test(update.reason);
      pending = startUnconfirmed;
      const complete = update.completeness === "complete" && update.method === "runtime_scan_buffer" && !startUnconfirmed;
      dependencies.setText("[data-control-studio-recording-evidence]", update.reason);
      if (start) start.disabled = false;
      if (stop) stop.disabled = true;
      dependencies.setWorkflowActionAvailability({ recording: complete, ...(complete ? { result: true } : {}) });
      const identify = document.querySelector<HTMLButtonElement>("[data-identify-run]");
      if (identify) identify.title = complete ? "" : update.reason;
    },
  };
}

function isRecordingStateNotice(value: RecordingUpdate): value is RecordingStateNotice {
  return "state" in value;
}
