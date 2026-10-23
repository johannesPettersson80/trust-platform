import {
  classifyOpcuaBrowseError,
  type OpcuaErrorAction,
} from "./opcuaClientModel";

export interface BrowseErrorView {
  readonly code: string;
  readonly action: OpcuaErrorAction;
  readonly title: string;
  readonly detail: string;
}

export function classifyBrowseError(
  protocol: string,
  error: { code?: string; message?: string }
): BrowseErrorView {
  if (protocol !== "ads") {
    return classifyOpcuaBrowseError(error);
  }
  const code = error.code ?? "symbol_upload_failed";
  const fallback = error.message ?? "ADS symbol browse failed.";
  switch (code) {
    case "ads_port_unavailable":
      return {
        code,
        action: "retry",
        title: "ADS port unavailable",
        detail:
          "The selected ADS server did not answer. Check the port and that this server is running, then browse again.",
      };
    case "symbol_upload_unsupported":
      return {
        code,
        action: "none",
        title: "Symbol Upload unsupported",
        detail:
          "This ADS server is reachable but does not expose Beckhoff Symbol Upload. Enable symbol generation for that server or choose another ADS port.",
      };
    case "empty_symbol_table":
      return {
        code,
        action: "none",
        title: "No compatible symbols",
        detail:
          "The ADS server returned an empty symbol table or no symbols that truST can import.",
      };
    default:
      return {
        code,
        action: "retry",
        title: "ADS symbol browse failed",
        detail: fallback,
      };
  }
}
