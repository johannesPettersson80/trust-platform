export function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

export function explanation(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
