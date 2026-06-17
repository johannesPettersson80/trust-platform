import * as net from "net";
import * as vscode from "vscode";

import {
  parseControlEndpoint,
  type ParsedControlEndpoint,
} from "./runtimeControl";

export interface RuntimeControlRequestOptions {
  timeoutMs?: number;
  cancellationToken?: vscode.CancellationToken;
  socketFactory?: RuntimeControlSocketFactory;
}

export interface RuntimeControlSocket {
  setTimeout(timeout: number, callback?: () => void): this;
  once(event: string, listener: (...args: unknown[]) => void): this;
  on(event: string, listener: (...args: unknown[]) => void): this;
  removeListener(event: string, listener: (...args: unknown[]) => void): this;
  write(data: string, callback?: (error?: Error | null) => void): boolean;
  destroy(): void;
}

export type RuntimeControlSocketFactory = (
  parsed: ParsedControlEndpoint
) => RuntimeControlSocket;

export class RuntimeControlError extends Error {
  constructor(
    message: string,
    public readonly code?: string
  ) {
    super(code ? `${code}: ${message}` : message);
    this.name = "RuntimeControlError";
  }
}

let nextRequestId = 1;

export async function probeRuntimeControlEndpoint(
  endpoint: string,
  options: RuntimeControlRequestOptions = {}
): Promise<boolean> {
  if (options.cancellationToken?.isCancellationRequested) {
    return false;
  }
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    return false;
  }
  const timeoutMs = options.timeoutMs ?? 400;
  const socketFactory = options.socketFactory ?? createRuntimeControlSocket;

  return await new Promise<boolean>((resolve) => {
    let settled = false;
    const socket = socketFactory(parsed);
    let disposables: vscode.Disposable[] = [];

    const finish = (value: boolean): void => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      disposeAll(disposables);
      resolve(value);
    };
    disposables = cancellationDisposables(options, () => finish(false));

    socket.setTimeout(timeoutMs, () => finish(false));
    socket.once("error", () => finish(false));
    socket.once("close", () => finish(false));
    socket.once("connect", () => finish(true));
  });
}

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
  const parsed = parseControlEndpoint(endpoint);
  if (!parsed) {
    throw new Error(`invalid control endpoint '${endpoint}'`);
  }

  const timeoutMs = options.timeoutMs ?? 2000;
  const socketFactory = options.socketFactory ?? createRuntimeControlSocket;
  const requestId = nextRequestId++;
  const requestEnvelope = {
    id: requestId,
    type: requestType,
    params,
    auth: authToken,
  };

  return await new Promise<T>((resolve, reject) => {
    let settled = false;
    let buffer = "";
    const socket = socketFactory(parsed);
    let disposables: vscode.Disposable[] = [];

    const finish = (callback: () => void): void => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      disposeAll(disposables);
      socket.removeListener("data", dataHandler);
      socket.removeListener("error", errorHandler);
      socket.removeListener("close", closeHandler);
      callback();
    };
    disposables = cancellationDisposables(options, () =>
      finish(() => reject(new Error("Cancelled.")))
    );

    const errorHandler = (error: unknown): void => {
      const message = error instanceof Error ? error.message : String(error);
      finish(() => reject(new Error(message)));
    };
    const closeHandler = (): void => {
      finish(() => reject(new Error("Runtime control connection closed.")));
    };
    const dataHandler = (chunk: unknown): void => {
      buffer += Buffer.isBuffer(chunk) ? chunk.toString() : String(chunk);
      let newlineIndex = buffer.indexOf("\n");
      while (newlineIndex !== -1) {
        const line = buffer.slice(0, newlineIndex).trim();
        buffer = buffer.slice(newlineIndex + 1);
        if (line.length > 0) {
          const handled = handleResponseLine<T>(
            line,
            requestId,
            (value) => finish(() => resolve(value)),
            (error) => finish(() => reject(error))
          );
          if (handled) {
            return;
          }
        }
        newlineIndex = buffer.indexOf("\n");
      }
    };

    socket.setTimeout(timeoutMs, () => {
      finish(() => reject(new Error("control request timeout")));
    });
    socket.once("error", errorHandler);
    socket.once("close", closeHandler);
    socket.once("connect", () => {
      socket.write(`${JSON.stringify(requestEnvelope)}\n`, (error) => {
        if (error) {
          finish(() => reject(error));
        }
      });
    });
    socket.on("data", dataHandler);
  });
}

export async function requestRuntimeStatus(
  endpoint: string,
  authToken: string | undefined,
  options: RuntimeControlRequestOptions = {}
): Promise<unknown> {
  return await sendRuntimeControlRequest(
    endpoint,
    authToken,
    "status",
    undefined,
    options
  );
}

export function isRuntimeControlAuthError(error: unknown): boolean {
  if (error instanceof RuntimeControlError) {
    return isAuthMarker(error.code) || isAuthMarker(error.message);
  }
  if (error instanceof Error) {
    return isAuthMarker(error.message);
  }
  return false;
}

function handleResponseLine<T>(
  line: string,
  requestId: number,
  resolve: (value: T) => void,
  reject: (error: Error) => void
): boolean {
  let parsed: {
    id?: unknown;
    ok?: boolean;
    result?: T;
    error?: unknown;
    code?: unknown;
  };
  try {
    parsed = JSON.parse(line);
  } catch (error) {
    reject(error instanceof Error ? error : new Error(String(error)));
    return true;
  }

  if (parsed.id !== undefined && parsed.id !== requestId) {
    return false;
  }
  if (parsed.ok === true) {
    resolve(parsed.result as T);
    return true;
  }
  const code =
    typeof parsed.code === "string" && parsed.code.trim().length > 0
      ? parsed.code.trim()
      : undefined;
  const message =
    typeof parsed.error === "string" && parsed.error.trim().length > 0
      ? parsed.error.trim()
      : "control request rejected";
  reject(new RuntimeControlError(message, code));
  return true;
}

function createRuntimeControlSocket(
  parsed: ParsedControlEndpoint
): RuntimeControlSocket {
  return parsed.kind === "tcp"
    ? net.createConnection({ host: parsed.host, port: parsed.port })
    : net.createConnection({ path: parsed.path });
}

function cancellationDisposables(
  options: RuntimeControlRequestOptions,
  onCancel: () => void
): vscode.Disposable[] {
  const token = options.cancellationToken;
  if (!token) {
    return [];
  }
  return [token.onCancellationRequested(onCancel)];
}

function disposeAll(disposables: vscode.Disposable[]): void {
  for (const disposable of disposables) {
    disposable.dispose();
  }
}

function isAuthMarker(value: string | undefined): boolean {
  if (!value) {
    return false;
  }
  const normalized = value.toLowerCase();
  return (
    normalized.includes("auth") ||
    normalized.includes("unauthorized") ||
    normalized.includes("permission")
  );
}
