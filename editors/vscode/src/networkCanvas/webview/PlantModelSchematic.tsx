import React from "react";

const SUMMARIES: Readonly<Record<string, string>> = {
  static_gain: "An input step produces an immediate scaled output step.",
  first_order: "A step approaches its new level gradually.",
  first_order_dead_time: "A step waits, then approaches its new level gradually.",
  integrator: "A sustained input difference keeps moving the output as a ramp.",
  second_order: "The response may overshoot or ring; damping controls how it settles.",
  piecewise_linear: "Connected points define an immediate nonlinear input-output curve.",
  state_space: "Inputs drive coupled internal states, which produce one or more outputs.",
  gravity_tank: "Inflow fills the tank while gravity-driven outflow increases with level.",
  gravity_tank_cascade: "The upstream tank drains freely into the downstream tank; both levels change through coupled balances.",
  robot_joint: "Torque moves the joint; gravity varies with angle, while friction resists motion.",
};

/** A drawing of the model's typical behavior, named with Rust's product name for it. */
export function PlantModelSchematic({ model, productName }: { model: string; productName: string }) {
  const summary = SUMMARIES[model] ?? "";
  return (
    <figure
      data-plant-model-diagram={model}
      role="img"
      aria-label={`${productName}: ${summary}`}
      style={FIGURE}
    >
      <figcaption style={CAPTION}>Typical behavior</figcaption>
      <div style={CONTENT}>
        <svg
          viewBox="0 0 260 76"
          width="260"
          height="76"
          preserveAspectRatio="xMidYMid meet"
          aria-hidden="true"
          focusable="false"
          style={SVG}
        >
          <SchematicArtwork model={model} />
        </svg>
        <span data-plant-model-diagram-summary style={SUMMARY}>{summary}</span>
      </div>
    </figure>
  );
}

function SchematicArtwork({ model }: { model: string }) {
  switch (model) {
    case "static_gain":
      return <>
        <Label x={5}>Input</Label><Arrow x1={36} x2={92} />
        <Block x={92} width={72} label="Scale + bias" /><Arrow x1={164} x2={222} />
        <Label x={224}>Output</Label>
      </>;
    case "first_order":
      return <>
        <Axes xLabel="" /><path d="M30 58 H58 V22 H76" style={INPUT_TRACE} />
        <path d="M30 58 H58 C70 28 96 22 190 22" style={OUTPUT_TRACE} />
        <Label x={90} y={72}>gradual settling</Label>
      </>;
    case "first_order_dead_time":
      return <>
        <Axes xLabel="" /><path d="M30 58 H58 V22 H76" style={INPUT_TRACE} />
        <path d="M30 58 H110 C122 28 148 22 218 22" style={OUTPUT_TRACE} />
        <Label x={78} y={72}>delay</Label><Label x={142} y={72}>gradual settling</Label>
      </>;
    case "integrator":
      return <>
        <Label x={5}>Difference</Label><Arrow x1={54} x2={96} />
        <Block x={96} width={48} label="Store ∫" /><Arrow x1={144} x2={190} />
        <path d="M190 58 L232 20" style={OUTPUT_TRACE} />
        <Label x={202} y={72}>ramp</Label>
      </>;
    case "second_order":
      return <>
        <Axes xLabel="" /><path d="M30 58 H58 V22 H76" style={INPUT_TRACE} />
        <path d="M30 58 H58 C71 58 91 8 111 18 S132 39 149 23 S169 29 187 22 S201 24 218 22" style={OUTPUT_TRACE} />
        <Label x={112} y={72}>may ring or settle smoothly</Label>
      </>;
    case "piecewise_linear":
      return <>
        <Axes xLabel="input" yLabel="output" />
        <polyline points="42,58 82,47 121,50 164,21 218,14" style={OUTPUT_TRACE} />
        {["42,58", "82,47", "121,50", "164,21", "218,14"].map((point) => {
          const [cx, cy] = point.split(",");
          return <circle key={point} cx={cx} cy={cy} r="2.6" style={POINT} />;
        })}
      </>;
    case "state_space":
      return <>
        <Label x={5}>Inputs</Label><Arrow x1={38} x2={78} />
        <Block x={78} width={104} label="Coupled states" /><Arrow x1={182} x2={222} />
        <Label x={224}>Outputs</Label>
        <path d="M154 52 C154 68 104 68 104 52" style={STATE_LOOP} />
      </>;
    case "gravity_tank":
      return <>
        <Label x={5}>Inflow</Label><Arrow x1={42} x2={91} y={18} />
        <path d="M91 16 V63 H169 V16" style={OUTLINE} />
        <path d="M94 38 H166 V61 H94 Z" style={WATER} />
        <Arrow x1={169} x2={220} y={56} /><Label x={222} y={59}>Outflow</Label>
        <Label x={111} y={33}>level</Label>
      </>;
    case "gravity_tank_cascade":
      return <>
        <Label x={4} y={15}>Pump</Label><Arrow x1={15} x2={50} y={23} />
        <path d="M50 10 V50 H110 V10" style={OUTLINE} />
        <path d="M53 28 H107 V48 H53 Z" style={WATER} />
        <Arrow x1={110} x2={150} y={45} /><Label x={75} y={23}>h1</Label>
        <path d="M150 26 V70 H210 V26" style={OUTLINE} />
        <path d="M153 50 H207 V68 H153 Z" style={WATER} />
        <Arrow x1={210} x2={249} y={64} /><Label x={175} y={44}>h2</Label>
      </>;
    case "robot_joint":
      return <>
        <Label x={5}>Torque</Label><Arrow x1={42} x2={88} />
        <circle cx="112" cy="38" r="12" style={OUTLINE} />
        <path d="M112 38 L166 16 L205 42" style={JOINT} />
        <path d="M140 38 A30 30 0 0 0 151 22" style={OUTPUT_TRACE} />
        <Label x={164} y={12}>angle</Label><Label x={185} y={59}>speed</Label>
      </>;
    default:
      return null;
  }
}

