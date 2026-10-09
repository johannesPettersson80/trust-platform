import * as vscode from "vscode";

export function operationTimeout(name: string): number {
  // Project discovery compiles the complete declared Plant/Control/OSCAT dependencies.
  if (["hmi_get_context", "hmi_compose_page", "hmi_edit_draft", "hmi_validate_draft"].includes(name)) return 120000;
  if (["hmi_browse_library", "hmi_inspect_page"].includes(name)) return 30000;
  // Composition and validation perform the same cold discovery; they must have its budget too.
  return 120000;
}

/** One cancellation owner for native requests and rendering/private mutation. It never races
 * a completed source transaction with a synthetic timeout result: the operation reports its
 * actual committed outcome before this lifetime is disposed. */
export class AuthoringLifetime implements vscode.Disposable {
  private readonly source: vscode.CancellationTokenSource;
  readonly controller = new AbortController();
  private readonly listener: vscode.Disposable;
  private readonly parentListener?: vscode.Disposable;
  private readonly timer: ReturnType<typeof setTimeout>;
  expired = false;
  constructor(name: string, parent?: vscode.CancellationToken, timeoutMs = operationTimeout(name)) {
    this.source = new vscode.CancellationTokenSource();
    this.parentListener = parent?.onCancellationRequested(() => this.source.cancel());
    if (parent?.isCancellationRequested) this.source.cancel();
    this.listener = this.source.token.onCancellationRequested(() => this.controller.abort());
    if (this.source.token.isCancellationRequested) this.controller.abort();
    this.timer = setTimeout(() => {
      this.expired = true;
      this.source.cancel();
    }, timeoutMs);
  }
  get token(): vscode.CancellationToken {
    return this.source.token;
  }
  get signal(): AbortSignal {
    return this.controller.signal;
  }
  dispose(): void {
    clearTimeout(this.timer);
    this.listener.dispose();
    this.parentListener?.dispose();
    this.source.dispose();
  }
}

/** Native authoring calls only read/compile/format. Stop awaiting their result on cancellation
 * even when a provider ignores its token; late replies cannot resume a private/source edit.
 * Source WorkspaceEdit and durable receipt writes deliberately do not use this helper. */
export function cancellableNativeRead<T>(token: vscode.CancellationToken, request: () => Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let finished = false;
    // The listener is called only after it is registered, so `finish` always finds it.
    const finish = (failed: boolean, value?: T, error?: unknown): void => {
      if (finished) return;
      finished = true;
      listener.dispose();
      if (failed) reject(error);
      else resolve(value as T);
    };
    const listener = token.onCancellationRequested(() => finish(true, undefined, new vscode.CancellationError()));
    if (token.isCancellationRequested) {
      finish(true, undefined, new vscode.CancellationError());
      listener.dispose();
      return;
    }
    Promise.resolve()
      .then(() => {
        if (token.isCancellationRequested) throw new vscode.CancellationError();
        return request();
      })
      .then(
        (value) => finish(false, value),
        (error) => finish(true, undefined, error),
      );
  });
}
