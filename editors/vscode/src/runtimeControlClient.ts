import * as vscode from "vscode";

import { projectRequest } from "./projectRequest";

// Every request of the extension to a runtime goes through the language server
// (`trust-lsp.runtimeRequest`, runtime protocol annex E0.1): it reads the endpoint, connects, sends
// the request and decodes the answer, the credential failure named by the refusal's code
// (specification 11 §6.9.6). This module forwards a request and turns the answer into a result or
// an error.

const RUNTIME_REQUEST_COMMAND = "trust-lsp.runtimeRequest";

export interface RuntimeControlRequestOptions {
  timeoutMs?: number;
  cancellationToken?: vscode.CancellationToken;
}

/** The runtime refused the request: its text, its code, the result a refusal may carry and the credential failure the
 * code names. */
export class RuntimeControlError extends Error {
  constructor(
    message: string,
    public readonly code?: string,
    public readonly result?: unknown,
    public readonly authFailure?: "missing" | "rejected"
  ) {
    super(code ? `${code}: ${message}` : message);
    this.name = "RuntimeControlError";
  }
}

/** The runtime could not be reached or stopped answering: the request never produced a runtime answer. */
export class RuntimeTransportError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RuntimeTransportError";
  }
}

type RequestAnswer =
  | { readonly outcome: "result"; readonly result: unknown }
  | {
      readonly outcome: "refused";
      readonly error: string;
      readonly code: string | null;
      readonly authFailure: "missing" | "rejected" | null;
      readonly result: unknown;
    }
  | { readonly outcome: "transport" | "malformed"; readonly error: string };

/** Whether the runtime's control endpoint accepts a connection; nothing is sent. With `waitMs` the language server tries
 * again until the endpoint accepts or the wait ends. */
export async function probeRuntimeControlEndpoint(
  endpoint: string,
  options: RuntimeControlRequestOptions & { readonly waitMs?: number } = {}
): Promise<boolean> {
  if (options.cancellationToken?.isCancellationRequested) {
    return false;
  }
  const answer = await untilCancelled(
    projectRequest<{ reachable: boolean }>(RUNTIME_REQUEST_COMMAND, {
      action: "probe",
      endpoint,
      timeoutMs: options.timeoutMs,
      waitMs: options.waitMs,
    }),
    options.cancellationToken
  ).catch(() => undefined);
  return answer?.ok === true && answer.reachable === true;
}

/** One control request: the runtime's result, or a RuntimeControlError for its refusal and a RuntimeTransportError when no
 * runtime answered. */
export async function sendRuntimeControlRequest<T = unknown>(
  endpoint: string,
  authToken: string | undefined,
  requestType: string,
  params?: unknown,
  options: RuntimeControlRequestOptions = {}
): Promise<T> {
  if (options.cancellationToken?.isCancellationRequested) {
    throw new Error("Cancelled.");
  }
  const answer = await untilCancelled(
    projectRequest<RequestAnswer>(RUNTIME_REQUEST_COMMAND, {
      action: "request",
      endpoint,
      authToken,
      type: requestType,
      params,
      timeoutMs: options.timeoutMs,
    }),
    options.cancellationToken
  );
  if (!answer.ok) {
    throw new RuntimeTransportError(answer.error);
  }
  switch (answer.outcome) {
    case "result":
      return answer.result as T;
    case "refused":
      throw new RuntimeControlError(
        answer.error,
        answer.code ?? undefined,
        answer.result ?? undefined,
        answer.authFailure ?? undefined
      );
    case "transport":
      throw new RuntimeTransportError(answer.error);
    default:
      throw new Error(answer.error);
  }
}

/** `request`, or the error `Cancelled.` as soon as `token` asks for it. */
async function untilCancelled<T>(request: Promise<T>, token: vscode.CancellationToken | undefined): Promise<T> {
  if (!token) {
    return await request;
  }
  let subscription: vscode.Disposable | undefined;
  const cancelled = new Promise<never>((_, reject) => {
    subscription = token.onCancellationRequested(() => reject(new Error("Cancelled.")));
  });
  try {
    return await Promise.race([request, cancelled]);
  } finally {
    subscription?.dispose();
  }
}
