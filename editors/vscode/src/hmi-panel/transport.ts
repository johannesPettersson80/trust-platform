import { getTrustConfiguration } from "../configuration";
import * as net from "net";
import * as vscode from "vscode";

import { defaultRuntimeControlEndpoint } from "../runtimeDefaults";
import {
  parseControlEndpoint,
  type ParsedControlEndpoint,
} from "../runtimeControl";
import { ControlRequestHandler } from "./types";

const REQUEST_TIMEOUT_MS = 2000;
const DEFAULT_POLL_INTERVAL_MS = 500;

export function runtimeEndpointSettings(): {
  endpoint: string;
  authToken: string | undefined;
  pollIntervalMs: number;
} {
  const config = getTrustConfiguration();
  const endpointEnabled = config.get<boolean>("runtime.controlEndpointEnabled", true);
  const configured = endpointEnabled
    ? (config.get<string>("runtime.controlEndpoint") ?? "").trim()
    : "";
  const endpoint = configured || defaultRuntimeControlEndpoint();
  const auth = (config.get<string>("runtime.controlAuthToken") ?? "").trim();
  const poll = config.get<number>("hmi.pollIntervalMs", DEFAULT_POLL_INTERVAL_MS);
  const pollIntervalMs = Number.isFinite(poll)
    ? Math.max(100, Math.floor(poll))
    : DEFAULT_POLL_INTERVAL_MS;
  return {
    endpoint,
    authToken: auth.length > 0 ? auth : undefined,
    pollIntervalMs,
  };
}

export function createControlRequestSender(): ControlRequestHandler {
  let requestSeq = 1;

  return async function sendControlRequest(
    endpoint: string,
    authToken: string | undefined,
    requestType: string,
    params?: unknown
  ): Promise<unknown> {
    const parsed: ParsedControlEndpoint | undefined = parseControlEndpoint(endpoint);
    if (!parsed) {
      throw new Error(`invalid control endpoint '${endpoint}'`);
    }
    const id = requestSeq++;
    const requestEnvelope = {
      id,
      type: requestType,
      params,
      auth: authToken,
    };

    return await new Promise<unknown>((resolve, reject) => {
      let settled = false;
      let buffer = "";
      const socket =
        parsed.kind === "tcp"
          ? net.createConnection({ host: parsed.host, port: parsed.port })
          : net.createConnection({ path: parsed.path });

      const finish = (fn: () => void): void => {
        if (settled) {
          return;
        }
        settled = true;
        socket.destroy();
        fn();
      };

      socket.setTimeout(REQUEST_TIMEOUT_MS, () => {
        finish(() => reject(new Error("control request timeout")));
      });
      socket.once("error", (error) => {
        finish(() => reject(error));
      });
      socket.once("connect", () => {
        socket.write(`${JSON.stringify(requestEnvelope)}\n`);
      });
      socket.on("data", (chunk: Buffer | string) => {
        buffer += chunk.toString();
        let newlineIndex = buffer.indexOf("\n");
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex).trim();
          buffer = buffer.slice(newlineIndex + 1);
          if (line.length > 0) {
            try {
              const parsedLine = JSON.parse(line) as {
                ok?: boolean;
                result?: unknown;
                error?: string;
              };
              if (parsedLine.ok) {
                finish(() => resolve(parsedLine.result));
              } else {
                const errorText =
                  typeof parsedLine.error === "string" && parsedLine.error.length > 0
                    ? parsedLine.error
                    : "control request rejected";
                finish(() => reject(new Error(errorText)));
              }
              return;
            } catch (error) {
              finish(() => reject(error));
              return;
            }
          }
          newlineIndex = buffer.indexOf("\n");
        }
      });
    });
  };
}
