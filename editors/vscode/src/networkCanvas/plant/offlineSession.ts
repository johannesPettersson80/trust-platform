import { RuntimeControlError } from "../../runtimeControlClient";
import {
  decodePlantContractError,
  decodePlantContractResponse,
  type PlantContractMessage,
} from "./contract";

export interface PlantCommandProcess {
  stdin: { write(data: string): boolean } | null;
  stdout: { on(event: "data", listener: (chunk: Buffer) => void): unknown } | null;
  stderr: { on(event: "data", listener: (chunk: Buffer) => void): unknown } | null;
  on(event: "exit", listener: (code: number | null) => void): unknown;
  kill(): boolean;
}

export class OfflinePlantCommandSession {
  private nextRequestId = 1;
  private outputBuffer = "";
  private errorDetail = "";
  private projectId: string | undefined;
  private disposed = false;
  private readonly pending = new Map<
    number,
    {
      operation: string;
      resolve(value: PlantContractMessage): void;
      reject(error: Error): void;
      timeout: NodeJS.Timeout;
    }
  >();

  constructor(
    private readonly process: PlantCommandProcess,
    private readonly timeoutMs = 5_000
  ) {
    process.stdout?.on("data", (chunk) => this.acceptOutput(chunk));
    process.stderr?.on("data", (chunk) => {
      this.errorDetail = `${this.errorDetail}${chunk.toString("utf8")}`.slice(-4_096);
    });
    process.on("exit", (code) => {
      this.disposed = true;
      this.rejectAll(
        new Error(
          `stopped-project plant session exited${code === null ? "" : ` with code ${code}`}${
            this.errorDetail ? `: ${this.errorDetail.trim()}` : ""
          }`
        )
      );
    });
  }

  async request(
    operation: string,
    params: Record<string, unknown>
  ): Promise<PlantContractMessage> {
    if (this.disposed || !this.process.stdin) {
      throw new Error("stopped-project plant session is unavailable");
    }
    const id = this.nextRequestId++;
    return await new Promise<PlantContractMessage>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`stopped-project plant request timed out: ${operation}`));
      }, this.timeoutMs);
      this.pending.set(id, { operation, resolve, reject, timeout });
      try {
        this.process.stdin?.write(`${JSON.stringify({ id, type: operation, params })}\n`);
      } catch (error) {
        clearTimeout(timeout);
        this.pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.rejectAll(new Error("stopped-project plant session was disposed"));
    this.process.kill();
  }

  private acceptOutput(chunk: Buffer): void {
    this.outputBuffer += chunk.toString("utf8");
    while (true) {
      const newline = this.outputBuffer.indexOf("\n");
      if (newline < 0) return;
      const line = this.outputBuffer.slice(0, newline);
      this.outputBuffer = this.outputBuffer.slice(newline + 1);
      this.acceptLine(line);
    }
  }

  private acceptLine(line: string): void {
    let response: Record<string, unknown>;
    try {
      const parsed = JSON.parse(line) as unknown;
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw new Error("response is not an object");
      }
      response = parsed as Record<string, unknown>;
    } catch (error) {
      this.rejectAll(
        new Error(`invalid stopped-project plant response: ${error instanceof Error ? error.message : error}`)
      );
      return;
    }
    if (!Number.isSafeInteger(response.id)) return;
    const pending = this.pending.get(response.id as number);
    if (!pending) return;
    this.pending.delete(response.id as number);
    clearTimeout(pending.timeout);

    if (response.ok === true) {
      const decoded = decodePlantContractResponse(response.result, this.projectId);
      if (decoded.kind !== "accepted" || decoded.message.operation !== pending.operation) {
        pending.reject(new Error("stopped-project plant response failed contract decoding"));
        return;
      }
      this.projectId ??= decoded.message.project_id;
      pending.resolve(decoded.message);
      return;
    }

    const code = typeof response.error_code === "string" ? response.error_code : undefined;
    const decoded = decodePlantContractError({ code, result: response.result }, this.projectId);
    if (decoded.kind !== "accepted_error" || decoded.message.operation !== pending.operation) {
      pending.reject(new Error("stopped-project plant error failed contract decoding"));
      return;
    }
    this.projectId ??= decoded.message.project_id;
    const detail = typeof response.error === "string" ? response.error : "plant request rejected";
    pending.reject(new RuntimeControlError(detail, decoded.code, decoded.message));
  }

  private rejectAll(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }
}