function Axes({ xLabel = "time", yLabel = "response" }: { xLabel?: string; yLabel?: string }) {
  return <>
    <path d="M30 10 V58 H230" style={AXIS} />
    <Label x={202} y={72}>{xLabel}</Label>
    <Label x={2} y={9}>{yLabel}</Label>
  </>;
}

function Arrow({ x1, x2, y = 38 }: { x1: number; x2: number; y?: number }) {
  return <>
    <line x1={x1} y1={y} x2={x2 - 7} y2={y} style={OUTLINE} />
    <path d={`M${x2 - 7} ${y - 4} L${x2} ${y} L${x2 - 7} ${y + 4}`} style={OUTLINE} />
  </>;
}

function Block({ x, width, label }: { x: number; width: number; label: string }) {
  return <>
    <rect x={x} y="22" width={width} height="32" rx="3" style={BLOCK} />
    <text x={x + width / 2} y="41" textAnchor="middle" style={TEXT}>{label}</text>
  </>;
}

function Label({ x, y = 41, children }: { x: number; y?: number; children: React.ReactNode }) {
  return <text x={x} y={y} style={TEXT}>{children}</text>;
}

const FIGURE: React.CSSProperties = {
  minWidth: 0,
  margin: "7px 0 0",
  padding: "7px 8px",
  border: "1px solid var(--vscode-widget-border, var(--trust-border))",
  borderRadius: 3,
  background: "var(--vscode-editorWidget-background, var(--trust-surface))",
  color: "var(--vscode-editor-foreground, var(--trust-text))",
};
const CAPTION: React.CSSProperties = { color: "var(--trust-text-muted)", fontSize: 10.5, fontWeight: 600 };
const CONTENT: React.CSSProperties = { display: "flex", alignItems: "center", flexWrap: "wrap", gap: 8, minWidth: 0 };
const SVG: React.CSSProperties = { display: "block", flex: "1 1 240px", minWidth: 0, maxWidth: 360, color: "var(--trust-text-muted)" };
const SUMMARY: React.CSSProperties = { flex: "1 1 170px", minWidth: 0, color: "var(--trust-text-muted)", fontSize: 10.5, lineHeight: 1.35 };
const STROKE: React.CSSProperties = { fill: "none", stroke: "currentColor", strokeWidth: 1.5, vectorEffect: "non-scaling-stroke" };
const OUTLINE: React.CSSProperties = STROKE;
const AXIS: React.CSSProperties = { ...STROKE, opacity: 0.55 };
const INPUT_TRACE: React.CSSProperties = { ...STROKE, opacity: 0.55 };
const OUTPUT_TRACE: React.CSSProperties = { ...STROKE, stroke: "var(--vscode-focusBorder, var(--trust-accent))", strokeWidth: 2 };
const STATE_LOOP: React.CSSProperties = { ...STROKE, strokeDasharray: "3 3" };
const JOINT: React.CSSProperties = { ...STROKE, strokeWidth: 3 };
const BLOCK: React.CSSProperties = { ...STROKE, fill: "var(--vscode-input-background, var(--trust-bg))" };
const WATER: React.CSSProperties = { fill: "var(--vscode-focusBorder, var(--trust-accent))", opacity: 0.25, stroke: "none" };
const POINT: React.CSSProperties = { fill: "var(--vscode-focusBorder, var(--trust-accent))", stroke: "none" };
const TEXT: React.CSSProperties = { fill: "currentColor", stroke: "none", fontSize: 8.5, fontFamily: "var(--vscode-font-family, sans-serif)" };
