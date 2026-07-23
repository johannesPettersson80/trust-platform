"use strict";

const fs = require("fs");
const path = require("path");
const { fileURLToPath } = require("url");
const { SourceMap } = require("module");

function convertRawProfile(profile, root) {
  if (!profile || !Array.isArray(profile.result)) {
    throw new Error("V8 coverage profile requires a result array");
  }
  const resolvedRoot = path.resolve(root);
  const extensionRoot = path.join(resolvedRoot, "editors", "vscode");
  const outputRoot = path.join(extensionRoot, "out");
  const coverage = {};
  for (const script of profile.result) {
    if (!script || typeof script.url !== "string" || !Array.isArray(script.functions)) {
      throw new Error("V8 coverage script has invalid shape");
    }
    const scriptPath = scriptFilePath(script.url);
    if (
      scriptPath === null ||
      !isWithin(scriptPath, outputRoot) ||
      isWithin(scriptPath, path.join(outputRoot, "test"))
    ) {
      continue;
    }
    const sourceMapPath = `${scriptPath}.map`;
    if (!fs.existsSync(scriptPath) || !fs.existsSync(sourceMapPath)) {
      continue;
    }
    const generated = fs.readFileSync(scriptPath, "utf8");
    const payload = JSON.parse(fs.readFileSync(sourceMapPath, "utf8"));
    const sourceMap = new SourceMap(payload);
    for (const fn of script.functions) {
      if (
        !fn ||
        typeof fn.functionName !== "string" ||
        fn.functionName.length === 0 ||
        !Array.isArray(fn.ranges) ||
        fn.ranges.length === 0
      ) {
        continue;
      }
      const range = fn.ranges[0];
      if (
        !range ||
        !Number.isInteger(range.startOffset) ||
        !Number.isInteger(range.endOffset) ||
        !Number.isInteger(range.count) ||
        range.count <= 0 ||
        range.startOffset < 0 ||
        range.endOffset <= range.startOffset ||
        range.endOffset > generated.length
      ) {
        continue;
      }
      const mapped = mappedRange({
        generated,
        sourceMap,
        payload,
        sourceMapPath,
        startOffset: range.startOffset,
        endOffset: range.endOffset,
        root: resolvedRoot,
      });
      if (mapped === null) {
        continue;
      }
      const record = coverage[mapped.path] || {
        path: mapped.path,
        statementMap: {},
        s: {},
        fnMap: {},
        f: {},
        branchMap: {},
        b: {},
      };
      const id = String(Object.keys(record.fnMap).length);
      record.fnMap[id] = {
        name: fn.functionName,
        decl: {
          start: mapped.start,
          end: mapped.start,
        },
        loc: {
          start: mapped.start,
          end: mapped.end,
        },
      };
      record.f[id] = range.count;
      coverage[mapped.path] = record;
    }
  }
  return Object.fromEntries(
    Object.entries(coverage).sort(([left], [right]) => left.localeCompare(right))
  );
}

function mappedRange({
  generated,
  sourceMap,
  payload,
  sourceMapPath,
  startOffset,
  endOffset,
  root,
}) {
  const start = findMappedEntry(generated, sourceMap, startOffset, endOffset, 1);
  const end = findMappedEntry(generated, sourceMap, endOffset - 1, startOffset, -1);
  if (
    !start ||
    !end ||
    typeof start.originalSource !== "string" ||
    typeof end.originalSource !== "string"
  ) {
    return null;
  }
  const startPath = originalPath(payload, sourceMapPath, start.originalSource);
  const endPath = originalPath(payload, sourceMapPath, end.originalSource);
  if (startPath !== endPath || !isWithin(startPath, root)) {
    return null;
  }
  const relative = path.relative(root, startPath).split(path.sep).join("/");
  if (
    !relative.startsWith("editors/vscode/src/") ||
    relative.startsWith("editors/vscode/src/test/")
  ) {
    return null;
  }
  const startPosition = {
    line: start.originalLine + 1,
    column: start.originalColumn,
  };
  let endPosition = {
    line: end.originalLine + 1,
    column: end.originalColumn + 1,
  };
  if (
    endPosition.line < startPosition.line ||
    (endPosition.line === startPosition.line &&
      endPosition.column <= startPosition.column)
  ) {
    endPosition = {
      line: startPosition.line,
      column: startPosition.column + 1,
    };
  }
  return { path: relative, start: startPosition, end: endPosition };
}

function findMappedEntry(generated, sourceMap, offset, bound, direction) {
  let current = offset;
  const limit = direction > 0 ? Math.min(bound, offset + 512) : Math.max(bound, offset - 512);
  while (direction > 0 ? current < limit : current >= limit) {
    const position = generatedPosition(generated, current);
    const entry = sourceMap.findEntry(position.line, position.column);
    if (entry && typeof entry.originalSource === "string") {
      return entry;
    }
    current += direction;
  }
  return null;
}

function generatedPosition(source, offset) {
  const prefix = source.slice(0, offset);
  const line = (prefix.match(/\n/g) || []).length;
  const lastNewline = prefix.lastIndexOf("\n");
  return {
    line,
    column: offset - (lastNewline < 0 ? 0 : lastNewline + 1),
  };
}

function originalPath(payload, sourceMapPath, originalSource) {
  if (originalSource.startsWith("file://")) {
    return path.resolve(fileURLToPath(originalSource));
  }
  const sourceRoot =
    payload && typeof payload.sourceRoot === "string" ? payload.sourceRoot : "";
  return path.resolve(path.dirname(sourceMapPath), sourceRoot, originalSource);
}

function scriptFilePath(url) {
  if (url.startsWith("file://")) {
    return path.resolve(fileURLToPath(url));
  }
  if (path.isAbsolute(url)) {
    return path.resolve(url);
  }
  return null;
}

function isWithin(candidate, directory) {
  const relative = path.relative(path.resolve(directory), path.resolve(candidate));
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function main() {
  if (process.argv.length !== 5) {
    throw new Error(
      "usage: node vscode_trace_convert.cjs <repo-root> <raw-profile.json> <coverage-final.json>"
    );
  }
  const root = path.resolve(process.argv[2]);
  const profile = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
  const converted = convertRawProfile(profile, root);
  fs.writeFileSync(
    process.argv[4],
    `${JSON.stringify(converted, null, 2)}\n`,
    "utf8"
  );
}

module.exports = {
  convertRawProfile,
  generatedPosition,
  isWithin,
};

if (require.main === module) {
  main();
}
