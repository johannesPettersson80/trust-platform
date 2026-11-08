// The acceptance runner's retries and its words for a failure (specification 22, "Acceptance journeys on the packaged
// extension"): test tooling, apart from the extension.

/** The value of `attempt`, tried up to `tries` times. After a failure that is not the last, the failure is logged and
 * `pause` waits before the next try; the last failure is thrown. */
export async function withRetries<T>(
  attempt: () => Promise<T>,
  tries: number,
  pause: (failedAttempt: number) => Promise<void>,
  log: (line: string) => void,
): Promise<T> {
  for (let tried = 1; ; tried += 1) {
    try {
      return await attempt();
    } catch (error) {
      if (tried >= tries) throw error;
      log(`attempt ${tried} of ${tries} failed: ${describeError(error)}`);
      await pause(tried);
    }
  }
}

/** A failure's own words: an error's message, else its stack or name, and any other value by its JSON, else its text,
 * so that an empty message hides nothing. */
export function describeError(error: unknown): string {
  if (error instanceof Error) return error.message || error.stack || error.name;
  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}
