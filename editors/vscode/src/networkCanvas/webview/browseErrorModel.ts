import type { OpcuaErrorAction } from "./opcuaClientModel";

// The recovery of a failed browse, as the server answers it (`trust-lsp.devicesCanvasAct`
// `browse_error`, and each discovered ADS port's `error_view`; specification 25 §5.4 and §11.2).
export interface BrowseErrorView {
  readonly code: string;
  readonly action: OpcuaErrorAction;
  readonly title: string;
  readonly detail: string;
}
