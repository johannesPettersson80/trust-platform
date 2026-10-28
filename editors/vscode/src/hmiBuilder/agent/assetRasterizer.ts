/** Runs only in the isolated preview browser (serialized by render.ts). Assets remain image
 * resources, never active inline SVG/HTML. Decoded pixels are frozen before the page capture. */
export async function rasterizeProjectImages(svg: SVGSVGElement): Promise<void> {
  const checkResourceText = (text: string): void => {
    // CSS escapes and comments cannot disguise an import or external paint/font URL.
    const normalized = text
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(
        /\\([0-9a-f]{1,6})\s?|\\([^\r\n])/gi,
        (_match, hex: string | undefined, character: string | undefined) =>
          hex ? String.fromCodePoint(Math.min(parseInt(hex, 16), 0x10ffff)) : (character ?? ""),
      );
    if (/@import\b/i.test(normalized)) throw new Error("External SVG styles are not available in a project preview.");
    for (const match of normalized.matchAll(/url\(\s*(['"]?)(.*?)\1\s*\)/gi)) {
      const target = match[2].trim();
      if (!target.startsWith("#") && !/^data:image\/(png|jpeg);base64,/i.test(target))
        throw new Error("External SVG resources are not available in a project preview.");
    }
  };
  const animatedStyle = (style: CSSStyleDeclaration): boolean => {
    const name = style.getPropertyValue("animation-name");
    return Boolean(name && name.split(",").some((value) => value.trim() !== "none"));
  };
  const checkRules = (rules: CSSRuleList): void => {
    for (const rule of Array.from(rules)) {
      if (rule.type === CSSRule.KEYFRAMES_RULE || rule.type === CSSRule.IMPORT_RULE)
        throw new Error("Project SVG images must be static and self-contained.");
      if ("style" in rule && animatedStyle((rule as CSSStyleRule).style))
        throw new Error("Animated SVG images need a static preview asset.");
      if ("cssRules" in rule) checkRules((rule as CSSGroupingRule).cssRules);
    }
  };
  for (const element of Array.from(svg.querySelectorAll("image"))) {
    const source = element.getAttribute("href") ?? "";
    if (!/^data:image\/(png|jpeg|svg\+xml);base64,/.test(source))
      throw new Error("Project images must be embedded from the selected project's assets.");
    if (source.startsWith("data:image/svg+xml;")) {
      const xml = new TextDecoder().decode(Uint8Array.from(atob(source.split(",")[1]), (value) => value.charCodeAt(0)));
      const document = new DOMParser().parseFromString(xml, "image/svg+xml");
      if (document.querySelector("parsererror") || document.documentElement.localName !== "svg")
        throw new Error("Project SVG image does not parse.");
      for (const node of Array.from(document.querySelectorAll("*"))) {
        if (
          ["script", "foreignObject", "set", "discard"].includes(node.localName) ||
          node.localName.startsWith("animate")
        )
          throw new Error("Project SVG images must be static, without scripts or animation.");
        for (const attribute of Array.from(node.attributes)) {
          checkResourceText(attribute.value);
          if (
            attribute.localName === "href" &&
            !attribute.value.startsWith("#") &&
            !attribute.value.startsWith("data:")
          )
            throw new Error("External SVG resources are not available in a project preview.");
        }
        if (node.hasAttribute("style")) {
          const style = window.document.createElement("span").style;
          style.cssText = node.getAttribute("style")!;
          if (animatedStyle(style)) throw new Error("Animated SVG images need a static preview asset.");
        }
        if (node.localName === "style") {
          checkResourceText(node.textContent ?? "");
          const sheet = new CSSStyleSheet();
          sheet.replaceSync(node.textContent ?? "");
          checkRules(sheet.cssRules);
        }
      }
    }
    const image = new Image();
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error("Project image could not be decoded."));
      image.src = source;
    });
    if (
      !image.naturalWidth ||
      !image.naturalHeight ||
      image.naturalWidth > 4096 ||
      image.naturalHeight > 4096 ||
      image.naturalWidth * image.naturalHeight > 16000000
    )
      throw new Error("Project image dimensions exceed preview limits.");
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Image rasterizer unavailable.");
    context.drawImage(image, 0, 0);
    element.setAttribute("href", canvas.toDataURL("image/png"));
  }
}
