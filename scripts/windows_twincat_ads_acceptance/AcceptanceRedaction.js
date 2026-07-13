"use strict";

const AUTH_TOKEN_ASSIGNMENT =
  /((?:"|')?(?:(?:runtime\.control\.)?auth[_-]?token|control[_-]?auth[_-]?token)(?:"|')?\s*[:=]\s*)(?:"(?:\\.|[^"])*"|'(?:\\.|[^'])*'|[^\s,;}]+)/gi;

function safeError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return message
    .replace(AUTH_TOKEN_ASSIGNMENT, '$1"<redacted>"')
    .replace(/([?&]token=)[^&\s]+/gi, "$1<redacted>")
    .slice(0, 2000);
}

function serializeWithoutCredential(value, credential) {
  const credentials = (Array.isArray(credential) ? credential : [credential]).filter(
    (candidate) => typeof candidate === "string" && candidate.length > 0
  );
  let serialized = JSON.stringify(value, null, 2) + "\n";
  let credentialFound = false;
  for (const candidate of new Set(credentials)) {
    if (!serialized.includes(candidate)) continue;
    credentialFound = true;
    serialized = serialized.split(candidate).join("<redacted>");
  }
  return {
    serialized,
    credentialFound,
  };
}

module.exports = { safeError, serializeWithoutCredential };
