import { type RuntimeTarget } from "../runtimeTarget";
import { sendRuntimeControlRequest } from "../runtimeControlClient";
import {
  shouldBlockSecretApply,
  validateSchemaValues,
  type CommApplyResponse,
  type CommFieldValidationError,
  type CommProtocolSchema,
  type CommSchemaResponse,
} from "./schemaForm";

export interface CommSetupMessage {
  readonly protocol?: unknown;
  readonly action?: unknown;
  readonly instanceId?: unknown;
  readonly params?: unknown;
}

export interface CommSetupResult {
  readonly protocol: string;
  readonly schema?: CommSchemaResponse;
  readonly applyResult: CommApplyResponse;
}

export interface CommProtocolError {
  readonly code?: string;
  readonly message?: string;
}

export interface CommTestControlResponse {
  readonly protocol: string;
  readonly supported: boolean;
  readonly ok: boolean;
  readonly detail: string;
  readonly error?: CommProtocolError | null;
  readonly field_errors?: Array<{ field: string; message: string }>;
}

export async function fetchCommSchema(
  runtime: RuntimeTarget,
  protocol?: string,
  timeoutMs = 2000
): Promise<CommSchemaResponse> {
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    throw new Error("Select a reachable online runtime before loading Communication setup.");
  }
  return await sendRuntimeControlRequest<CommSchemaResponse>(
    runtime.endpoint,
    runtime.authToken,
    "comm.schema",
    protocol ? { protocol } : undefined,
    { timeoutMs }
  );
}

export async function applyCommSetup(
  runtime: RuntimeTarget,
  message: CommSetupMessage,
  cachedSchema?: CommSchemaResponse
): Promise<CommSetupResult | undefined> {
  const protocol = normalizeProtocolId(message.protocol);
  if (!protocol) {
    return undefined;
  }
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        "Select a reachable online runtime before applying Communication setup."
      ),
    };
  }

  const schema = await schemaForProtocol(runtime, protocol, cachedSchema);
  if (!schema) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        "This runtime did not return a native setup schema for this protocol."
      ),
    };
  }

  const params = normalizeParams(message.params);
  const localErrors = validateSchemaValues(schema, params);
  if (localErrors.length > 0) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: {
        ...blockedApplyResult(protocol, "Fix the highlighted fields and try again."),
        field_errors: localErrors,
      },
    };
  }
  if (shouldBlockSecretApply(schema, params, runtime.credentialChannel)) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: secretBlockedResult(protocol),
    };
  }

  try {
    const applyResult = await sendRuntimeControlRequest<CommApplyResponse>(
      runtime.endpoint,
      runtime.authToken,
      "comm.apply",
      {
        protocol,
        action: normalizeApplyAction(message.action),
        instance_id: normalizedOptionalString(message.instanceId),
        params,
        credential_channel: runtime.credentialChannel,
      },
      { timeoutMs: 4000 }
    );
    return { protocol, schema: cachedSchema, applyResult };
  } catch (error) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        error instanceof Error ? error.message : String(error)
      ),
    };
  }
}

export async function testCommSetup(
  runtime: RuntimeTarget,
  message: CommSetupMessage,
  cachedSchema?: CommSchemaResponse
): Promise<CommSetupResult | undefined> {
  const protocol = normalizeProtocolId(message.protocol);
  if (!protocol) {
    return undefined;
  }
  if (runtime.status !== "online_reachable" || !runtime.endpoint) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        "Select a reachable online runtime before testing Communication setup."
      ),
    };
  }
  const schema = await schemaForProtocol(runtime, protocol, cachedSchema);
  if (!schema) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        "This runtime did not return a native setup schema for this protocol."
      ),
    };
  }
  const params = normalizeParams(message.params);
  const localErrors = validateSchemaValues(schema, params);
  if (localErrors.length > 0) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: {
        ...blockedApplyResult(protocol, "Fix the highlighted fields and try again."),
        field_errors: localErrors,
      },
    };
  }
  if (shouldBlockSecretApply(schema, params, runtime.credentialChannel)) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: secretBlockedResult(protocol),
    };
  }

  try {
    const result = await sendRuntimeControlRequest<CommTestControlResponse>(
      runtime.endpoint,
      runtime.authToken,
      "comm.test",
      {
        protocol,
        params,
        credential_channel: runtime.credentialChannel,
      },
      { timeoutMs: 4000 }
    );
    return {
      protocol,
      schema: cachedSchema,
      applyResult: {
        schema_version: 1,
        protocol,
        driver: "",
        action: "test",
        applied: false,
        lifecycle_effect: result.ok ? "test_ok" : "blocked",
        message: commTestMessage(protocol, result),
        field_errors: result.field_errors ?? [],
      },
    };
  } catch (error) {
    return {
      protocol,
      schema: cachedSchema,
      applyResult: blockedApplyResult(
        protocol,
        error instanceof Error ? error.message : String(error)
      ),
    };
  }
}

