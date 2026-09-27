/** Layout only. Colours, typography and controls come from the shared truST theme. */
export const THREE_PAGE_STYLES = `
body { margin: 0; padding: 0; color: var(--trust-text); background: var(--trust-canvas); }
[hidden] { display: none !important; }
.control-studio { padding: 16px 24px 24px; min-width: 0; }
.studio-navigation { display: flex; gap: 12px; border-bottom: 1px solid var(--trust-border-subtle); margin-bottom: 24px; }
.studio-navigation button { color: var(--trust-text); background: transparent; border: 0; border-bottom: 2px solid transparent; border-radius: 0; padding: 10px 16px; cursor: pointer; font: inherit; }
.studio-navigation button[aria-selected="true"] { color: var(--trust-text); border-bottom-color: var(--trust-accent); }
button:focus-visible, select:focus-visible, input:focus-visible { outline: 1px solid var(--trust-accent); outline-offset: 2px; }
.studio-fields { display: grid; gap: 12px; max-width: 680px; }
.studio-field { display: grid; grid-template-columns: 150px minmax(180px, 1fr); align-items: center; gap: 16px; }
.studio-controller-row { display: flex; align-items: center; gap: 12px; }
.studio-controller-row > label { flex: 1; min-width: 0; }
.studio-field select, .studio-field input { box-sizing: border-box; width: 100%; min-width: 0; color: var(--trust-text); background: var(--trust-input-bg); border: 1px solid var(--trust-input-border); border-radius: 2px; padding: 6px 8px; font: inherit; }
.studio-field textarea { box-sizing: border-box; width: 100%; min-width: 0; color: var(--trust-text); background: var(--trust-input-bg); border: 1px solid var(--trust-input-border); padding: 6px 8px; font: inherit; resize: vertical; }
.studio-field select[multiple] { min-height: 32px; max-height: 100px; }
.studio-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-top: 20px; max-width: 680px; }
.studio-auxiliary { margin-left: auto; }
.studio-action-message { margin: 10px 0 0; color: var(--trust-text); }
.studio-result { margin-top: 24px; }
.studio-result p { margin: 8px 0; }
.studio-setting-values { display: grid; grid-template-columns: max-content 1fr; gap: 8px 24px; margin: 12px 0; }
.studio-setting-values dd { margin: 0; font-variant-numeric: tabular-nums; }
.studio-model-equation { font-family: var(--trust-mono); font-size: 1.1em; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 7.5em; overflow: auto; }
.studio-response { margin-top: 24px; min-width: 0; }
.studio-response-title { font-size: inherit; font-weight: normal; color: var(--trust-text); margin: 0 0 8px; }
.studio-chart { width: 100%; height: max(280px, calc(100vh - 340px)); min-width: 0; }
[data-analysis-selected="true"][data-selected-stage="tune"] .studio-response { display: none; }
.studio-experiment, .studio-identification { margin-top: 16px; max-width: 680px; }
.studio-experiment .parameter-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 16px; margin-top: 16px; }
.studio-experiment .parameter-grid input { box-sizing: border-box; }
.studio-experiment-plan { display: grid; grid-template-columns: 100px 1fr; gap: 8px 16px; }
.studio-experiment-plan dd { margin: 0; }
.studio-identification-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin-top: 12px; }
.studio-identification-grid label { display: grid; gap: 6px; }
.studio-identification-grid input { color: var(--trust-text); background: var(--trust-input-bg); border: 1px solid var(--trust-input-border); padding: 6px 8px; width: 100%; box-sizing: border-box; font: inherit; }
.studio-application { margin-top: 16px; }
.studio-requirements { border-collapse: collapse; width: 100%; text-align: left; }
.studio-validation-scroll { max-height: min(240px, 30vh); overflow: auto; }
.studio-validation-scroll:has(table[hidden]) { display: none; }
.studio-requirements th { position: sticky; top: 0; background: var(--trust-canvas); z-index: 1; }
.studio-requirements th, .studio-requirements td { padding: 6px 16px 6px 0; border-bottom: 1px solid var(--trust-border-subtle); }
@media(max-width: 650px) { .control-studio { padding: 12px; } .studio-field { grid-template-columns: 1fr; gap: 5px; } .studio-navigation { gap: 0; } .studio-navigation button { padding: 10px 12px; } .studio-auxiliary { margin-left: 0; } .studio-chart { height: 320px; } }
[data-binding-draft="true"] [data-studio-model-result], [data-binding-draft="true"] [data-studio-tuning-result], [data-binding-draft="true"] .studio-response, [data-binding-draft="true"] [data-studio-validation-result] { display: none; }
[data-studio-manual-settings] { border: 0; padding: 0; margin: 0; max-width: 680px; }
.studio-manual-setting { display: grid; grid-template-columns: 150px 180px 76px 48px; align-items: center; gap: 16px; margin: 8px 0; }
.studio-manual-setting input[type="checkbox"] { width: auto; }
.studio-manual-setting label:last-child { display: flex; align-items: center; gap: 6px; }
@media (max-width: 600px) { .studio-manual-setting { grid-template-columns: 1fr 1fr; } }
.studio-mimo { max-width: 680px; min-width: 0; margin-top: 16px; }
.studio-mimo-controls { display: grid; gap: 10px; min-width: 0; }
.studio-mimo-window-scroll { max-width: 100%; min-width: 0; overflow-x: auto; border: 1px solid var(--trust-border-subtle); }
.studio-mimo-windows { width: 100%; min-width: 0; border-collapse: collapse; table-layout: fixed; font-variant-numeric: tabular-nums; }
.studio-mimo-windows th, .studio-mimo-windows td { padding: 5px 4px; border-bottom: 1px solid var(--trust-border-subtle); text-align: left; min-width: 0; }
.studio-mimo-windows th { color: var(--trust-text-muted); font-size: 11px; font-weight: 600; white-space: nowrap; }
.studio-mimo-windows th:first-child, .studio-mimo-windows td:first-child { width: 64px; }
.studio-mimo-windows input { box-sizing: border-box; width: 100%; min-width: 0; color: var(--trust-text); background: var(--trust-input-bg); border: 1px solid var(--trust-input-border); border-radius: 2px; padding: 5px 4px; font: inherit; font-size: 0.9em; font-variant-numeric: tabular-nums; }
.studio-mimo-primary-result { margin-top: 16px; min-width: 0; }
.studio-mimo-primary-result h3 { margin: 0 0 8px; font-size: inherit; font-weight: 600; }
.studio-mimo-primary-result table { max-width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
.studio-mimo-primary-result th, .studio-mimo-primary-result td { padding: 4px 10px; border-bottom: 1px solid var(--trust-border-subtle); text-align: right; }
.studio-mimo-primary-result th:first-child, .studio-mimo-primary-result td:first-child { text-align: left; }
.studio-mimo > details { margin-top: 12px; }
.studio-mimo > details summary { cursor: pointer; color: var(--trust-accent); }
.studio-mimo [data-three-page-mimo-analysis-status], .studio-mimo [data-three-page-mimo-analysis-summary] { color: var(--trust-text-muted); margin: 8px 0; overflow-wrap: anywhere; }
@media (max-width: 600px) { .studio-mimo-windows th, .studio-mimo-windows td { padding: 4px 2px; } .studio-mimo-windows th { font-size: 10px; white-space: normal; } .studio-mimo-windows th:first-child, .studio-mimo-windows td:first-child { width: 48px; } .studio-mimo-windows input { padding: 4px 2px; } }
`;
