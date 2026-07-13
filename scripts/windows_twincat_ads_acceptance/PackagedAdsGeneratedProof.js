"use strict";

function readGeneratedProof(source, mapping, expectedType) {
  if (!mapping || !expectedType) {
    return { typedLocal: false, qualityMapping: false };
  }
  const code = withoutComments(source);
  const typedLocal = count(
    code,
    new RegExp(
      `^\\s*${escapeRegExp(mapping.var)}\\s*:\\s*${escapeRegExp(expectedType)}\\s*;\\s*$`,
      "gm"
    )
  ) === 1;
  const qualityType = count(
    code,
    /^\s*ADS_QUALITY\s*:\s*\(\s*Stale\s*:=\s*0\s*,\s*Good\s*:=\s*1\s*,\s*Error\s*:=\s*2\s*\)\s*;\s*$/gm
  ) === 1;
  const qualityVariable = count(
    code,
    new RegExp(
      `^\\s*${escapeRegExp(mapping.var)}_quality\\s*:\\s*ADS_QUALITY\\s*:=\\s*Stale\\s*;\\s*$`,
      "gm"
    )
  ) === 1;
  return { typedLocal, qualityMapping: qualityType && qualityVariable };
}

function withoutComments(source) {
  let output = "";
  let blockDepth = 0;
  let lineComment = false;
  for (let index = 0; index < source.length; index += 1) {
    const pair = source.slice(index, index + 2);
    if (!lineComment && pair === "(*") {
      blockDepth += 1;
      output += "  ";
      index += 1;
    } else if (blockDepth > 0 && pair === "*)") {
      blockDepth -= 1;
      output += "  ";
      index += 1;
    } else if (blockDepth === 0 && !lineComment && pair === "//") {
      lineComment = true;
      output += "  ";
      index += 1;
    } else if (source[index] === "\n") {
      lineComment = false;
      output += "\n";
    } else {
      output += blockDepth > 0 || lineComment ? " " : source[index];
    }
  }
  return output;
}

function count(source, pattern) {
  return (source.match(pattern) || []).length;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

module.exports = { readGeneratedProof };
