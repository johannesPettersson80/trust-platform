import MarkdownIt from "markdown-it";
import type { MarkdownIt as MarkdownItParser, MarkdownItOptions } from "markdown-it";
import katex from "katex";

export interface LessonMarkup {
  readonly html: string;
  readonly files: readonly string[];
}

const SAFE_FILE_EXTENSIONS = new Set([
  ".csv",
  ".json",
  ".md",
  ".st",
  ".svg",
  ".toml",
]);
const MAX_MATH_EXPANSIONS = 1000;

function escapeAttribute(value: string): string {
  return value.replace(/[&"<>]/g, (character) => ({
    "&": "&amp;",
    "\"": "&quot;",
    "<": "&lt;",
    ">": "&gt;",
  }[character] ?? character));
}

function safeProjectFileTarget(raw: string): string | undefined {
  if (!raw || raw.startsWith("#") || raw.startsWith("/") || raw.includes("\\") || raw.includes("%") || raw.includes("?") || raw.includes("#") || /[\u0000-\u001f]/.test(raw)) {
    return undefined;
  }
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(raw)) {
    return undefined;
  }
  const path = raw;
  if (!path || path.split("/").some((part) => part === "..")) {
    return undefined;
  }
  const extension = path.slice(path.lastIndexOf(".")).toLowerCase();
  if (!SAFE_FILE_EXTENSIONS.has(extension)) {
    return undefined;
  }
  return raw;
}

function renderMath(source: string, displayMode: boolean): string | undefined {
  try {
    return katex.renderToString(source, {
      displayMode,
      output: "mathml",
      throwOnError: true,
      trust: false,
      maxExpand: MAX_MATH_EXPANSIONS,
      strict: "ignore",
    });
  } catch {
    return undefined;
  }
}

function mathPlugin(md: MarkdownItParser): void {
  md.inline.ruler.before("escape", "lesson_inline_math", (state, silent) => {
    const start = state.pos;
    if (state.src.charCodeAt(start) !== 36 || state.src.charCodeAt(start + 1) === 36 || (start > 0 && state.src[start - 1] === "\\")) {
      return false;
    }
    let end = start + 1;
    while (end < state.posMax) {
      if (state.src.charCodeAt(end) === 36 && state.src.charCodeAt(end - 1) !== 92) break;
      end += 1;
    }
    if (end >= state.posMax || end === start + 1 || state.src.slice(start + 1, end).includes("\n")) return false;
    const rendered = renderMath(state.src.slice(start + 1, end), false);
    if (!rendered) return false;
    if (!silent) {
      const token = state.push("lesson_math", "span", 0);
      token.content = rendered;
      token.meta = { rendered };
    }
    state.pos = end + 1;
    return true;
  });

  md.block.ruler.before("fence", "lesson_display_math", (state, startLine, endLine, silent) => {
    const start = state.bMarks[startLine] + state.tShift[startLine];
    const first = state.src.slice(start, state.eMarks[startLine]).trim();
    if (first !== "$$") return false;
    let nextLine = startLine + 1;
    while (nextLine < endLine) {
      const lineStart = state.bMarks[nextLine] + state.tShift[nextLine];
      if (state.src.slice(lineStart, state.eMarks[nextLine]).trim() === "$$") break;
      nextLine += 1;
    }
    if (nextLine >= endLine) return false;
    if (silent) return true;
    const content = state.getLines(startLine + 1, nextLine, state.blkIndent, true).trim();
    const token = state.push("lesson_display_math", "div", 0);
    token.block = true;
    token.content = content;
    token.meta = { rendered: renderMath(content, true) };
    state.line = nextLine + 1;
    return true;
  });
}

export function renderLessonMarkup(source: string): LessonMarkup {
  const files: string[] = [];
  const options: MarkdownItOptions = { html: false, linkify: false, typographer: false };
  const md = new MarkdownIt(options);
  mathPlugin(md);
  md.renderer.rules.lesson_math = (tokens, index) => {
    const rendered = tokens[index].meta?.rendered;
    return typeof rendered === "string" ? rendered : md.utils.escapeHtml(tokens[index].content);
  };
  md.renderer.rules.lesson_display_math = (tokens, index) => {
    const rendered = tokens[index].meta?.rendered;
    return typeof rendered === "string" ? `<div class="lesson-math">${rendered}</div>` : md.utils.escapeHtml(tokens[index].content);
  };
  md.renderer.rules.image = (tokens, index) => md.utils.escapeHtml(tokens[index].content);
  md.renderer.rules.link_open = (tokens, index) => {
    const href = String(tokens[index].attrGet("href") ?? "");
    const target = safeProjectFileTarget(href);
    if (!target) return '<span class="lesson-unsafe-link">';
    files.push(target);
    return `<button type="button" class="lesson-file-link" data-lesson-file="${escapeAttribute(target)}">`;
  };
  md.renderer.rules.link_close = (tokens, index) => {
    for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
      if (tokens[cursor].type === "link_close") return "</span>";
      if (tokens[cursor].type === "link_open") {
        return safeProjectFileTarget(String(tokens[cursor].attrGet("href") ?? "")) ? "</button>" : "</span>";
      }
    }
    return "</span>";
  };
  return { html: md.render(source), files };
}
