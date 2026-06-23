// Pure token-resolution rule (NO vscode import) so it's unit-testable standalone. §0.6.8:
// SecretStorage value wins; the legacy plaintext setting is only a fallback.
export function pickAuthToken(
  secretValue: string | undefined,
  legacySetting: string | undefined
): string | undefined {
  const secret = (secretValue ?? "").trim();
  if (secret.length > 0) {
    return secret;
  }
  const legacy = (legacySetting ?? "").trim();
  return legacy.length > 0 ? legacy : undefined;
}
