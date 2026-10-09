// The VS Code side of the webview.

import type { WebviewToHost } from "../messages";

export interface VsCodeApi {
  postMessage(message: WebviewToHost): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

export function vscodeApi(): VsCodeApi {
  return acquireVsCodeApi();
}
