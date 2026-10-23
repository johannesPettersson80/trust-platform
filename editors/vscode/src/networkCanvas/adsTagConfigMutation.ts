export interface AdsTagConfigTarget {
  readonly host?: string;
  readonly targetNetId?: string;
  readonly port: number;
  readonly path: string;
}

export interface AdsTagConfigMutation {
  readonly text: string;
  readonly removedCount: number;
}

export function removeAdsTagFromToml(
  text: string,
  target: AdsTagConfigTarget,
): AdsTagConfigMutation {
  const connections = sectionStarts(text, /^\s*\[\[connections\]\]\s*(?:#.*)?$/gm);
  if (connections.length === 0) {
    return { text, removedCount: 0 };
  }

  let removedCount = 0;
  let output = text.slice(0, connections[0]);
  for (let index = 0; index < connections.length; index += 1) {
    const start = connections[index];
    const end = connections[index + 1] ?? text.length;
    const mutation = removeTagFromConnection(text.slice(start, end), target);
    output += mutation.text;
    removedCount += mutation.removedCount;
  }
  return { text: output, removedCount };
}

function removeTagFromConnection(
  block: string,
  target: AdsTagConfigTarget,
): AdsTagConfigMutation {
  const points = sectionStarts(
    block,
    /^\s*\[\[connections\.points\]\]\s*(?:#.*)?$/gm,
  );
  if (points.length === 0) {
    return { text: block, removedCount: 0 };
  }
  const header = block.slice(0, points[0]);
  if (!connectionMatches(header, target)) {
    return { text: block, removedCount: 0 };
  }

  const kept: string[] = [];
  let removedCount = 0;
  for (let index = 0; index < points.length; index += 1) {
    const start = points[index];
    const end = points[index + 1] ?? block.length;
    const point = block.slice(start, end);
    const path = tomlString(point, "symbol") ?? tomlString(point, "path");
    if (path === target.path) {
      removedCount += 1;
    } else {
      kept.push(point);
    }
  }
  if (removedCount === 0) {
    return { text: block, removedCount: 0 };
  }
  return {
    text: kept.length > 0 ? `${header}${kept.join("")}` : "",
    removedCount,
  };
}

function connectionMatches(
  header: string,
  target: AdsTagConfigTarget,
): boolean {
  const port = tomlInteger(header, "ams_port") ?? 851;
  if (port !== target.port) {
    return false;
  }
  const netId = tomlString(header, "target_net_id") ?? tomlString(header, "ams_net_id");
  const host = tomlString(header, "host") ?? tomlString(header, "ip");
  if (target.targetNetId && netId && target.targetNetId !== netId) {
    return false;
  }
  if (target.host && host && target.host !== host) {
    return false;
  }
  return Boolean(
    (target.targetNetId && netId) ||
    (target.host && host) ||
    (!target.targetNetId && !target.host),
  );
}

function sectionStarts(text: string, pattern: RegExp): number[] {
  return [...text.matchAll(pattern)].map((match) => match.index);
}

function tomlString(text: string, key: string): string | undefined {
  const match = new RegExp(`^\\s*${key}\\s*=\\s*(.+?)\\s*(?:#.*)?$`, "m").exec(text);
  const raw = match?.[1]?.trim();
  if (!raw) {
    return undefined;
  }
  if (raw.startsWith('"') && raw.endsWith('"')) {
    try {
      return JSON.parse(raw) as string;
    } catch {
      return undefined;
    }
  }
  if (raw.startsWith("'") && raw.endsWith("'")) {
    return raw.slice(1, -1);
  }
  return undefined;
}

function tomlInteger(text: string, key: string): number | undefined {
  const match = new RegExp(`^\\s*${key}\\s*=\\s*(\\d+)\\s*(?:#.*)?$`, "m").exec(text);
  if (!match?.[1]) {
    return undefined;
  }
  const value = Number(match[1]);
  return Number.isSafeInteger(value) ? value : undefined;
}