export function commTestMessage(protocol: string, result: CommTestControlResponse): string {
  if (result.ok) {
    return result.detail;
  }
  if (protocol !== "opcua_client") {
    return result.detail;
  }
  switch (result.error?.code) {
    case "endpoint_unreachable":
      return "OPC UA server is not reachable. Check the endpoint URL, port, and that the server is running.";
    case "auth_required":
      return "The OPC UA server needs valid credentials. Enter the username and password, then test again.";
    case "cert_untrusted":
      return "The OPC UA server certificate is not trusted. Use Browse nodes and choose Trust certificate if this is the expected server.";
    case "unsupported_security_profile":
      return "The OPC UA server does not offer the selected security policy and mode. Pick a supported endpoint and test again.";
    case "browse_denied":
      return "The OPC UA server denied access. Check the selected credentials and permissions.";
    default:
      return result.detail;
  }
}

export function clientErrorResult(
  protocolValue: unknown,
  fieldErrorsValue: unknown
): CommSetupResult | undefined {
  const protocol = normalizeProtocolId(protocolValue);
  if (!protocol) {
    return undefined;
  }
  return {
    protocol,
    applyResult: {
      ...blockedApplyResult(protocol, "Fix the highlighted fields and try again."),
      field_errors: normalizeFieldErrors(fieldErrorsValue),
    },
  };
}

export function normalizeProtocolId(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim().replace(/-/g, "_").toLowerCase();
  return normalized.length > 0 ? normalized : undefined;
}

export function blockedApplyResult(protocol: string, message: string): CommApplyResponse {
  return {
    schema_version: 1,
    protocol,
    driver: "",
    action: "upsert",
    applied: false,
    lifecycle_effect: "blocked",
    message,
    field_errors: [],
  };
}

async function schemaForProtocol(
  runtime: RuntimeTarget,
  protocol: string,
  cachedSchema: CommSchemaResponse | undefined
): Promise<CommProtocolSchema | undefined> {
  const fromState = cachedSchema?.protocols.find((entry) => entry.id === protocol);
  if (fromState) {
    return fromState;
  }
  return (await fetchCommSchema(runtime, protocol)).protocols.find(
    (entry) => entry.id === protocol
  );
}

function secretBlockedResult(protocol: string): CommApplyResponse {
  return {
    ...blockedApplyResult(
      protocol,
      "Secret fields cannot be sent over this runtime control channel. Use a same-host runtime endpoint or apply a generated snippet on the runtime host."
    ),
    field_errors: [
      {
        field: "password",
        message: "Secret fields cannot be sent over an untrusted remote plain-TCP channel.",
      },
    ],
  };
}

function normalizeParams(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? { ...(value as Record<string, unknown>) }
    : {};
}

function normalizeApplyAction(value: unknown): string {
  if (typeof value !== "string") {
    return "upsert";
  }
  const normalized = value.trim().toLowerCase();
  return normalized.length > 0 ? normalized : "upsert";
}

function normalizedOptionalString(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : undefined;
}

function normalizeFieldErrors(value: unknown): CommFieldValidationError[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.flatMap((entry) => {
    if (
      typeof entry === "object" &&
      entry !== null &&
      typeof (entry as { field?: unknown }).field === "string" &&
      typeof (entry as { message?: unknown }).message === "string"
    ) {
      return [
        {
          field: (entry as { field: string }).field,
          message: (entry as { message: string }).message,
        },
      ];
    }
    return [];
  });
}
