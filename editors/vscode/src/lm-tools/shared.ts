import {
  setRuntimeControlRequestHandler,
  type RuntimeControlRequestHandler,
} from "./runtimeControl";

export * from "./shared/types";
export * from "./shared/lm";
export * from "./shared/workspace";
export * from "./shared/lsp";

export function __testSetRuntimeControlRequestHandler(
  handler?: RuntimeControlRequestHandler,
): void {
  setRuntimeControlRequestHandler(handler);
}
