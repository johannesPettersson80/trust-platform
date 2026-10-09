/** Specification 43 section 4: shared, transport-independent authoring request boundary. */
export type AuthoringErrorCode =
  | "INVALID_ARGUMENT"
  | "UNSUPPORTED_VERSION"
  | "PROJECT_UNAVAILABLE"
  | "COMPILATION_FAILED"
  | "CONFLICT"
  | "CANCELLED"
  | "VALIDATION_FAILED"
  | "RENDER_FAILED"
  | "NOT_CONNECTED"
  | "UNSUPPORTED_CONTEXT"
  | "LIMIT_EXCEEDED"
  | "EXPIRED_DRAFT";
export type AuthoringResult =
  | { ok: true; data: unknown; revisions: Record<string, string>; warnings: string[] }
  | { ok: false; code: AuthoringErrorCode; message: string; retryable: boolean; details?: unknown };
export interface AuthoringSession {
  workspaceId: string;
  signal: AbortSignal;
}
export type AuthoringHandler = (input: Record<string, unknown>, session: AuthoringSession) => Promise<AuthoringResult>;

const MAX_REQUEST_BYTES = 8 * 1024 * 1024;
const ENVELOPE_KEYS = new Set(["contractVersion", "workspaceId", "input"]);
function record(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
function failure(code: AuthoringErrorCode, message: string): AuthoringResult {
  return { ok: false, code, message, retryable: false };
}

/** Validate before invoking any service. Per-tool schemas and workspace URI checks remain
 * the responsibility of the selected operation; this envelope never grants path authority. */
export async function invokeAuthoring(
  request: unknown,
  session: AuthoringSession,
  handler: AuthoringHandler,
): Promise<AuthoringResult> {
  if (session.signal.aborted) return failure("CANCELLED", "The authoring request was cancelled.");
  if (
    !record(request) ||
    Object.keys(request).some((key) => !ENVELOPE_KEYS.has(key)) ||
    !Number.isInteger(request.contractVersion) ||
    typeof request.workspaceId !== "string" ||
    !request.workspaceId.trim() ||
    !record(request.input)
  ) {
    return failure("INVALID_ARGUMENT", "Provide contractVersion, workspaceId and an input object only.");
  }
  if (request.contractVersion !== 1)
    return failure("UNSUPPORTED_VERSION", "This authoring service supports contractVersion 1.");
  if (!session.workspaceId || request.workspaceId !== session.workspaceId) {
    return failure("PROJECT_UNAVAILABLE", "The request does not belong to this connected workspace.");
  }
  let encoded: string;
  try {
    encoded = JSON.stringify(request, (_key, value: unknown) => {
      if (
        value === undefined ||
        typeof value === "function" ||
        typeof value === "symbol" ||
        typeof value === "bigint" ||
        (typeof value === "number" && !Number.isFinite(value))
      ) {
        throw new Error("Non-JSON input");
      }
      return value;
    });
  } catch {
    return failure("INVALID_ARGUMENT", "Authoring input must be finite, acyclic JSON data.");
  }
  if (Buffer.byteLength(encoded, "utf8") > MAX_REQUEST_BYTES) {
    return failure("LIMIT_EXCEEDED", "The request exceeds 8 MiB; submit a smaller batch.");
  }
  return handler(request.input, session);
}
