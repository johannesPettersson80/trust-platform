import test from "node:test";
import assert from "node:assert/strict";

import { buildRequestPositions } from "./lsp-position-resolver.js";

class MockModel {
  constructor(text) {
    this.lines = String(text).split(/\r?\n/);
  }

  getLineCount() {
    return this.lines.length;
  }

  getLineMaxColumn(lineNumber) {
    const line = this.lines[lineNumber - 1] || "";
    return line.length + 1;
  }

  getLineContent(lineNumber) {
    return this.lines[lineNumber - 1] || "";
  }

  getWordAtPosition(position) {
    const line = this.getLineContent(position.lineNumber);
    const column = Math.max(1, Number(position.column || 1));
    const idx = column - 1;
    if (idx < 0 || idx >= line.length) return null;
    if (!isIdentChar(line[idx])) return null;

    let start = idx;
    while (start > 0 && isIdentChar(line[start - 1])) {
      start -= 1;
    }
    let end = idx + 1;
    while (end < line.length && isIdentChar(line[end])) {
      end += 1;
    }
    return {
      word: line.slice(start, end),
      startColumn: start + 1,
      endColumn: end + 1,
    };
  }
}

function isIdentChar(ch) {
  return /[A-Za-z0-9_]/.test(ch);
}

function columns(points) {
  return points.map((point) => point.column);
}

test("buildRequestPositions recovers symbol before ':' declaration punctuation", () => {
  const line = "Enable : BOOL;";
  const model = new MockModel(line);
  const colonColumn = line.indexOf(":") + 1;

  const points = buildRequestPositions(model, { lineNumber: 1, column: colonColumn });
  const cols = columns(points);

  const enableStart = line.indexOf("Enable") + 1;
  const enableEnd = enableStart + "Enable".length - 1;
  assert.ok(cols.includes(enableStart), "expected identifier start candidate");
  assert.ok(cols.includes(enableEnd), "expected identifier end candidate");
});

test("buildRequestPositions recovers enum type token when anchor is on qualified value", () => {
  const line = "Status.State := E_PumpState#Idle;";
  const model = new MockModel(line);
  const idleColumn = line.indexOf("Idle") + 3; // inside Idle token

  const points = buildRequestPositions(model, { lineNumber: 1, column: idleColumn });
  const cols = columns(points);

  const enumTypeStart = line.indexOf("E_PumpState") + 1;
  const enumTypeEnd = enumTypeStart + "E_PumpState".length - 1;
  assert.ok(cols.includes(enumTypeStart), "expected qualified type start candidate");
  assert.ok(cols.includes(enumTypeEnd), "expected qualified type end candidate");
});

test("buildRequestPositions recovers both sides of field/member access delimiters", () => {
  const line = "Status.State := Pump.Status;";
  const model = new MockModel(line);
  const dotColumn = line.indexOf(".") + 1;

  const points = buildRequestPositions(model, { lineNumber: 1, column: dotColumn });
  const cols = columns(points);

  const leftStart = line.indexOf("Status") + 1;
  const rightStart = line.indexOf("State") + 1;
  assert.ok(cols.includes(leftStart), "expected left token candidate");
  assert.ok(cols.includes(rightStart), "expected right token candidate");
});

test("buildRequestPositions clamps out-of-range input and keeps unique points", () => {
  const model = new MockModel("ramp := ramp + 0.2;");
  const points = buildRequestPositions(model, { lineNumber: 99, column: -42 });

  assert.ok(points.length > 0, "expected non-empty candidate list");
  assert.equal(points[0].lineNumber, 1, "line number should clamp to valid range");
  assert.equal(points[0].column, 1, "column should clamp to valid range");

  const keys = new Set(points.map((point) => `${point.lineNumber}:${point.column}`));
  assert.equal(keys.size, points.length, "expected deduplicated candidate positions");
});
