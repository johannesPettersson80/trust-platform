// The navigation icons truST ships (specification 39, §4.6): simple line drawings in the
// current text colour, 24 × 24.

const PATHS: Record<string, string> = {
  overview: "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
  process: "M3 7h9a4 4 0 0 1 4 4v9 M3 16h5 M16 4v3",
  trend: "M3 18l6-6 4 4 8-9 M3 21h18",
  alarm: "M6 16V11a6 6 0 0 1 12 0v5l2 2H4z M10 20a2 2 0 0 0 4 0",
  settings:
    "M12 8.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7z M12 2v3 M12 19v3 M2 12h3 M19 12h3 M4.9 4.9l2.1 2.1 M17 17l2.1 2.1 M4.9 19.1L7 17 M17 7l2.1-2.1",
  pump: "M12 4a8 8 0 1 0 0 16a8 8 0 1 0 0-16z M9 8l7 4-7 4z",
  valve: "M3 7l9 5 9-5v10l-9-5-9 5z M12 12V4 M9 4h6",
  tank: "M5 5a7 2 0 0 1 14 0v14a7 2 0 0 1-14 0z M5 13a7 2 0 0 0 14 0",
  // The language button's globe (§14.5).
  globe: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M3 12h18 M12 3a14 14 0 0 1 0 18 M12 3a14 14 0 0 0 0 18",
  motor: "M4 7h12v10H4z M16 10h4v4h-4 M7 4v3 M13 4v3",
  heat: "M12 3c3 4 5 6 5 10a5 5 0 0 1-10 0c0-2 1-4 2-5 0 2 1 3 2 3 0-3 0-5 1-8z",
  flow: "M3 9c3-3 6 3 9 0s6 3 9 0 M3 15c3-3 6 3 9 0s6 3 9 0",
  power: "M13 2L5 14h6l-1 8 8-12h-6z",
  list: "M8 6h13 M8 12h13 M8 18h13 M3 6h1 M3 12h1 M3 18h1",
  info: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 11v6 M12 7.5v.5",
};

/** An icon as an SVG element, or undefined for an unknown name. */
export function icon(document: Document, name: string | undefined): SVGSVGElement | undefined {
  const path = name === undefined ? undefined : PATHS[name];
  if (!path || !name) return undefined;
  const namespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(namespace, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "20");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("data-icon", name);
  const drawn = document.createElementNS(namespace, "path");
  drawn.setAttribute("d", path);
  drawn.setAttribute("fill", "none");
  drawn.setAttribute("stroke", "currentColor");
  drawn.setAttribute("stroke-width", "1.7");
  drawn.setAttribute("stroke-linecap", "round");
  drawn.setAttribute("stroke-linejoin", "round");
  svg.append(drawn);
  return svg;
}
