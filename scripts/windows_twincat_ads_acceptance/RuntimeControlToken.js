"use strict";

function runtimeControlToken(source) {
  const text = String(source || "");
  const table = /^\s*\[runtime\.control\]\s*(?:#.*)?\r?\n([\s\S]*?)(?=^\s*\[|(?![\s\S]))/m.exec(
    text
  );
  if (table) {
    const token = /^\s*auth_token\s*=\s*["']([^"']*)["']/m.exec(table[1]);
    return token ? token[1] : undefined;
  }
  const dotted = /^\s*runtime\.control\.auth_token\s*=\s*["']([^"']*)["']/m.exec(
    text
  );
  return dotted ? dotted[1] : undefined;
}

module.exports = { runtimeControlToken };
