import { initializeLoopSetupForm } from "./loopSetupForm";

declare function acquireVsCodeApi(): { postMessage(message: unknown): void };

const vscode = acquireVsCodeApi();
const root = document.querySelector<HTMLElement>("main");
if (root) {
  const form = initializeLoopSetupForm(root, message => vscode.postMessage(message));
  window.addEventListener("message", event => {
    if (event.data?.kind === "control-studio.setup.error") form.showError(String(event.data.reason));
  });
}
