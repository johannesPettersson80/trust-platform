import { isDirectAddress, sanitizeIdentifier } from "./stNaming";

export interface DirectIoDeclaration {
  name: string;
  type: string;
  address: string;
}

const DIRECT_ADDRESS_TOKEN = /%[IQM][XBWDL]\d+(?:\.\d+)?/gi;

export function normalizeDirectIoAddress(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed || !isDirectAddress(trimmed)) {
    return undefined;
  }
  return trimmed.toUpperCase();
}

export function directIoSymbolName(address: string): string {
  const suffix = address.toUpperCase().replace(/^%/, "");
  return sanitizeIdentifier(`ld_io_${suffix}`, "ld_io_address");
}

export function typeForDirectIoAddress(address: string, fallback = "BOOL"): string {
  const size = normalizeDirectIoAddress(address)?.match(/^%[IQM]([XBWDL])/)?.[1];
  if (size === "X") {
    return "BOOL";
  }
  if (size === "B") {
    return "BYTE";
  }
  if (size === "W") {
    return "INT";
  }
  if (size === "D") {
    return "DINT";
  }
  if (size === "L") {
    return "LINT";
  }
  return fallback;
}

export function collectDirectIoDeclarationsFromText(text: string): DirectIoDeclaration[] {
  const declarations: DirectIoDeclaration[] = [];
  const emitted = new Set<string>();
  for (const match of text.matchAll(DIRECT_ADDRESS_TOKEN)) {
    const address = normalizeDirectIoAddress(match[0]);
    if (!address || emitted.has(address)) {
      continue;
    }
    declarations.push({
      name: directIoSymbolName(address),
      type: typeForDirectIoAddress(address),
      address,
    });
    emitted.add(address);
  }
  return declarations;
}

export function rewriteDirectIoReferencesInText(text: string): string {
  return text.replace(DIRECT_ADDRESS_TOKEN, (address) => {
    const normalized = normalizeDirectIoAddress(address);
    return normalized ? directIoSymbolName(normalized) : address;
  });
}

export function rewriteDirectIoInFunctionBlock(source: string): {
  source: string;
  declarations: DirectIoDeclaration[];
} {
  const declarations = collectDirectIoDeclarationsFromText(source);
  if (declarations.length === 0) {
    return { source, declarations };
  }

  const rewritten = rewriteDirectIoReferencesInText(source);
  const lines = rewritten.split(/\r?\n/);
  const functionBlockIndex = lines.findIndex((line) =>
    /^\s*FUNCTION_BLOCK\b/i.test(line)
  );
  if (functionBlockIndex < 0) {
    return { source: rewritten, declarations };
  }

  lines.splice(
    functionBlockIndex + 1,
    0,
    "VAR_EXTERNAL",
    ...declarations.map((declaration) => `  ${declaration.name} : ${declaration.type};`),
    "END_VAR",
    ""
  );
  return { source: lines.join("\n"), declarations };
}
