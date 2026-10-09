// Entry of the runtime's copy of the drawing module (specification 39, §17.3): the runtime's
// HMI page draws with the same code as the HMI Builder. `npm run build:hmi-drawing` writes it to
// crates/trust-runtime/src/web/ui/modules/hmi-drawing.js as the global `TrustHmiDrawing`.

export { adaptPage } from "./adaptive";
export { priorityMarkImage } from "./alarms";
export { drawPage } from "./index";
export { renderToString } from "./svg";
export { registerSymbols } from "./symbols";
export { TOKEN_THEMES, tokensCss } from "./tokens";
// The editor's design samples, for `trust-runtime hmi test`'s pictures (§18 phase 5 item 6); the
// runtime display never draws them (§3.4).
export { designValues } from "../editor/designSamples";
