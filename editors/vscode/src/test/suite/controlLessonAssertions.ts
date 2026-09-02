export const canonicalLessonReadmeHeadings = [
  "The problem",
  "What we need",
  "The new idea",
  "Prediction",
  "Implementation",
  "Experiment in Control Studio",
  "Evidence and decision",
  "Conclusion",
  "Reason for the next lesson",
] as const;

/** Normalize rendered Markdown mathematics for terminology assertions. */
export function pedagogicalMarkdownText(markdown: string): string {
  const normalized = markdown
    .replace(/[*`$]/g, "")
    .replace(/\{\}\s*\^\{?\\circ\}?\s*\\mathrm\{C\}/g, " °C")
    .replace(/\^\{?\\circ\}?\s*\\mathrm\{C\}/g, " °C")
    .replace(/\\begin\{(?:aligned|align|array)\}/g, "")
    .replace(/\\end\{(?:aligned|align|array)\}/g, "")
    .replace(/\\begin\{bmatrix\}/g, "[[")
    .replace(/\\end\{bmatrix\}/g, "]]" )
    .replace(/\\\\/g, "], [")
    .replace(/&/g, ", ")
    .replace(/\\(?:left|right)/g, "")
    .replace(/\\(?:lVert|rVert|Vert)/g, "||")
    .replace(/\\mid/g, "|")
    .replace(/\\(?:lvert|rvert)/g, "|")
    .replace(/\\(?:,|;|!)/g, " ")
    .replace(/\\(?:mathrm|operatorname|text|mathbf)\{([^}]*)\}/g, "$1")
    .replace(/\\(?:d?frac)\{([^{}]+)\}\{([^{}]+)\}/g, "$1 / $2")
    .replace(/\\sqrt\{([^{}]+)\}/g, "sqrt($1)")
    .replace(/_\{([^}]*)\}/g, "_$1")
    .replace(/\^\{([^}]*)\}/g, "^$1")
    .replace(/\\dot\{?([A-Za-z]+)\}?/g, "$1_dot")
    .replace(/\\Delta\s*([A-Za-z]+)/g, "delta_$1")
    .replace(/\\theta/g, "theta")
    .replace(/\\omega/g, "omega")
    .replace(/\\tau/g, "tau")
    .replace(/\\epsilon/g, "epsilon")
    .replace(/\\varepsilon/g, "epsilon")
    .replace(/\\gamma/g, "gamma")
    .replace(/\\rho/g, "rho")
    .replace(/\\nu/g, "nu")
    .replace(/\\alpha/g, "alpha")
    .replace(/\\kappa/g, "kappa")
    .replace(/\\sigma/g, "sigma")
    .replace(/\\Lambda/g, "Lambda")
    .replace(/\\hat\{?([A-Za-z]+)\}?/g, "$1_hat")
    .replace(/\\exp/g, "exp")
    .replace(/\\(?:max|min)/g, (name) => name.slice(1))
    .replace(/\\ast/g, "*")
    .replace(/\\log_10/g, "log10")
    .replace(/\\(?:times|cdot)/g, "*")
    .replace(/\\(?:leq|le)/g, "<=")
    .replace(/\\(?:geq|ge)/g, ">=")
    .replace(/\\infty/g, "infinity")
    .replace(/\\sum/g, "sum")
    .replace(/_([A-Za-z]+)\s+([A-Za-z]+)/g, "_$1_$2")
    .replace(/_([0-9])/g, "$1")
    .replace(/T_([idtf])/g, "T$1")
    .replace(/K_([pid])/g, "K$1")
    .replace(/[{}\\]/g, "")
    .replace(/\s+/g, " ");
  const compactOperators = normalized.replace(/\s*([=+*/-])\s*/g, "$1");
  const spacedOperators = normalized.replace(/\s*([=+*/-])\s*/g, " $1 ");
  const implicitProducts = spacedOperators.replace(/\s+\*\s+/g, " ");
  const equationStyle = normalized
    .replace(/\s*=\s*/g, " = ")
    .replace(/\s*\*\s*/g, "*")
    .replace(/\s*([+/-])\s*/g, " $1 ")
    .replace(/\s+/g, " ");
  const legacyAliases = normalized
    .replace(/delta_t/g, "dt")
    .replace(/delta_u_([A-Z])/g, "delta_u$1")
    .replace(/delta_MV_([A-Z])/g, "Delta MV_$1")
    .replace(/\bt_([a-z]+),([a-z]+)\b/g, "t_$1_$2")
    .replace(/\bf_s\b/g, "fs")
    .replace(/\br_k\b/g, "r[k]")
    .replace(/x_k\|k-1/g, "x_prior")
    .replace(/x_k-1\|k-1/g, "x_post_previous")
    .replace(/x_k\|k/g, "x_post")
    .replace(/P_k\|k-1/g, "P_prior")
    .replace(/P_k-1\|k-1/g, "P_post_previous")
    .replace(/P_k\|k/g, "P_post")
    .replace(/u_k-1/g, "u_previous")
    .replace(/Q_in/g, "Qin")
    .replace(/\*/g, "×");
  const legacyCompact = legacyAliases.replace(/\s*([=+*/-])\s*/g, "$1");
  const legacyEquationStyle = legacyAliases
    .replace(/\s*=\s*/g, " = ")
    .replace(/\s*\*\s*/g, "*")
    .replace(/\s*([+/-])\s*/g, " $1 ")
    .replace(/\s+/g, " ");
  // Keep both explicit and compact subscript spellings available to terminology
  // assertions. The README remains authoritative LaTeX; this helper only makes
  // tests insensitive to whether a symbol is rendered as K_u or legacy Ku.
  const compactSubscripts = normalized.replace(/_([A-Za-z0-9]+)/g, "$1");
  return `${normalized} ${compactOperators} ${spacedOperators} ${implicitProducts} ${equationStyle} ${legacyAliases} ${legacyCompact} ${legacyEquationStyle} ${compactSubscripts}`;
}

/** Count authored lesson prose without charging the required click-by-click workflow. */
export function pedagogicalCoreWordCount(markdown: string): number {
  const workflowStart = markdown.indexOf("### Control Studio actions");
  const nextHeading = workflowStart >= 0
    ? markdown.indexOf("\n## ", workflowStart + "### Control Studio actions".length)
    : -1;
  const core = workflowStart >= 0 && nextHeading > workflowStart
    ? `${markdown.slice(0, workflowStart)}${markdown.slice(nextHeading + 1)}`
    : markdown;
  const prose = core.replace(/^#{1,6}\s+.+$/gm, "");
  return prose.trim().split(/\s+/).filter(Boolean).length;
}
