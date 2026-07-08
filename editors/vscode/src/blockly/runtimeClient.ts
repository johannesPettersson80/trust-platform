import { getTrustConfiguration } from "../configuration";

/**
 * Client for communicating with trust-runtime control endpoint
 * Sends I/O write commands for hardware execution from Blockly programs
 */

import * as net from "net";
import * as vscode from "vscode";

export interface RuntimeConfig {
  controlEndpoint: string; // e.g., "unix:///tmp/trust-debug.sock" or "tcp://127.0.0.1:9000"
  controlAuthToken?: string;
  requestTimeoutMs?: number;
}

export interface IoWriteParams {
  address: string; // IEC 61131-3 address (e.g., %QX0.0)
  value: any;
}

export interface ControlRequest {
  id: number;
  type: string;
  auth?: string;
  params?: any;
}

export interface ControlResponse {
  id: number;
  ok: boolean;
  result?: any;
  error?: string;
}

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: Error) => void;
  timeout: NodeJS.Timeout;
}

/**
 * Client for sending commands to trust-runtime control endpoint from Blockly
 */
export class RuntimeClient {
  private socket: net.Socket | null = null;
  private requestId = 1;
  private endpoint: string;
  private authToken?: string;
  private requestTimeoutMs: number;
  private buffer = "";
  private pendingRequests = new Map<number, PendingRequest>();

  constructor(config: RuntimeConfig) {
    this.endpoint = config.controlEndpoint;
    this.authToken = config.controlAuthToken;
    this.requestTimeoutMs = config.requestTimeoutMs ?? 5000;
  }

  /**
   * Connect to the control endpoint
   */
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      let settled = false;

      try {
        // Parse endpoint
        if (this.endpoint.startsWith("tcp://")) {
          const address = this.endpoint.replace("tcp://", "");
          const [host, port] = address.split(":");
          this.socket = net.createConnection(
            { host, port: parseInt(port, 10) },
            () => {
              console.log("Blockly: Connected to trust-runtime via TCP:", address);
              settled = true;
              resolve();
            }
          );
        } else if (this.endpoint.startsWith("unix://")) {
          const socketPath = this.endpoint.replace("unix://", "");
          this.socket = net.createConnection(socketPath, () => {
            console.log("Blockly: Connected to trust-runtime via Unix socket:", socketPath);
            settled = true;
            resolve();
          });
        } else {
          settled = true;
          reject(new Error(`Invalid control endpoint format: ${this.endpoint}`));
          return;
        }

        this.socket.on("data", (data) => {
          this.buffer += data.toString();
          this.processBuffer();
        });

        this.socket.on("error", (err) => {
          console.error("Blockly: Runtime connection error:", err);
          this.rejectAllPending(err instanceof Error ? err : new Error(String(err)));
          if (!settled) {
            settled = true;
            reject(err);
          }
        });

        this.socket.on("close", () => {
          console.log("Blockly: Runtime connection closed");
          this.rejectAllPending(new Error("Connection closed"));
          this.socket = null;
        });

      } catch (error) {
        settled = true;
        reject(error);
      }
    });
  }

  /**
   * Process incoming data buffer
   */
  private processBuffer(): void {
    const lines = this.buffer.split("\n");
    this.buffer = lines.pop() || "";

    for (const line of lines) {
      if (!line.trim()) continue;
      
      try {
        const response: ControlResponse = JSON.parse(line);
        const pending = this.pendingRequests.get(response.id);
        
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(response.id);
          if (response.ok) {
            pending.resolve(response.result);
          } else {
            pending.reject(new Error(response.error || "Unknown error"));
          }
        }
      } catch (error) {
        console.error("Blockly: Failed to parse response:", line, error);
      }
    }
  }

  /**
   * Disconnect from the control endpoint
   */
  disconnect(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
    this.rejectAllPending(new Error("Connection closed"));
  }

  private rejectAllPending(error: Error): void {
    for (const [id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(error);
      this.pendingRequests.delete(id);
    }
  }

  private sendRequest(request: ControlRequest): Promise<any> {
    if (!this.socket) {
      return Promise.reject(new Error("Not connected to runtime"));
    }

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(request.id);
        reject(new Error("Request timeout"));
      }, this.requestTimeoutMs);

      this.pendingRequests.set(request.id, { resolve, reject, timeout });

      this.socket!.write(JSON.stringify(request) + "\n", (err) => {
        if (!err) {
          return;
        }

        const pending = this.pendingRequests.get(request.id);
        if (pending) {
          clearTimeout(pending.timeout);
          this.pendingRequests.delete(request.id);
        }
        reject(err);
      });
    });
  }

  /**
   * Write value to an I/O address (queued, may be overwritten by program)
   */
  async writeIo(address: string, value: any): Promise<void> {
    if (!this.socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type: "io.force",  // Use force instead of write to persist across cycles
      auth: this.authToken,
      params: { address, value },
    };

    await this.sendRequest(request);
  }

  /**
   * Force value on an I/O address until explicitly released.
   */
  async forceIo(address: string, value: any): Promise<void> {
    if (!this.socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type: "io.force",
      auth: this.authToken,
      params: { address, value },
    };

    await this.sendRequest(request);
  }

  /**
   * Release forced value on an I/O address.
   */
  async unforceIo(address: string): Promise<void> {
    if (!this.socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type: "io.unforce",
      auth: this.authToken,
      params: { address },
    };

    await this.sendRequest(request);
  }

  /**
   * Resume runtime execution (ensure it's running PLC cycles)
   */
  async resume(): Promise<void> {
    if (!this.socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type: "resume",
      auth: this.authToken,
    };

    await this.sendRequest(request);
  }

  /**
   * Pause runtime execution
   */
  async pause(): Promise<void> {
    if (!this.socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type: "pause",
      auth: this.authToken,
    };

    await this.sendRequest(request);
  }

  /**
   * Check if connected to runtime
   */
  isConnected(): boolean {
    return this.socket !== null;
  }
}

/**
 * Get runtime configuration from workspace settings
 */
export function getRuntimeConfig(): RuntimeConfig {
  const config = getTrustConfiguration();
  
  return {
    controlEndpoint: config.get("runtime.controlEndpoint") || "unix:///tmp/trust-debug.sock",
    controlAuthToken: config.get("runtime.controlAuthToken"),
    requestTimeoutMs: config.get("runtime.requestTimeoutMs") || 5000,
  };
}
