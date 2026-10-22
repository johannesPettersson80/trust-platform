import { spawn, type ChildProcessWithoutNullStreams } from "child_process";
import * as vscode from "vscode";
import { createMessageConnection, type MessageConnection, StreamMessageReader, StreamMessageWriter } from "vscode-jsonrpc/node";
import { getBinaryPath } from "../../binary";

/** Starts allowed within `START_WINDOW_MS`: the first start and three restarts. */
const MAX_STARTS = 4;
const START_WINDOW_MS = 60_000;
/** How long a disposed engine has to end once its standard input is closed, before it is killed. */
const END_GRACE_MS = 2_000;
/** The end of the engine's standard error, which tells why it stopped. */
const STDERR_TAIL_CHARS = 2_000;

type NotificationHandler = (params: unknown) => void;

export interface StartEngineOptions {
  /** The engine's executable; `engineExecutable` resolves it for the window. */
  readonly executable: string;
  /** Called once, with the reason, when the engine stops for good. */
  readonly onFailure?: (reason: string) => void;
}

/** The engine's executable: the setting `trust.controlStudio.executablePath`, else the bundled binary. */
export function engineExecutable(context: vscode.ExtensionContext): string {
  return getBinaryPath(context, "trust-control-studio", "controlStudio.executablePath");
}

/** Starts the engine, `trust-control-studio serve`, which the window keeps for its lifetime (§0.1a). */
export function startEngine(options: StartEngineOptions): EngineProcess {
  return new EngineProcess(options);
}

/**
 * The engine process and its JSON-RPC connection. An unexpected exit starts the engine again, as long as it
 * starts no more than `MAX_STARTS` times in `START_WINDOW_MS`; after that the reason is kept, and every call
 * fails with it. A call made while the engine restarts waits for the new connection. Disposing closes the
 * engine's standard input, which ends it.
 */
export class EngineProcess implements vscode.Disposable {
  private readonly handlers = new Map<string, Set<NotificationHandler>>();
  /** The methods whose notifications the current connection already dispatches. */
  private registeredMethods = new Set<string>();
  private readonly startTimes: number[] = [];
  private readonly waiting: Array<{
    resolve: (connection: MessageConnection) => void;
    reject: (error: Error) => void;
  }> = [];
  private child: ChildProcessWithoutNullStreams | undefined;
  private current: MessageConnection | undefined;
  private stderrTail = "";
  private failure: string | undefined;
  private disposed = false;

  constructor(private readonly options: StartEngineOptions) {
    this.start();
  }

  /** The live connection. It waits while the engine restarts, and fails once the engine stopped for good. */
  connection(): Promise<MessageConnection> {
    if (this.current) {
      return Promise.resolve(this.current);
    }
    if (this.failure) {
      return Promise.reject(new Error(this.failure));
    }
    return new Promise((resolve, reject) => this.waiting.push({ resolve, reject }));
  }

  /** Runs `handler` for each notification `method` the engine sends, on every connection it has. */
  onNotification(method: string, handler: NotificationHandler): vscode.Disposable {
    let set = this.handlers.get(method);
    if (!set) {
      set = new Set();
      this.handlers.set(method, set);
    }
    const handlersOfMethod = set;
    handlersOfMethod.add(handler);
    if (this.current) {
      this.listenFor(this.current, method);
    }
    return new vscode.Disposable(() => {
      handlersOfMethod.delete(handler);
    });
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.failure = "The Control Studio engine was disposed.";
    for (const waiter of this.waiting.splice(0)) {
      waiter.reject(new Error(this.failure));
    }
    this.current?.dispose();
    this.current = undefined;
    const child = this.child;
    this.child = undefined;
    if (child) {
      // Closing the standard input ends the engine (specification 35, §0.1a).
      child.stdin.end();
      const kill = setTimeout(() => {
        if (child.exitCode === null && child.signalCode === null) {
          child.kill();
        }
      }, END_GRACE_MS);
      child.once("exit", () => clearTimeout(kill));
    }
  }

  private start(): void {
    const child = spawn(this.options.executable, ["serve"], {
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    this.child = child;
    this.startTimes.push(Date.now());
    this.stderrTail = "";
    child.stderr.on("data", (chunk: Buffer) => {
      this.stderrTail = `${this.stderrTail}${chunk.toString("utf8")}`.slice(-STDERR_TAIL_CHARS);
    });
    // A write after the engine ended fails here; the exit below reports the stop.
    child.stdin.on("error", () => undefined);
    child.on("error", (error) => this.stopped(child, `it could not run (${error.message})`));
    child.on("exit", (code, signal) =>
      this.stopped(
        child,
        code !== null ? `it exited with code ${code}` : `it was stopped by ${signal}`
      )
    );

    // The engine counts as started once the process runs: a binary that cannot be launched fails before any request is sent, so
    // its waiters get the start failure, not a broken connection.
    child.once("spawn", () => {
      if (child !== this.child || this.disposed) {
        return;
      }
      const connection = createMessageConnection(
        new StreamMessageReader(child.stdout),
        new StreamMessageWriter(child.stdin)
      );
      this.current = connection;
      this.registeredMethods = new Set();
      for (const method of this.handlers.keys()) {
        this.listenFor(connection, method);
      }
      connection.listen();
      for (const waiter of this.waiting.splice(0)) {
        waiter.resolve(connection);
      }
    });
  }

  private listenFor(connection: MessageConnection, method: string): void {
    if (this.registeredMethods.has(method) || connection !== this.current) {
      return;
    }
    this.registeredMethods.add(method);
    connection.onNotification(method, (params: unknown) => {
      for (const handler of this.handlers.get(method) ?? []) {
        handler(params);
      }
    });
  }

  private stopped(child: ChildProcessWithoutNullStreams, reason: string): void {
    if (child !== this.child) {
      return;
    }
    this.child = undefined;
    this.current = undefined;
    if (this.disposed) {
      return;
    }
    const now = Date.now();
    const recentStarts = this.startTimes.filter((time) => now - time <= START_WINDOW_MS);
    const detail = lastLine(this.stderrTail);
    const why = `The Control Studio engine stopped: ${reason}${detail ? `; its last message: ${detail}` : ""}.`;
    if (child.pid === undefined) {
      this.fail(`${why} It could not be started.`);
      return;
    }
    if (recentStarts.length >= MAX_STARTS) {
      this.fail(
        `${why} It was not restarted again: the limit is ${MAX_STARTS} starts in ${START_WINDOW_MS / 1000} seconds.`
      );
      return;
    }
    this.start();
  }

  private fail(reason: string): void {
    this.failure = reason;
    for (const waiter of this.waiting.splice(0)) {
      waiter.reject(new Error(reason));
    }
    this.options.onFailure?.(reason);
  }
}

function lastLine(text: string): string {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return lines[lines.length - 1] ?? "";
}
