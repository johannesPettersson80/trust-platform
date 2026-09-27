/** Shared styling for authored lesson previews inside the existing gallery. */
export const LESSON_MARKUP_STYLES = `
.lesson-markup{color:var(--trust-text);font-size:12px;line-height:1.5;min-width:0;overflow-wrap:anywhere}
.lesson-markup p,.card .lesson-markup p{color:inherit;font:inherit;margin:0 0 .65em}.lesson-markup p:last-child{margin-bottom:0}
.lesson-markup ul,.lesson-markup ol{margin:.5em 0;padding-left:1.6em}.lesson-markup li{margin:.35em 0}
.lesson-markup code{font-family:var(--vscode-editor-font-family);color:var(--vscode-textPreformat-foreground,var(--trust-text));background:var(--vscode-textPreformat-background);padding:1px 3px;border-radius:2px}
.lesson-markup pre{overflow:auto;padding:8px;background:var(--vscode-textCodeBlock-background)}.lesson-markup pre code{padding:0}
.lesson-markup table{border-collapse:collapse;width:100%;margin:.6em 0}.lesson-markup th,.lesson-markup td{padding:5px 8px;border:1px solid var(--trust-border);text-align:left}
.lesson-markup math{font-size:1.08em}.lesson-markup .lesson-file-link{appearance:none;border:0;padding:0;background:none;color:var(--vscode-textLink-foreground);font:inherit;text-decoration:underline;cursor:pointer}
.lesson-markup .lesson-file-link:focus-visible{outline:1px solid var(--vscode-focusBorder);outline-offset:2px}
.lesson-preview-error{color:var(--vscode-errorForeground)!important}
`;

/** Self-contained so the gallery can embed the same tested function in its webview. */
export function appendLessonMarkup(parent: HTMLElement, html: string | undefined, fallback: string, onFile: (target: string) => void): void {
  const doc = parent.ownerDocument;
  const block = doc.createElement("div");
  block.className = "lesson-markup";
  if (typeof html === "string") {
    // The host generates this with HTML-disabled Markdown and untrusted-mode
    // math. Parse into an inert template and remove active content before use.
    const template = doc.createElement("template");
    template.innerHTML = html;
    for (const element of template.content.querySelectorAll("script,style,link,img,iframe,object,embed,form,input,textarea,select")) element.remove();
    for (const element of template.content.querySelectorAll("*")) {
      for (const attribute of [...element.attributes]) {
        if (/^on/i.test(attribute.name) || ["href", "xlink:href", "src", "srcset", "srcdoc", "action", "formaction"].includes(attribute.name)) {
          element.removeAttribute(attribute.name);
        }
        if (attribute.name === "style" && !/^text-align:\s*(left|right|center);?$/.test(attribute.value)) element.removeAttribute("style");
      }
    }
    block.appendChild(template.content);
    for (const button of block.querySelectorAll<HTMLButtonElement>("button[data-lesson-file]")) {
      const target = button.dataset.lessonFile!;
      button.type = "button";
      button.title = `Preview bundled ${target} (read only)`;
      button.setAttribute("aria-label", `Preview bundled ${button.textContent?.trim() || target}, read only`);
      button.addEventListener("click", () => onFile(target));
    }
  } else {
    block.textContent = fallback;
  }
  parent.appendChild(block);
}
