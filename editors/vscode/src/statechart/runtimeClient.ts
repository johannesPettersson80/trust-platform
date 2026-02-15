/**
 * Client for communicating with trust-runtime control endpoint
 * Sends I/O write commands for hardware execution
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

/**
 * Client for sending commands to trust-runtime control endpoint
 */
export class RuntimeClient {
  private socket: net.Socket | null = null;
  private requestId = 1;
  private endpoint: string;
  private authToken?: string;
  private requestTimeoutMs: number;
  private buffer = "";

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
      try {
        // Parse endpoint
        if (this.endpoint.startsWith("tcp://")) {
          const address = this.endpoint.replace("tcp://", "");
          const [host, port] = address.split(":");
          this.socket = net.createConnection(
            { host, port: parseInt(port, 10) },
            () => {
              console.log("Connected to trust-runtime via TCP:", address);
              resolve();
            }
          );
        } else if (this.endpoint.startsWith("unix://")) {
          const socketPath = this.endpoint.replace("unix://", "");
          this.socket = net.createConnection(socketPath, () => {
            console.log("Connected to trust-runtime via Unix socket:", socketPath);
            resolve();
          });
        } else {
          reject(new Error(`Invalid control endpoint format: ${this.endpoint}`));
          return;
        }

        this.socket.on("error", (err) => {
          console.error("Runtime connection error:", err);
          reject(err);
        });

        this.socket.on("close", () => {
          console.log("Runtime connection closed");
          this.socket = null;
        });

      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Disconnect from the control endpoint
   */
  disconnect(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
    }
  }

  /**
   * Send a control request and wait for response
   */
  private async sendRequest(type: string, params?: any): Promise<ControlResponse> {
    const socket = this.socket;
    if (!socket) {
      throw new Error("Not connected to runtime");
    }

    const id = this.requestId++;
    const request: ControlRequest = {
      id,
      type,
      auth: this.authToken,
      params,
    };

    return new Promise((resolve, reject) => {
      let settled = false;
      const cleanup = () => {
        clearTimeout(timeout);
        socket.removeListener("data", dataHandler);
        socket.removeListener("error", errorHandler);
        socket.removeListener("close", closeHandler);
      };
      const settle = (handler: () => void) => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        handler();
      };
      const timeout = setTimeout(() => {
        settle(() => reject(new Error(`Request ${id} timed out`)));
      }, this.requestTimeoutMs);

      // Setup response handler
      const dataHandler = (data: Buffer) => {
        this.buffer += data.toString();

        // Try to parse complete JSON lines
        const lines = this.buffer.split("\n");
        this.buffer = lines.pop() || "";

        for (const line of lines) {
          if (!line.trim()) continue;

          try {
            const response: ControlResponse = JSON.parse(line);
            if (response.id === id) {
              settle(() => resolve(response));
            }
          } catch (err) {
            console.error("Failed to parse response:", err);
          }
        }
      };

      const errorHandler = (err: Error) => {
        settle(() => reject(err));
      };
      const closeHandler = () => {
        settle(() => reject(new Error("Runtime connection closed")));
      };

      socket.on("data", dataHandler);
      socket.on("error", errorHandler);
      socket.on("close", closeHandler);

      // Send request
      const requestLine = JSON.stringify(request) + "\n";
      socket.write(requestLine, (err) => {
        if (err) {
          settle(() => reject(err));
        }
      });
    });
  }

  /**
   * Convert a value to the format expected by trust-runtime
   */
  private formatValue(value: any): string {
    if (typeof value === "boolean") {
      return value ? "TRUE" : "FALSE";
    }
    if (typeof value === "number") {
      return value.toString();
    }
    return String(value);
  }

  /**
   * Write a value to an I/O address
   */
  async writeIo(address: string, value: any): Promise<void> {
    try {
      const formattedValue = this.formatValue(value);
      const response = await this.sendRequest("io.write", { address, value: formattedValue });
      
      if (!response.ok) {
        throw new Error(response.error || "IO write failed");
      }
      
      console.log(`✅ Wrote ${formattedValue} to ${address}`);
    } catch (error) {
      console.error(`❌ Failed to write to ${address}:`, error);
      throw error;
    }
  }

  /**
   * Read a value from an I/O address
   */
  async readIo(address: string): Promise<any> {
    try {
      const response = await this.sendRequest("io.read", { address });
      
      if (!response.ok) {
        throw new Error(response.error || "IO read failed");
      }
      
      console.log(`✅ Read ${response.result} from ${address}`);
      return response.result;
    } catch (error) {
      console.error(`❌ Failed to read from ${address}:`, error);
      throw error;
    }
  }

  /**
   * Force a value on an I/O address (stays until unforced)
   */
  async forceIo(address: string, value: any): Promise<void> {
    try {
      const formattedValue = this.formatValue(value);
      const response = await this.sendRequest("io.force", { address, value: formattedValue });
      
      if (!response.ok) {
        throw new Error(response.error || "IO force failed");
      }
      
      console.log(`✅ Forced ${formattedValue} on ${address}`);
    } catch (error) {
      console.error(`❌ Failed to force ${address}:`, error);
      throw error;
    }
  }

  /**
   * Release a forced I/O value
   */
  async unforceIo(address: string): Promise<void> {
    try {
      const response = await this.sendRequest("io.unforce", { address });
      
      if (!response.ok) {
        throw new Error(response.error || "IO unforce failed");
      }
      
      console.log(`✅ Released force on ${address}`);
    } catch (error) {
      console.error(`❌ Failed to unforce ${address}:`, error);
      throw error;
    }
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.socket !== null && !this.socket.destroyed;
  }
}

/**
 * Get runtime configuration from workspace
 */
export async function getRuntimeConfig(workspaceFolder?: vscode.WorkspaceFolder): Promise<RuntimeConfig | null> {
  // Try to get from VS Code settings if workspace is available
  if (workspaceFolder) {
    const config = vscode.workspace.getConfiguration("trust-lsp", workspaceFolder.uri);
    
    const endpoint = config.get<string>("runtime.controlEndpoint");
    const token = config.get<string>("runtime.controlAuthToken");

    if (endpoint) {
      return {
        controlEndpoint: endpoint,
        controlAuthToken: token,
      };
    }
  }

  // Default to Unix socket (works even without workspace)
  return {
    controlEndpoint: "unix:///tmp/trust-debug.sock",
  };
}
