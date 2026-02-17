const IDENTIFIER_RE = /[A-Za-z_][A-Za-z0-9_]*/g;
const QUALIFIER_CHARS = new Set(["#", "."]);

export function clamp(value, min, max) {
  const numeric = Number.isFinite(value) ? Number(value) : min;
  return Math.min(max, Math.max(min, numeric));
}

function getLineCount(model) {
  if (!model || typeof model.getLineCount !== "function") return 1;
  return Math.max(1, Number(model.getLineCount()) || 1);
}

function getLineMaxColumn(model, lineNumber) {
  if (!model || typeof model.getLineMaxColumn !== "function") return 1;
  return Math.max(1, Number(model.getLineMaxColumn(lineNumber)) || 1);
}

function getLineContent(model, lineNumber) {
  if (!model || typeof model.getLineContent !== "function") return "";
  const content = model.getLineContent(lineNumber);
  return typeof content === "string" ? content : "";
}

function collectIdentifiers(lineText) {
  const tokens = [];
  IDENTIFIER_RE.lastIndex = 0;
  let match = IDENTIFIER_RE.exec(lineText);
  while (match) {
    const startColumn = match.index + 1;
    const endColumn = startColumn + match[0].length;
    tokens.push({
      word: match[0],
      startColumn,
      endColumn,
    });
    match = IDENTIFIER_RE.exec(lineText);
  }
  IDENTIFIER_RE.lastIndex = 0;
  return tokens;
}

function wordAtPosition(model, lineNumber, column) {
  if (!model || typeof model.getWordAtPosition !== "function") return null;
  return model.getWordAtPosition({ lineNumber, column }) || null;
}

function pushWordEdges(push, word) {
  if (!word) return;
  if (!Number.isFinite(word.startColumn) || !Number.isFinite(word.endColumn)) return;
  push(word.startColumn);
  push(Math.max(word.startColumn, word.endColumn - 1));
}

function tokenContaining(tokens, column) {
  return (
    tokens.find((token) => column >= token.startColumn && column < token.endColumn) || null
  );
}

function nearestToken(tokens, column, maxDistance = 4) {
  let best = null;
  for (const token of tokens) {
    const distance =
      column < token.startColumn
        ? token.startColumn - column
        : column >= token.endColumn
          ? column - (token.endColumn - 1)
          : 0;
    if (distance > maxDistance) continue;
    if (!best || distance < best.distance) {
      best = { token, distance };
    }
  }
  return best ? best.token : null;
}

function pushQualifiedNeighbors(lineText, tokens, anchorColumn, push) {
  const maxColumn = Math.max(1, lineText.length + 1);
  const startColumn = Math.max(1, anchorColumn - 8);
  const endColumn = Math.min(maxColumn, anchorColumn + 8);

  for (let column = startColumn; column <= endColumn; column++) {
    const idx = column - 1;
    const ch = lineText[idx];
    if (!QUALIFIER_CHARS.has(ch)) continue;

    const left = tokens.find((token) => token.endColumn === column);
    const right = tokens.find((token) => token.startColumn === column + 1);
    pushWordEdges(push, left);
    pushWordEdges(push, right);
  }

  const containing = tokenContaining(tokens, anchorColumn) || tokenContaining(tokens, anchorColumn - 1);
  if (!containing) return;

  const beforeIdx = containing.startColumn - 2;
  if (beforeIdx >= 0 && QUALIFIER_CHARS.has(lineText[beforeIdx])) {
    const left = tokens.find((token) => token.endColumn === containing.startColumn - 1);
    pushWordEdges(push, left);
  }

  const afterIdx = containing.endColumn - 1;
  if (afterIdx < lineText.length && QUALIFIER_CHARS.has(lineText[afterIdx])) {
    const right = tokens.find((token) => token.startColumn === containing.endColumn + 1);
    pushWordEdges(push, right);
  }
}

function pushNearbyColumns(push, anchorColumn, radius) {
  for (let delta = -radius; delta <= radius; delta++) {
    push(anchorColumn + delta);
  }
}

export function buildRequestPositions(model, position, options = {}) {
  const lineNumber = clamp(
    Number(position?.lineNumber || 1),
    1,
    getLineCount(model),
  );
  const maxColumn = getLineMaxColumn(model, lineNumber);
  const anchorColumn = clamp(Number(position?.column || 1), 1, maxColumn);
  const radius = clamp(Number(options.radius || 3), 1, 8);

  const points = [];
  const seen = new Set();
  const push = (column) => {
    const clampedColumn = clamp(Number(column || 1), 1, maxColumn);
    const key = `${lineNumber}:${clampedColumn}`;
    if (seen.has(key)) return;
    seen.add(key);
    points.push({ lineNumber, column: clampedColumn });
  };

  pushNearbyColumns(push, anchorColumn, radius);

  const lineText = getLineContent(model, lineNumber);
  const tokens = collectIdentifiers(lineText);
  const nearest = nearestToken(tokens, anchorColumn, 5);
  pushWordEdges(push, nearest);

  const probeColumns = [
    anchorColumn,
    anchorColumn - 1,
    anchorColumn + 1,
    anchorColumn - 2,
    anchorColumn + 2,
  ];
  for (const probeColumn of probeColumns) {
    const probe = clamp(probeColumn, 1, maxColumn);
    const word =
      wordAtPosition(model, lineNumber, probe)
      || wordAtPosition(model, lineNumber, clamp(probe - 1, 1, maxColumn))
      || wordAtPosition(model, lineNumber, clamp(probe + 1, 1, maxColumn));
    pushWordEdges(push, word);
  }

  pushQualifiedNeighbors(lineText, tokens, anchorColumn, push);

  return points;
}
