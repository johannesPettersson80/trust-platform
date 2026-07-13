const RUNTIME_ADS_HEADER = "[runtime.ads]";
const REQUIRED_KEYS = [
  "enabled",
  "config_path",
  "worker_tick_interval_ms",
] as const;

type RuntimeAdsKey = (typeof REQUIRED_KEYS)[number];
type MultilineQuote = '"""' | "'''" | undefined;

export function enableRuntimeAdsToml(
  source: string,
  configPath: string,
): string {
  const newline = preferredNewline(source);
  const hadFinalNewline = /(?:\r\n|\n|\r)$/.test(source);
  const lines = source.split(/\r\n|\n|\r/);
  if (hadFinalNewline) {
    lines.pop();
  }
  const structural = structuralLineFlags(lines);
  const sectionStart = lines.findIndex(
    (line, index) => structural[index] && isRuntimeAdsHeader(line),
  );
  if (sectionStart < 0) {
    const prefix = source.trimEnd();
    const separator = prefix ? `${newline}${newline}` : "";
    return `${prefix}${separator}${RUNTIME_ADS_HEADER}${newline}enabled = true${newline}config_path = ${quoteTomlString(configPath)}${newline}worker_tick_interval_ms = 20${newline}`;
  }

  let sectionEnd = lines.length;
  for (let index = sectionStart + 1; index < lines.length; index += 1) {
    if (structural[index] && isTomlTableHeader(lines[index])) {
      sectionEnd = index;
      break;
    }
  }

  const rewritten = rewriteRuntimeAdsSection(
    lines.slice(sectionStart, sectionEnd),
    configPath,
  );
  const result = [
    ...lines.slice(0, sectionStart),
    ...rewritten,
    ...lines.slice(sectionEnd),
  ].join(newline);
  return hadFinalNewline ? `${result}${newline}` : result;
}

function rewriteRuntimeAdsSection(
  section: readonly string[],
  configPath: string,
): string[] {
  const seen = new Set<RuntimeAdsKey>();
  const rewritten = [section[0]];
  const structural = structuralLineFlags(section);
  for (let index = 1; index < section.length; index += 1) {
    const line = section[index];
    const key = structural[index] ? runtimeAdsAssignmentKey(line) : undefined;
    if (!key) {
      rewritten.push(line);
      continue;
    }
    if (seen.has(key)) {
      const comment = inlineTomlComment(line);
      if (comment) {
        rewritten.push(`${leadingWhitespace(line)}${comment}`);
      }
      continue;
    }
    seen.add(key);
    if (key === "worker_tick_interval_ms") {
      rewritten.push(line);
      continue;
    }
    rewritten.push(
      replaceAssignmentValue(
        line,
        key,
        key === "enabled" ? "true" : quoteTomlString(configPath),
      ),
    );
  }

  const missing = REQUIRED_KEYS.filter((key) => !seen.has(key)).map((key) => {
    switch (key) {
      case "enabled":
        return "enabled = true";
      case "config_path":
        return `config_path = ${quoteTomlString(configPath)}`;
      case "worker_tick_interval_ms":
        return "worker_tick_interval_ms = 20";
    }
  });
  let insertion = rewritten.length;
  while (insertion > 1 && rewritten[insertion - 1].trim() === "") {
    insertion -= 1;
  }
  rewritten.splice(insertion, 0, ...missing);
  return rewritten;
}

function runtimeAdsAssignmentKey(line: string): RuntimeAdsKey | undefined {
  const assignment = line.slice(0, inlineTomlCommentStart(line));
  const match = /^\s*([A-Za-z0-9_-]+)\s*=/.exec(assignment);
  const key = match?.[1] as RuntimeAdsKey | undefined;
  return key && REQUIRED_KEYS.includes(key) ? key : undefined;
}

function replaceAssignmentValue(
  line: string,
  key: RuntimeAdsKey,
  value: string,
): string {
  const commentStart = inlineTomlCommentStart(line);
  const assignment = line.slice(0, commentStart);
  const comment = line.slice(commentStart);
  const trailing = /\s*$/.exec(assignment)?.[0] ?? "";
  return `${leadingWhitespace(line)}${key} = ${value}${trailing}${comment}`;
}

function isRuntimeAdsHeader(line: string): boolean {
  return /^\[\s*runtime\s*\.\s*ads\s*\]$/.test(
    stripInlineTomlComment(line).trim(),
  );
}

function isTomlTableHeader(line: string): boolean {
  const content = stripInlineTomlComment(line).trim();
  return (
    /^\[[^\[\]]+\]$/.test(content) ||
    /^\[\[[^\[\]]+\]\]$/.test(content)
  );
}

function preferredNewline(source: string): string {
  const first = /\r\n|\n|\r/.exec(source)?.[0];
  return first ?? "\n";
}

function quoteTomlString(value: string): string {
  return JSON.stringify(value);
}

function leadingWhitespace(value: string): string {
  return /^\s*/.exec(value)?.[0] ?? "";
}

function inlineTomlComment(line: string): string {
  const start = inlineTomlCommentStart(line);
  return start < line.length ? line.slice(start).trimStart() : "";
}

function stripInlineTomlComment(line: string): string {
  return line.slice(0, inlineTomlCommentStart(line));
}

function inlineTomlCommentStart(line: string): number {
  let quote: '"' | "'" | undefined;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (quote === '"' && character === "\\") {
      index += 1;
      continue;
    }
    if (quote) {
      if (character === quote) {
        quote = undefined;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "#") {
      return index;
    }
  }
  return line.length;
}

function structuralLineFlags(lines: readonly string[]): boolean[] {
  let multiline: MultilineQuote;
  return lines.map((line) => {
    const structural = multiline === undefined;
    multiline = multilineQuoteAfterLine(line, multiline);
    return structural;
  });
}

function multilineQuoteAfterLine(
  line: string,
  initial: MultilineQuote,
): MultilineQuote {
  let multiline = initial;
  let quote: '"' | "'" | undefined;
  for (let index = 0; index < line.length; index += 1) {
    if (multiline) {
      const closing = line.indexOf(multiline, index);
      if (closing < 0) {
        return multiline;
      }
      index = closing + 2;
      multiline = undefined;
      continue;
    }
    const character = line[index];
    if (quote === '"' && character === "\\") {
      index += 1;
      continue;
    }
    if (quote) {
      if (character === quote) {
        quote = undefined;
      }
      continue;
    }
    if (character === "#") {
      break;
    }
    const triple = line.slice(index, index + 3);
    if (triple === '"""' || triple === "'''") {
      multiline = triple;
      index += 2;
    } else if (character === '"' || character === "'") {
      quote = character;
    }
  }
  return multiline;
}
